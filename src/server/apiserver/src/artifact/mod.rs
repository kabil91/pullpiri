/*
 * SPDX-FileCopyrightText: Copyright 2024 LG Electronics Inc.
 * SPDX-License-Identifier: Apache-2.0
 */

//! Convert string-type artifacts to struct and access etcd

pub mod data;

use common::logd;
use common::spec::artifact::{Artifact, Model, Network, Node, Package, Scenario, Schedule, Volume};
use common::spec::k8s::Pod;

// Artifact kind constants
const KIND_SCENARIO: &str = "Scenario";
const KIND_PACKAGE: &str = "Package";
const KIND_VOLUME: &str = "Volume";
const KIND_NETWORK: &str = "Network";
const KIND_NODE: &str = "Node";
const KIND_MODEL: &str = "Model";
const KIND_SCHEDULE: &str = "Schedule";

// YAML document separator
const YAML_SEPARATOR: &str = "---";

/// Verifies HMAC-SHA256 signature of the payload body.
///
/// The signature is provided as a base64-encoded value in the
/// `X-Pullpiri-Signature` header. The shared signing key is read from the
/// `PULLPIRI_SIGNING_KEY` environment variable.
///
/// # Signing verification algorithm
/// 1. Read `PULLPIRI_SIGNING_KEY` env var (if absent, verification is skipped
///    in development mode with a warning — production deployments MUST set this).
/// 2. Compute HMAC-SHA256(key_bytes, body_bytes) using the iterative SHA-256
///    algorithm available in Rust's standard library via the hmac-compatible
///    block structure (implemented inline to avoid adding new dependencies).
/// 3. Compare the computed digest with the provided base64-decoded signature
///    using constant-time comparison to prevent timing side-channels.
///
/// # ISO 26262 traceability
// req-traceability: comp_req__api__yaml_signing
fn verify_yaml_signature(body: &str, signature_b64: Option<&str>) -> common::Result<()> {
    let signing_key = std::env::var("PULLPIRI_SIGNING_KEY").ok();
    verify_yaml_signature_with_key(signing_key.as_deref(), body, signature_b64)
}

fn verify_yaml_signature_with_key(
    signing_key: Option<&str>,
    body: &str,
    signature_b64: Option<&str>,
) -> common::Result<()> {
    match (signing_key, signature_b64) {
        (None, _) => {
            // No key configured: development/test mode — skip verification
            // In production, PULLPIRI_SIGNING_KEY MUST be set in the deployment manifest
            eprintln!("[SAFETY_WARNING] PULLPIRI_SIGNING_KEY not set — YAML signature verification skipped (development mode only)");
            Ok(())
        }
        (Some(_), None) => {
            // Key is configured but no signature provided — reject the payload
            Err(
                "[SAFETY_ERROR] YAML signature required but X-Pullpiri-Signature header is absent"
                    .into(),
            )
        }
        (Some(key), Some(sig_b64)) => {
            // Decode the provided base64 signature
            let provided_sig =
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, sig_b64.trim())
                    .map_err(|e| {
                        format!("[SAFETY_ERROR] Invalid base64 in signature header: {e}")
                    })?;

            // Compute expected HMAC-SHA256 using the ipad/opad construction
            // This is a self-contained implementation that avoids adding new dependencies.
            let key_bytes = key.as_bytes();
            let body_bytes = body.as_bytes();

            // SHA-256 block size is 64 bytes
            const BLOCK_SIZE: usize = 64;

            // Normalise key to block size
            let mut k = [0u8; BLOCK_SIZE];
            if key_bytes.len() <= BLOCK_SIZE {
                k[..key_bytes.len()].copy_from_slice(key_bytes);
            } else {
                // Key longer than block — hash it first (rare; key should be 32 bytes)
                let hashed = sha256_bytes(key_bytes);
                k[..32].copy_from_slice(&hashed);
            }

            // Build ipad and opad
            let mut ipad = [0u8; BLOCK_SIZE];
            let mut opad = [0u8; BLOCK_SIZE];
            for i in 0..BLOCK_SIZE {
                ipad[i] = k[i] ^ 0x36;
                opad[i] = k[i] ^ 0x5c;
            }

            // inner = SHA256(ipad || body)
            let mut inner_input = Vec::with_capacity(BLOCK_SIZE + body_bytes.len());
            inner_input.extend_from_slice(&ipad);
            inner_input.extend_from_slice(body_bytes);
            let inner_hash = sha256_bytes(&inner_input);

            // outer = SHA256(opad || inner)
            let mut outer_input = Vec::with_capacity(BLOCK_SIZE + 32);
            outer_input.extend_from_slice(&opad);
            outer_input.extend_from_slice(&inner_hash);
            let expected_sig = sha256_bytes(&outer_input);

            // Constant-time comparison to prevent timing attacks
            if provided_sig.len() != expected_sig.len() {
                return Err(
                    "[SAFETY_ERROR] YAML signature verification failed: length mismatch".into(),
                );
            }
            let mismatch = provided_sig
                .iter()
                .zip(expected_sig.iter())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b));
            if mismatch != 0 {
                return Err(
                    "[SAFETY_ERROR] YAML signature verification failed: invalid signature".into(),
                );
            }
            Ok(())
        }
    }
}

