/*
* SPDX-FileCopyrightText: Copyright 2024 LG Electronics Inc.
* SPDX-License-Identifier: Apache-2.0
*/

pub mod container;

use common::nodeagent::fromactioncontroller::WorkloadCommand;
use hyper::{Body, Client, Method, Request, Uri};
use hyperlocal::{UnixConnector, Uri as UnixUri};

fn get_socket_path() -> String {
    std::env::var("PODMAN_SOCKET").unwrap_or_else(|_| "/var/run/podman/podman.sock".to_string())
}

pub async fn get(path: &str) -> Result<hyper::body::Bytes, hyper::Error> {
    let connector = UnixConnector;
    let client = Client::builder().build::<_, Body>(connector);

    let socket = get_socket_path();
    let uri: Uri = UnixUri::new(&socket, path).into();

    let res = client.get(uri).await?;
    hyper::body::to_bytes(res).await
}

pub async fn post(path: &str, body: Body) -> Result<hyper::body::Bytes, hyper::Error> {
    let connector = UnixConnector;
    let client = Client::builder().build::<_, Body>(connector);

    let socket = get_socket_path();
    let uri: Uri = UnixUri::new(&socket, path).into();

    let req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .body(body)
        .unwrap();

    let res = client.request(req).await?;
    hyper::body::to_bytes(res).await
}

pub async fn delete(path: &str) -> Result<hyper::body::Bytes, hyper::Error> {
    let connector = UnixConnector;
    let client = Client::builder().build::<_, Body>(connector);

    let socket = get_socket_path();
    let uri: Uri = UnixUri::new(&socket, path).into();

    let req = Request::builder()
        .method(Method::DELETE)
        .uri(uri)
        .body(Body::empty())
        .unwrap();

    let res = client.request(req).await?;
    hyper::body::to_bytes(res).await
}

pub async fn handle_workload(
    command: i32,
    pod: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    println!(
        "handle_workload called with command: {} for model(pod)",
        command
    );
    match command {
        x if x == WorkloadCommand::Start as i32 => {
            let container_ids = container::start(pod).await?;
            return Ok(container_ids);
        }
        x if x == WorkloadCommand::Stop as i32 => {
            container::stop(pod).await?;
        }
        x if x == WorkloadCommand::Restart as i32 => {
            container::restart(pod).await?;
        }
        _ => {
            // Do nothing for unimplemented commands
            return Err("unimplemented command".into());
        }
    };

    Ok(vec![])
}

//Unit tets cases
#[cfg(test)]
mod tests {
    use super::*;
    use hyper::body::Bytes;
    use hyper::Error;
    use tokio;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    #[tokio::test]
    async fn test_get_with_valid_path() {
        let _guard = match test_helpers::TEST_LOCK.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let (_tx, socket_path) = test_helpers::start_mock_server().await;
        std::env::set_var("PODMAN_SOCKET", &socket_path);

        let result: Result<Bytes, Error> = get("/v1.0/version").await;
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());

        std::env::remove_var("PODMAN_SOCKET");
        let _ = std::fs::remove_file(&socket_path);
    }

    #[tokio::test]
    async fn test_podman_mock_server_interaction() {
        let _guard = match test_helpers::TEST_LOCK.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let socket_dir = std::env::temp_dir();
        let socket_path = socket_dir.join("test_podman_mock.sock");
        let _ = std::fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path).unwrap();

        // Spawn mock Podman server accepting multiple connections in a loop
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    if let Ok(n) = stream.read(&mut buf).await {
                        let request_str = String::from_utf8_lossy(&buf[..n]);

                        let response_body = if request_str.contains("GET") {
                            r#"{"version":"1.0.0"}"#
                        } else if request_str.contains("POST") {
                            r#"{"Id":"mock_container_started"}"#
                        } else {
                            r#"{"status":"deleted"}"#
                        };

                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                            response_body.len(),
                            response_body
                        );
                        let _ = stream.write_all(response.as_bytes()).await;
                        let _ = stream.flush().await;
                    }
                });
            }
        });

        std::env::set_var("PODMAN_SOCKET", socket_path.to_str().unwrap());

        // Test GET
        let get_res = get("/v1.0/version").await;
        assert!(get_res.is_ok());
        assert!(String::from_utf8_lossy(&get_res.unwrap()).contains("1.0.0"));

        // Test POST
        let post_res = post("/v1.0/start", Body::empty()).await;
        assert!(post_res.is_ok());
        assert!(String::from_utf8_lossy(&post_res.unwrap()).contains("mock_container_started"));

        // Test DELETE
        let delete_res = delete("/v1.0/delete").await;
        assert!(delete_res.is_ok());
        assert!(String::from_utf8_lossy(&delete_res.unwrap()).contains("deleted"));

        std::env::remove_var("PODMAN_SOCKET");
        let _ = std::fs::remove_file(&socket_path);
    }

    #[tokio::test]
    async fn test_handle_workload_unimplemented() {
        let result = handle_workload(999, "test-pod").await;
        assert!(result.is_err());
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    pub async fn start_mock_server() -> (tokio::sync::oneshot::Sender<()>, String) {
        let socket_dir = std::env::temp_dir();
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let socket_path =
            socket_dir.join(format!("test_podman_{}_{}.sock", std::process::id(), id));
        let _ = std::fs::remove_file(&socket_path);

        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
        let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            tokio::select! {
                _ = async {
                    while let Ok((mut stream, _)) = listener.accept().await {
                        tokio::spawn(async move {
                            let mut buf = [0u8; 4096];
                            if let Ok(n) = stream.read(&mut buf).await {
                                let req = String::from_utf8_lossy(&buf[..n]);

                                let response_body = if req.contains("containers/json") {
                                    r#"[{"Id":"test_id_123","Names":["/test_container"],"Image":"test_image","State":"running","Status":"Up 5 seconds"}]"#
                                } else if req.contains("json?all=true") {
                                    r#"{"Id":"test_id_123","Name":"/test_container","State":{"Status":"running","Running":true,"Paused":false,"Restarting":false,"OOMKilled":false,"Dead":false,"Pid":1234,"ExitCode":0,"Error":"","StartedAt":"2026-07-24","FinishedAt":""},"Config":{"Hostname":"test-hostname","Image":"test_image","Domainname":"","User":"","AttachStdin":false,"AttachStdout":true,"AttachStderr":true,"Tty":false,"OpenStdin":false,"StdinOnce":false,"WorkingDir":"/","Annotations":{}}}"#
                                } else if req.contains("stats") {
                                    r#"{"cpu_stats":{"cpu_usage":{"total_usage":1000,"usage_in_kernelmode":500,"usage_in_usermode":500}},"memory_stats":{"usage":2000,"limit":4000},"networks":{"eth0":{"rx_bytes":100,"tx_bytes":200}}}"#
                                } else if req.contains("images/json") {
                                    r#"[{"Id":"test_id_123","RepoTags":["alpine:latest"]}]"#
                                } else {
                                    r#"{"Id":"test_id_123","version":"1.0.0"}"#
                                };

                                let response = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                                    response_body.len(),
                                    response_body
                                );
                                let _ = stream.write_all(response.as_bytes()).await;
                                let _ = stream.flush().await;
                            }
                        });
                    }
                } => {}
                _ = &mut rx => {}
            }
        });

        (tx, socket_path.to_str().unwrap().to_string())
    }
}
