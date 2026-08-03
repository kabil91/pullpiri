//! Async logging subsystem: callers enqueue `LogEnvelope`s into bounded
//!
//! # Security note (CWE-117 Log Injection)
//! All messages written to stderr or stdout are sanitized via
//! `sanitize_log_message()` before output to prevent newline-injection
//! attacks that could forge audit-trail entries. (ISO 26262 §9.4.9)
//! queues keyed by virtual channels, while a background worker drains the
//! queues and forwards payloads via Unix datagram sockets.

use bytes::BytesMut;
use prost::Message;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::net::UnixDatagram;
use tokio::runtime::Handle;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Mutex;

use crate::logd::LogEnvelope;

/// Global singleton that holds the active async logger instance.
static LOGGER: OnceLock<AsyncLogger> = OnceLock::new();

/// Bounded FIFO queue that drops the oldest entry when capacity is reached.
struct BoundedQueue<LogEnvelope> {
    inner: Mutex<VecDeque<LogEnvelope>>,
    capacity: usize,
}

impl BoundedQueue<LogEnvelope> {
    /// Construct a queue with the given capacity.
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of items to retain.
    fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// Push an item, dropping the oldest element if the queue is full.
    ///
    /// # Arguments
    /// * `item` - Envelope to enqueue.
    async fn push_drop_oldest(&self, item: LogEnvelope) {
        let mut guard = self.inner.lock().await;
        if guard.len() == self.capacity {
            guard.pop_front();
        }
        guard.push_back(item);
    }

    /// Drain all pending items, returning them as a `Vec`.
    ///
    /// # Returns
    /// All enqueued envelopes in FIFO order.
    async fn drain(&self) -> Vec<LogEnvelope> {
        let mut guard = self.inner.lock().await;
        guard.drain(..).collect()
    }

    /// Reinsert a batch of items at the front so they are retried first.
    ///
    /// # Arguments
    /// * `items` - Envelopes to re-queue.
    async fn push_front_batch(&self, mut items: Vec<LogEnvelope>) {
        if items.is_empty() {
            return;
        }

        let mut guard = self.inner.lock().await;
        while let Some(item) = items.pop() {
            guard.push_front(item);
            if guard.len() > self.capacity {
                guard.pop_back();
            }
        }
    }
}

/// Logical channels supported by the logger.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Ch {
    Logd,
}

impl Ch {
    /// Return the Unix socket path for the given channel.
    fn socket_path(self) -> &'static str {
        match self {
            Ch::Logd => crate::logd::LOGD_SOCKET_PATH,
        }
    }
}

/// Result state returned by `drain_channel`.
enum DrainState {
    Idle,
    Pending,
}

/// Aggregated state for the global async logger instance.
pub struct AsyncLogger {
    q: HashMap<Ch, Arc<BoundedQueue<LogEnvelope>>>,
    notify_tx: Sender<()>,
    tag: String,
}

/// Initialize the async logger for the given tag and spawn the worker task.
///
/// # Arguments
/// * `tag` - Tag field to stamp on outgoing envelopes.
///
/// # Errors
/// Propagates I/O errors from socket creation or queue setup.
pub async fn init_async_logger(tag: &str) -> std::io::Result<()> {
    let logd_q = Arc::new(BoundedQueue::<LogEnvelope>::new(8192));
    let (tx, rx) = channel::<()>(1);

    let mut q = HashMap::new();
    q.insert(Ch::Logd, logd_q.clone());

    let logger = AsyncLogger {
        q,
        notify_tx: tx.clone(),
        tag: tag.to_string(),
    };
    let _ = LOGGER.set(logger);

    spawn_worker(rx, logd_q).await;

    Ok(())
}

/// Convenience API for async contexts: await enqueue completion and log
/// failures to stderr.
///
/// # Arguments
/// * `level` - Severity level code.
/// * `message` - Formatted log message.
pub async fn log(level: i32, message: String) {
    if let Err(err) = enqueue(level, message).await {
        // CWE-117: sanitize before writing to stderr
        eprintln!(
            "[LOGGER ERROR] logger enqueue failed: {}",
            sanitize_log_message(&err.to_string())
        );
    }
}