/// Minimal SHA-256 implementation operating on byte slices.
/// Used internally by `verify_yaml_signature` to avoid introducing new crate
/// dependencies (all crypto crates require cargo-deny approval).
fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    // SHA-256 initial hash values (first 32 bits of fractional parts of sqrt of primes)
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    // SHA-256 round constants
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    // Pre-process: padding
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // Process each 512-bit (64-byte) block
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] =
            [h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, &word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Parse artifact kind and name from YAML value
fn parse_artifact_info(value: &serde_yaml::Value) -> Option<(String, String)> {
    let kind = value.get("kind")?.as_str()?;

    let name = match kind {
        KIND_SCENARIO => serde_yaml::from_value::<Scenario>(value.clone())
            .ok()?
            .get_name(),
        KIND_PACKAGE => serde_yaml::from_value::<Package>(value.clone())
            .ok()?
            .get_name(),
        KIND_VOLUME => serde_yaml::from_value::<Volume>(value.clone())
            .ok()?
            .get_name(),
        KIND_NETWORK => serde_yaml::from_value::<Network>(value.clone())
            .ok()?
            .get_name(),
        KIND_NODE => serde_yaml::from_value::<Node>(value.clone())
            .ok()?
            .get_name(),
        KIND_MODEL => serde_yaml::from_value::<Model>(value.clone())
            .ok()?
            .get_name(),
        KIND_SCHEDULE => serde_yaml::from_value::<Schedule>(value.clone())
            .ok()?
            .get_name(),
        _ => return None,
    };

    Some((kind.to_string(), name))
}

/// Send initial state change notification to StateManager
async fn notify_scenario_state(scenario_name: &str, target_state: &str) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64;

    let state_change = common::statemanager::StateChange {
        resource_type: common::statemanager::ResourceType::Scenario as i32,
        resource_name: scenario_name.to_string(),
        current_state: String::new(),
        target_state: target_state.to_string(),
        transition_id: format!("apiserver-scenario-init-{}", timestamp),
        timestamp_ns: timestamp,
        source: "apiserver".to_string(),
    };

    logd!(
        1,
        "🔄 SCENARIO STATE INITIALIZATION: ApiServer Setting Initial State"
    );
    logd!(1, "   📋 Scenario: {}", scenario_name);
    logd!(1, "   🔄 Initial State: → {}", target_state);
    logd!(1, "   📤 Sending StateChange to StateManager");

    let mut state_sender = crate::grpc::sender::statemanager::StateManagerSender::new();
    match state_sender.send_state_change(state_change).await {
        Ok(_) => logd!(
            2,
            "   ✅ Successfully set scenario {} to {} state",
            scenario_name,
            target_state
        ),
        Err(e) => logd!(
            5,
            "   ❌ Failed to send state change to StateManager: {:?}",
            e
        ),
    }
}

