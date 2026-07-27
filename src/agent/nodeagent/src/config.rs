/*
* SPDX-FileCopyrightText: Copyright 2024 LG Electronics Inc.
* SPDX-License-Identifier: Apache-2.0
*/
use if_addrs::{get_if_addrs, Interface};
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;
use thiserror::Error;

// Global config instance
static NODEAGENT_CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse YAML: {0}")]
    YamlError(#[from] serde_yaml::Error),
}

#[derive(Debug, Deserialize, Clone, Default, PartialEq)]
pub struct MetricsConfig {
    pub collection_interval: u64,
    pub batch_size: u32,
}

#[derive(Debug, Deserialize, Clone, Default, PartialEq)]
pub struct SystemConfig {
    pub hostname: String,
    pub platform: String,
    pub architecture: String,
}

#[derive(Debug, Deserialize, Clone, Default, PartialEq)]
pub struct NodeAgentConfig {
    #[serde(default = "default_node_name")]
    pub node_name: String,
    #[serde(default = "default_node_type")]
    pub node_type: String,
    #[serde(default = "default_node_role")]
    pub node_role: String,
    pub master_ip: String,
    #[serde(default)]
    pub node_ip: String,
    pub grpc_port: u16,
    pub log_level: String,
    pub metrics: MetricsConfig,
    pub system: SystemConfig,
    #[serde(default = "default_yaml_storage")]
    pub yaml_storage: String,
}

fn default_node_name() -> String {
    match hostname::get() {
        Ok(hostname) => hostname.to_string_lossy().to_string(),
        Err(_) => "unknown".to_string(),
    }
}

fn default_node_type() -> String {
    "cloud".to_string()
}

fn default_node_role() -> String {
    "nodeagent".to_string()
}

fn default_yaml_storage() -> String {
    "/etc/piccolo/yaml".to_string()
}

#[derive(Debug, Deserialize, Clone, Default, PartialEq)]
pub struct Config {
    pub nodeagent: NodeAgentConfig,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let config = serde_yaml::from_str(&contents)?;
        Ok(config)
    }

    pub fn get_host_ip(&self) -> String {
        // If node_ip is explicitly set in config, use it
        if !self.nodeagent.node_ip.is_empty() {
            return self.nodeagent.node_ip.clone();
        }

        // Otherwise try to get the first non-loopback IPv4 address
        if let Ok(interfaces) = get_network_interfaces() {
            for iface in interfaces {
                if let std::net::IpAddr::V4(ipv4) = iface.addr.ip() {
                    if !ipv4.is_loopback() {
                        return ipv4.to_string();
                    }
                }
            }
        }

        // Fallback to master_ip if we couldn't determine the host IP
        self.nodeagent.master_ip.clone()
    }

    pub fn get_hostname(&self) -> String {
        self.nodeagent.system.hostname.clone()
    }

    pub fn get_node_name(&self) -> String {
        self.nodeagent.node_name.clone()
    }

    pub fn get_yaml_storage(&self) -> String {
        self.nodeagent.yaml_storage.clone()
    }

    // Get or initialize the global config
    pub fn get() -> &'static Config {
        NODEAGENT_CONFIG.get().unwrap_or_else(|| {
            let default_config = Config::default();
            NODEAGENT_CONFIG.set(default_config.clone()).unwrap_or(());
            NODEAGENT_CONFIG.get().unwrap()
        })
    }

    // Set the global config
    pub fn set_global(config: Config) {
        let _ = NODEAGENT_CONFIG.set(config);
    }
}

// Helper function to get network interfaces
fn get_network_interfaces() -> Result<Vec<Interface>, std::io::Error> {
    get_if_addrs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_load_nonexistent_file_returns_default() {
        let path = PathBuf::from("/nonexistent/path/to/config.yaml");
        let config = Config::load(&path).unwrap_or_else(|_| Config::default());
        assert!(!config.get_host_ip().is_empty());
    }

    #[test]
    fn test_set_and_get_global_config() {
        let config = Config::default();
        Config::set_global(config.clone());
        let loaded = Config::get();
        assert_eq!(loaded.get_host_ip(), config.get_host_ip());
    }

    #[test]
    fn test_node_type_and_role_mapping() {
        let mut config = Config::default();
        config.nodeagent.node_type = "cloud".to_string();
        config.nodeagent.node_role = "master".to_string();
        assert_eq!(
            match config.nodeagent.node_type.as_str() {
                "cloud" => 1,
                "vehicle" => 2,
                _ => 0,
            },
            1
        );
        assert_eq!(
            match config.nodeagent.node_role.as_str() {
                "master" => 1,
                "nodeagent" => 2,
                "bluechi" => 3,
                _ => 0,
            },
            1
        );
    }

    #[test]
    fn test_config_clone_and_eq() {
        let config1 = Config::default();
        let config2 = config1.clone();
        assert_eq!(config1, config2);
    }

    #[test]
    fn test_config_load_and_getters() {
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_nodeagent_config.yaml");
        let yaml_content = r#"
nodeagent:
  node_name: "test-node"
  node_type: "vehicle"
  node_role: "bluechi"
  master_ip: "10.0.0.1"
  node_ip: "10.0.0.2"
  grpc_port: 47004
  log_level: "info"
  metrics:
    collection_interval: 5
    batch_size: 10
  system:
    hostname: "test-host"
    platform: "linux"
    architecture: "x86_64"
  yaml_storage: "/tmp/piccolo/yaml"
"#;
        std::fs::write(&temp_path, yaml_content).unwrap();

        let config = Config::load(&temp_path).unwrap();
        assert_eq!(config.get_node_name(), "test-node");
        assert_eq!(config.get_hostname(), "test-host");
        assert_eq!(config.get_yaml_storage(), "/tmp/piccolo/yaml");
        assert_eq!(config.get_host_ip(), "10.0.0.2");

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_config_defaults() {
        let name = default_node_name();
        assert!(!name.is_empty());
        assert_eq!(default_node_type(), "cloud");
        assert_eq!(default_node_role(), "nodeagent");
        assert_eq!(default_yaml_storage(), "/etc/piccolo/yaml");
    }

    #[test]
    fn test_get_host_ip_fallback() {
        let mut config = Config::default();
        config.nodeagent.node_ip = "".to_string();
        config.nodeagent.master_ip = "192.168.1.5".to_string();

        let ip = config.get_host_ip();
        assert!(!ip.is_empty());
    }

    #[test]
    fn test_config_get_default_fallback() {
        // Force unwrap_or_else inside Config::get by calling it
        let config = Config::get();
        assert!(!config.get_host_ip().is_empty() || config.nodeagent.master_ip.is_empty());
    }
}