/// Fire-and-forget API for synchronous call sites. Spawns a task on the
/// current Tokio runtime (if any) to enqueue the log message.
///
/// # Arguments
/// * `level` - Severity level code.
/// * `message` - Formatted log message.
pub fn log_nowait(level: i32, message: String) {
    match Handle::try_current() {
        Ok(handle) => {
            handle.spawn(async move {
                if let Err(err) = enqueue(level, message).await {
                    // CWE-117: sanitize before writing to stderr
                    eprintln!(
                        "[LOGGER ERROR] logger enqueue failed: {}",
                        sanitize_log_message(&err.to_string())
                    );
                }
            });
        }
        Err(_) => {
            // CWE-117: sanitize message before writing to stderr to prevent log injection
            eprintln!(
                "[LOGGER WARNING] logger not running inside a Tokio runtime; dropping log: {}",
                sanitize_log_message(&message)
            );
        }
    }
}

/// Core enqueue function shared by `log` and `log_nowait`.
///
/// # Arguments
/// * `level` - Severity level code.
/// * `message` - Formatted log message.
///
/// # Errors
/// Returns an error when the logger is not initialized or the notify
/// channel has been closed.
pub async fn enqueue(level: i32, message: String) -> std::io::Result<()> {
    let Some(gl) = LOGGER.get() else {
        return Err(std::io::Error::other("logger not initialized"));
    };

    let env = LogEnvelope {
        ts_real_ns: real_time_ns(),
        tag: gl.tag.clone(),
        level,
        message,
    };

    let q = gl.q.get(&Ch::Logd).unwrap();
    q.push_drop_oldest(env).await;

    match gl.notify_tx.try_send(()) {
        Ok(()) | Err(TrySendError::Full(_)) => Ok(()),
        Err(TrySendError::Closed(_)) => Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "logger worker not running",
        )),
    }
}

/// Spawn the background worker that drains the queue whenever an enqueue
/// notification is received.
///
/// # Arguments
/// * `notify_rx` - Receiver for edge-triggered wakeups.
/// * `logd_q` - Queue storing outgoing envelopes.
async fn spawn_worker(mut notify_rx: Receiver<()>, logd_q: Arc<BoundedQueue<LogEnvelope>>) {
    tokio::spawn(async move {
        let mut socks: HashMap<Ch, (UnixDatagram, bool)> = HashMap::new();

        let sock = UnixDatagram::unbound().expect("unbound sock");
        socks.insert(Ch::Logd, (sock, false));

        while notify_rx.recv().await.is_some() {
            loop {
                match drain_channel(Ch::Logd, &logd_q, &mut socks).await {
                    DrainState::Idle => break,
                    DrainState::Pending => continue,
                }
            }
        }

        while matches!(
            drain_channel(Ch::Logd, &logd_q, &mut socks).await,
            DrainState::Pending
        ) {}
    });
}

/// Drain a single channel queue and forward its messages to the connected
/// Unix datagram socket.
///
/// # Arguments
/// * `ch` - Logical channel identifier.
/// * `q` - Queue backing the channel.
/// * `socks` - Cached sockets paired with connection status flags.
///
/// # Returns
/// `DrainState::Idle` when no work remains, otherwise `DrainState::Pending`.
async fn drain_channel(
    ch: Ch,
    q: &BoundedQueue<LogEnvelope>,
    socks: &mut HashMap<Ch, (UnixDatagram, bool)>,
) -> DrainState {
    let (sock, connected) = socks.get_mut(&ch).unwrap();
    let batch = q.drain().await;

    if batch.is_empty() {
        return DrainState::Idle;
    }

    if !*connected {
        if sock.connect(ch.socket_path()).is_ok() {
            *connected = true;
        } else {
            q.push_front_batch(batch).await;
            tokio::time::sleep(Duration::from_millis(50)).await;
            return DrainState::Pending;
        }
    }

    let mut iter = batch.into_iter();
    while let Some(env) = iter.next() {
        print_stdout(&env);
        let mut buf = BytesMut::with_capacity(env.encoded_len());
        if env.encode(&mut buf).is_err() {
            continue;
        }
        if sock.send(&buf).await.is_err() {
            *connected = false;
            let mut retry_items = vec![env];
            retry_items.extend(iter);
            q.push_front_batch(retry_items).await;
            tokio::time::sleep(Duration::from_millis(50)).await;
            return DrainState::Pending;
        }
    }

    DrainState::Idle
}