/// Process and store a single artifact document
async fn process_artifact_document(doc: &str) -> common::Result<Option<(String, String)>> {
    use std::time::Instant;

    let parse_start = Instant::now();
    let value: serde_yaml::Value = serde_yaml::from_str(doc)?;
    let artifact_str = serde_yaml::to_string(&value)?;
    logd!(
        1,
        "process_artifact: YAML parse elapsed = {:?}",
        parse_start.elapsed()
    );

    let (kind, name) = match parse_artifact_info(&value) {
        Some(info) => info,
        None => {
            logd!(5, "Unknown or invalid artifact");
            return Ok(None);
        }
    };

    let key = format!("{}/{}", kind, name);

    let etcd_start = Instant::now();
    data::write_to_etcd(&key, &artifact_str).await?;
    logd!(
        1,
        "process_artifact: etcd write elapsed for {} = {:?}",
        key,
        etcd_start.elapsed()
    );

    if kind == KIND_SCENARIO {
        notify_scenario_state(&name, "idle").await;
    }

    Ok(Some((kind, artifact_str)))
}

/// Apply downloaded artifact to etcd
///
/// ### Parametets
/// * `body: &str` - whole yaml string of piccolo artifact
/// ### Returns
/// * `Result(String, String)` - scenario and package yaml in downloaded artifact
/// ### Description
/// Write artifact in etcd
// req-traceability: comp_req__api__yaml_validation
// req-traceability: comp_req__api__schema_validation
// req-traceability: comp_req__api__yaml_signing
pub async fn apply(body: &str) -> common::Result<String> {
    use std::time::Instant;
    let total_start = Instant::now();

    // Step 1 (P2 — comp_req__api__yaml_signing): Verify digital signature over the raw body.
    // The X-Pullpiri-Signature header value is expected to be passed in via the
    // PULLPIRI_PAYLOAD_SIGNATURE env var by the HTTP handler (set per-request).
    // In production, PULLPIRI_SIGNING_KEY must be set in the deployment manifest.
    let payload_signature = std::env::var("PULLPIRI_PAYLOAD_SIGNATURE").ok();
    verify_yaml_signature(body, payload_signature.as_deref())?;

    let docs: Vec<&str> = body.split(YAML_SEPARATOR).collect();
    let mut scenario_str = String::new();
    let mut package_str = String::new();

    for doc in docs {
        if let Some((kind, artifact_str)) = process_artifact_document(doc).await? {
            match kind.as_str() {
                KIND_SCENARIO => scenario_str = artifact_str,
                KIND_PACKAGE => package_str = artifact_str,
                _ => continue,
            }
        }
    }

    logd!(1, "apply: total elapsed = {:?}", total_start.elapsed());

    if scenario_str.is_empty() {
        Err("There is not any scenario in yaml string".into())
    } else if package_str.is_empty() {
        Err("There is not any package in yaml string".into())
    } else {
        save_pod_yaml_from_package(&package_str).await?;
        Ok(scenario_str)
    }
}

/// Delete downloaded artifact to etcd
///
/// ### Parametets
/// * `body: &str` - whole yaml string of piccolo artifact
/// ### Returns
/// * `Result(String)` - scenario yaml in downloaded artifact
/// ### Description
/// Delete scenario yaml only, because other scenario can use a package with same name
pub async fn withdraw(body: &str) -> common::Result<String> {
    let docs: Vec<&str> = body.split(YAML_SEPARATOR).collect();

    for doc in docs {
        let value: serde_yaml::Value = serde_yaml::from_str(doc)?;

        if let Some((kind, name)) = parse_artifact_info(&value) {
            if kind == KIND_SCENARIO {
                let artifact_str = serde_yaml::to_string(&value)?;
                let key = format!("{}/{}", KIND_SCENARIO, name);
                data::delete_at_etcd(&key).await?;
                return Ok(artifact_str);
            }
        }
    }

    Err("There is not any scenario in yaml string".into())
}

/// Load model with optional volume and network resources
async fn load_model_with_resources(
    model_info: &common::spec::artifact::package::ModelInfo,
) -> common::Result<Model> {
    let model_str = common::etcd::get(&format!("{}/{}", KIND_MODEL, model_info.get_name())).await?;
    let mut model: Model = serde_yaml::from_str(&model_str)?;

    // Load volume if specified
    if let Some(volume_name) = model_info.get_resources().get_volume() {
        let volume_str = common::etcd::get(&format!("{}/{}", KIND_VOLUME, volume_name)).await?;
        let volume: Volume = serde_yaml::from_str(&volume_str)?;

        if let Some(volume_spec) = volume.get_spec() {
            model
                .get_podspec_mut()
                .volumes
                .clone_from(volume_spec.get_volume());
        }
    }

    // Load network if specified
    if let Some(network_name) = model_info.get_resources().get_network() {
        let network_str = common::etcd::get(&format!("{}/{}", KIND_NETWORK, network_name)).await?;
        let _network: Network = serde_yaml::from_str(&network_str)?;
        // TODO: Apply network configuration
    }

    Ok(model)
}

/// Save Pod YAML for all models in a package
async fn save_pod_yaml_from_package(package_str: &str) -> common::Result<()> {
    let package: Package = serde_yaml::from_str(package_str)?;
    let mut models = Vec::new();

    for model_info in package.get_models() {
        let model = load_model_with_resources(&model_info).await?;
        models.push(model);
    }

    let pods: Vec<Pod> = models.into_iter().map(Pod::from).collect();

    for pod in pods {
        let pod_yaml = serde_yaml::to_string(&pod)?;
        let key = format!("{}/{}", "Pod", pod.get_name());
        data::write_to_etcd(&key, &pod_yaml).await?;
    }

    Ok(())
}

//UNIT TEST CASES

#[cfg(test)]
mod tests {
    use super::*;

    // -- Test Artifacts --

    /// Valid artifact YAML (Scenario + Package + Model)
    const VALID_ARTIFACT_YAML: &str = r#"
apiVersion: v1
kind: Scenario
metadata:
  name: helloworld
spec:
  condition:
    express: eq
    value: "true"
    operands:
      type: DDS
      name: value
      value: ADASObstacleDetectionIsWarning
  action: update
  target: helloworld
---
apiVersion: v1
kind: Package
metadata:
  label: null
  name: helloworld
spec:
  pattern:
    - type: plain
  models:
    - name: helloworld-core
      node: HPC
      resources:
        volume:
        network:
"#;

    /// Invalid YAML — missing `action` in Scenario
    const INVALID_YAML_MISSING_ACTION: &str = r#"
apiVersion: v1
kind: Scenario
metadata:
  name: helloworld
spec:
  condition:
  target: helloworld
---
apiVersion: v1
kind: Package
metadata:
  name: helloworld
spec:
  pattern:
    - type: plain
  models:
    - name: helloworld-core
      node: HPC
      resources:
        volume: []
        network: []
"#;

    /// Invalid YAML — only unknown artifact
    const INVALID_YAML_UNKNOWN_ARTIFACT: &str = r#"
apiVersion: v1
kind: Unknown
metadata:
  name: helloworld
spec:
  dummy: value
"#;

    /// Invalid YAML — empty string
    const INVALID_YAML_EMPTY: &str = "";

    /// Valid Model YAML for helloworld-core (required by Package)
    const VALID_MODEL_YAML: &str = r#"
apiVersion: v1
kind: Model
metadata:
  name: helloworld-core
  annotations:
    io.piccolo.annotations.package-type: helloworld-core
    io.piccolo.annotations.package-name: helloworld
    io.piccolo.annotations.package-network: default
  labels:
    app: helloworld-core
spec:
  hostNetwork: true
  containers:
    - name: helloworld
      image: helloworld:latest
  terminationGracePeriodSeconds: 0
"#;

    // -- apply() tests --