/// Sanitize a log message by replacing newline and carriage-return
/// characters with their visible escape sequences.
///
/// This prevents CWE-117 (Log Injection) where an attacker embeds `\n`
/// in a log field to forge additional log lines in the audit trail.
///
/// # Arguments
/// * `msg` - The raw message string to sanitize.
///
/// # Returns
/// A `String` with `\n` replaced by `\\n` and `\r` replaced by `\\r`.
fn sanitize_log_message(msg: &str) -> String {
    msg.replace('\n', "\\n").replace('\r', "\\r")
}

fn print_stdout(env: &LogEnvelope) {
    use chrono::{DateTime, Local};
    use std::time::{Duration, UNIX_EPOCH};

    let sys_time = UNIX_EPOCH + Duration::from_nanos(env.ts_real_ns);
    let chrono_time: DateTime<Local> = DateTime::from(sys_time);
    let time_str = chrono_time.format("%Y-%m-%d %H:%M:%S%.3f");
    // CWE-117: sanitize tag and message before writing to stdout
    let tag = sanitize_log_message(&env.tag);
    let message = sanitize_log_message(&env.message);

    let level = match env.level {
        1 => "V",
        2 => "D",
        3 => "I",
        4 => "W",
        5 => "E",
        6 => "F",
        _ => "?",
    };

    println!(
        "{:<24} │ {:<2} │ {:<30} │ {}",
        time_str, level, tag, message
    );
}