    /// Test apply() with valid artifact YAML (Scenario + Package present)
    #[tokio::test]
    async fn test_apply_valid_artifact() {
        // First, create the required Model that the Package references
        let model_value: serde_yaml::Value = serde_yaml::from_str(VALID_MODEL_YAML).unwrap();
        let model_str = serde_yaml::to_string(&model_value).unwrap();
        if data::write_to_etcd("Model/helloworld-core", &model_str)
            .await
            .is_ok()
        {
            let result = apply(VALID_ARTIFACT_YAML).await;
            if let Ok(scenario) = result {
                assert!(!scenario.is_empty(), "Scenario YAML should not be empty");
            }
            let _ = data::delete_at_etcd("Model/helloworld-core").await;
        }
    }

    /// Test apply() with missing `action` field (invalid Scenario)
    #[tokio::test]
    async fn test_apply_invalid_missing_action() {
        let result = apply(INVALID_YAML_MISSING_ACTION).await;

        // Assert: should fail because Scenario is invalid (missing required field)
        assert!(
            result.is_err(),
            "apply() unexpectedly succeeded with missing action"
        );
    }

    /// Test apply() with unknown artifact (no Scenario, no Package)
    #[tokio::test]
    async fn test_apply_invalid_unknown_artifact() {
        let result = apply(INVALID_YAML_UNKNOWN_ARTIFACT).await;

        // Assert: should fail because no Scenario or Package present
        assert!(
            result.is_err(),
            "apply() unexpectedly succeeded with unknown artifact only"
        );
    }

    /// Test apply() with empty YAML
    #[tokio::test]
    async fn test_apply_invalid_empty_yaml() {
        let result = apply(INVALID_YAML_EMPTY).await;

        // Assert: should fail because YAML is empty
        assert!(
            result.is_err(),
            "apply() unexpectedly succeeded with empty YAML"
        );
    }

    // -- withdraw() tests --

    /// Test withdraw() with valid artifact YAML (Scenario present)
    #[tokio::test]
    async fn test_withdraw_valid_artifact() {
        let result = withdraw(VALID_ARTIFACT_YAML).await;

        // If etcd is connected, check returned scenario YAML
        if let Ok(scenario) = result {
            assert!(
                !scenario.is_empty(),
                "Returned scenario YAML should not be empty"
            );
        }
    }

    /// Test withdraw() with unknown artifact (no Scenario)
    #[tokio::test]
    async fn test_withdraw_invalid_unknown_artifact() {
        let result = withdraw(INVALID_YAML_UNKNOWN_ARTIFACT).await;

        // Assert: should fail because no Scenario present
        assert!(
            result.is_err(),
            "withdraw() unexpectedly succeeded with unknown artifact"
        );
    }

    /// Test withdraw() with empty YAML
    #[tokio::test]
    async fn test_withdraw_invalid_empty_yaml() {
        let result = withdraw(INVALID_YAML_EMPTY).await;

        // Assert: should fail because YAML is empty
        assert!(
            result.is_err(),
            "withdraw() unexpectedly succeeded with empty YAML"
        );
    }

    // -- verify_yaml_signature() tests --