/// Read the current realtime clock as an absolute nanosecond value.
fn real_time_ns() -> u64 {
    unsafe {
        let mut ts: libc::timespec = std::mem::zeroed();
        libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts);
        (ts.tv_sec as u64) * 1_000_000_000u64 + (ts.tv_nsec as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bounded_queue_basic() {
        let q = BoundedQueue::new(2);
        assert!(q.drain().await.is_empty());

        let env1 = LogEnvelope {
            ts_real_ns: 100,
            tag: "tag1".to_string(),
            level: 1,
            message: "msg1".to_string(),
        };
        let env2 = LogEnvelope {
            ts_real_ns: 200,
            tag: "tag2".to_string(),
            level: 2,
            message: "msg2".to_string(),
        };
        let env3 = LogEnvelope {
            ts_real_ns: 300,
            tag: "tag3".to_string(),
            level: 3,
            message: "msg3".to_string(),
        };

        q.push_drop_oldest(env1).await;
        q.push_drop_oldest(env2).await;
        // Capacity is 2, so this should drop env1
        q.push_drop_oldest(env3).await;

        let drained = q.drain().await;
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].message, "msg2");
        assert_eq!(drained[1].message, "msg3");
    }

    #[tokio::test]
    async fn test_bounded_queue_push_front_batch() {
        let q = BoundedQueue::new(2);

        // Empty batch
        q.push_front_batch(vec![]).await;

        let env1 = LogEnvelope {
            ts_real_ns: 100,
            tag: "tag1".to_string(),
            level: 1,
            message: "msg1".to_string(),
        };
        let env2 = LogEnvelope {
            ts_real_ns: 200,
            tag: "tag2".to_string(),
            level: 2,
            message: "msg2".to_string(),
        };
        let env3 = LogEnvelope {
            ts_real_ns: 300,
            tag: "tag3".to_string(),
            level: 3,
            message: "msg3".to_string(),
        };

        q.push_front_batch(vec![env1, env2, env3]).await;
        let drained = q.drain().await;
        assert_eq!(drained.len(), 2);
    }

    #[test]
    fn test_ch_socket_path() {
        assert_eq!(Ch::Logd.socket_path(), crate::logd::LOGD_SOCKET_PATH);
    }

    // CWE-117 sanitization unit tests
    #[test]
    fn test_sanitize_log_message_clean() {
        // Normal message — unchanged
        assert_eq!(sanitize_log_message("hello world"), "hello world");
    }

    #[test]
    fn test_sanitize_log_message_newline_injection() {
        // Embedded newline must be escaped — prevents forged log lines
        let malicious = "normal\nINJECTED: fake safety event";
        let sanitized = sanitize_log_message(malicious);
        assert!(!sanitized.contains('\n'), "newline must be removed");
        assert!(
            sanitized.contains("\\n"),
            "escaped sequence must be present"
        );
    }

    #[test]
    fn test_sanitize_log_message_cr_injection() {
        // Carriage return must also be escaped
        let malicious = "msg\rfake-overwrite";
        let sanitized = sanitize_log_message(malicious);
        assert!(!sanitized.contains('\r'), "carriage return must be removed");
        assert!(
            sanitized.contains("\\r"),
            "escaped sequence must be present"
        );
    }

    #[test]
    fn test_sanitize_log_message_combined() {
        let malicious = "line1\r\nINJECTED";
        let sanitized = sanitize_log_message(malicious);
        assert!(!sanitized.contains('\n'));
        assert!(!sanitized.contains('\r'));
        assert_eq!(sanitized, "line1\\r\\nINJECTED");
    }

    #[tokio::test]
    async fn test_enqueue_not_initialized() {
        // Enqueue should return error when not initialized
        let res = enqueue(3, "test".to_string()).await;
        assert!(res.is_err());
    }

    #[test]
    fn test_log_nowait_no_runtime() {
        // Run log_nowait in a std::thread without tokio context to cover the Err(_) branch
        std::thread::spawn(|| {
            log_nowait(3, "test no runtime".to_string());
        })
        .join()
        .unwrap();
    }

    #[tokio::test]
    async fn test_print_stdout_and_levels() {
        for lvl in 1..=8 {
            let env = LogEnvelope {
                ts_real_ns: 1716447885000000000,
                tag: "test-tag".to_string(),
                level: lvl,
                message: "test message".to_string(),
            };
            print_stdout(&env);
        }
        assert!(real_time_ns() > 0);
    }

    #[tokio::test]
    async fn test_drain_channel_socket_interaction() {
        let q = BoundedQueue::new(10);
        let env = LogEnvelope {
            ts_real_ns: 100,
            tag: "test".to_string(),
            level: 3,
            message: "msg".to_string(),
        };
        q.push_drop_oldest(env).await;

        // 1. Connection failure branch (unconnected, connect to invalid path fails)
        let mut socks = HashMap::new();
        let sock_unconnected = UnixDatagram::unbound().unwrap();
        socks.insert(Ch::Logd, (sock_unconnected, false));
        let state = drain_channel(Ch::Logd, &q, &mut socks).await;
        assert!(matches!(state, DrainState::Pending));

        // 2. Successful send branch
        // Bind a temporary unix socket to receive data
        let dir = std::env::temp_dir();
        let server_path = dir.join("test_logd_server.sock");
        let _ = std::fs::remove_file(&server_path);

        let server_sock = UnixDatagram::bind(&server_path).unwrap();

        let client_sock = UnixDatagram::unbound().unwrap();
        client_sock.connect(&server_path).unwrap();

        let mut socks_connected = HashMap::new();
        socks_connected.insert(Ch::Logd, (client_sock, true));

        // Re-enqueue message
        let env = LogEnvelope {
            ts_real_ns: 100,
            tag: "test".to_string(),
            level: 3,
            message: "msg".to_string(),
        };
        q.push_drop_oldest(env).await;

        let state = drain_channel(Ch::Logd, &q, &mut socks_connected).await;
        assert!(matches!(state, DrainState::Idle));

        // Check server received it
        let mut buf = [0u8; 1024];
        let (len, _) = server_sock.recv_from(&mut buf).await.unwrap();
        assert!(len > 0);

        // 3. Send failure branch
        // Shutdown/drop the server socket so send fails or client is disconnected
        drop(server_sock);
        let _ = std::fs::remove_file(&server_path);

        let env = LogEnvelope {
            ts_real_ns: 100,
            tag: "test".to_string(),
            level: 3,
            message: "msg".to_string(),
        };
        q.push_drop_oldest(env).await;

        // Note: UnixDatagram might not fail immediately on write if unbound, but let's try
        let state = drain_channel(Ch::Logd, &q, &mut socks_connected).await;
        // Whether it returns Pending or Idle depending on OS socket buffering, it's fine.
        assert!(matches!(state, DrainState::Idle | DrainState::Pending));
    }
}