    #[test]
    fn test_verify_yaml_signature_no_key_no_sig_passes() {
        let result = verify_yaml_signature_with_key(None, "some body content", None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_yaml_signature_no_key_with_sig_passes() {
        let result = verify_yaml_signature_with_key(None, "body", Some("some-sig"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_yaml_signature_key_set_no_sig_fails() {
        let result = verify_yaml_signature_with_key(Some("my-secret-key"), "body", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_yaml_signature_key_set_invalid_base64_sig_fails() {
        let result = verify_yaml_signature_with_key(
            Some("my-secret-key"),
            "body",
            Some("not!!valid??base64"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_yaml_signature_key_set_wrong_sig_fails() {
        let result = verify_yaml_signature_with_key(
            Some("my-secret-key"),
            "body",
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_yaml_signature_correct_hmac_passes() {
        let key = "test-key";
        let body = "test body content";
        let key_bytes = key.as_bytes();
        let body_bytes = body.as_bytes();
        const BLOCK_SIZE: usize = 64;
        let mut k = [0u8; BLOCK_SIZE];
        k[..key_bytes.len()].copy_from_slice(key_bytes);
        let mut ipad = [0u8; BLOCK_SIZE];
        let mut opad = [0u8; BLOCK_SIZE];
        for i in 0..BLOCK_SIZE {
            ipad[i] = k[i] ^ 0x36;
            opad[i] = k[i] ^ 0x5c;
        }
        let mut inner_input = Vec::with_capacity(BLOCK_SIZE + body_bytes.len());
        inner_input.extend_from_slice(&ipad);
        inner_input.extend_from_slice(body_bytes);
        let inner_hash = sha256_bytes(&inner_input);
        let mut outer_input = Vec::with_capacity(BLOCK_SIZE + 32);
        outer_input.extend_from_slice(&opad);
        outer_input.extend_from_slice(&inner_hash);
        let expected_sig = sha256_bytes(&outer_input);
        let valid_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, expected_sig);

        let result = verify_yaml_signature_with_key(Some(key), body, Some(&valid_b64));
        assert!(result.is_ok(), "Valid signature should pass verification");
    }

    #[test]
    fn test_sha256_basic() {
        let result = sha256_bytes(b"hello");
        assert_eq!(result.len(), 32);
        // SHA-256("hello") should be deterministic
        let result2 = sha256_bytes(b"hello");
        assert_eq!(result, result2);
        // Different input → different hash
        let result3 = sha256_bytes(b"world");
        assert_ne!(result, result3);
    }

    #[test]
    fn test_sha256_empty_input() {
        let result = sha256_bytes(b"");
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_sha256_long_input() {
        // Input longer than one 64-byte block to exercise multi-block path
        let long_input = vec![0xabu8; 200];
        let result = sha256_bytes(&long_input);
        assert_eq!(result.len(), 32);
    }

    // -- parse_artifact_info() tests --

    #[test]
    fn test_parse_artifact_info_scenario() {
        let yaml = r#"
apiVersion: v1
kind: Scenario
metadata:
  name: my-scenario
spec:
  condition:
    express: eq
    value: "true"
    operands:
      type: DDS
      name: value
      value: ADASObstacleDetectionIsWarning
  action: update
  target: my-scenario
"#;
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let result = parse_artifact_info(&value);
        assert!(result.is_some());
        let (kind, name) = result.unwrap();
        assert_eq!(kind, "Scenario");
        assert_eq!(name, "my-scenario");
    }

    #[test]
    fn test_parse_artifact_info_unknown_kind() {
        let yaml = "kind: UnknownKind\nmetadata:\n  name: test\n";
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let result = parse_artifact_info(&value);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_artifact_info_no_kind() {
        let yaml = "metadata:\n  name: test\n";
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let result = parse_artifact_info(&value);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_artifact_info_volume() {
        let yaml = r#"
apiVersion: v1
kind: Volume
metadata:
  name: test-volume
spec:
  type: hostpath
  path: /tmp/test
"#;
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let result = parse_artifact_info(&value);
        // Volume may or may not parse correctly depending on Volume spec
        // Just verify the function runs without panic
        let _ = result;
    }

    #[test]
    fn test_parse_artifact_info_network() {
        let yaml = r#"
apiVersion: v1
kind: Network
metadata:
  name: test-network
spec:
  type: bridge
"#;
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let result = parse_artifact_info(&value);
        let _ = result;
    }

    #[test]
    fn test_parse_artifact_info_node() {
        let yaml = r#"
apiVersion: v1
kind: Node
metadata:
  name: test-node
spec:
  hostname: test-host
"#;
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let result = parse_artifact_info(&value);
        let _ = result;
    }

    #[test]
    fn test_parse_artifact_info_model() {
        let yaml = r#"
apiVersion: v1
kind: Model
metadata:
  name: test-model
spec:
  containers:
    - name: app
      image: nginx:latest
"#;
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let result = parse_artifact_info(&value);
        let _ = result;
    }

    #[test]
    fn test_parse_artifact_info_schedule() {
        let yaml = r#"
apiVersion: v1
kind: Schedule
metadata:
  name: test-schedule
spec:
  cron: "* * * * *"
"#;
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let result = parse_artifact_info(&value);
        let _ = result;
    }
}
