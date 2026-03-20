use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K2KNodeConfig {
    /// Node identity
    #[serde(default = "default_node_name")]
    pub node_name: String,
    #[serde(default)]
    pub node_id: Option<String>,

    /// Server settings
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,

    /// Data directory (SQLite DB, keys, model cache)
    #[serde(default = "default_data_dir")]
    pub data_dir: String,

    /// Paths to index
    #[serde(default)]
    pub index_paths: Vec<String>,

    /// Security
    #[serde(default)]
    pub registration_secret: Option<String>,
    #[serde(default)]
    pub allowed_clients: Vec<String>,
    #[serde(default = "default_auto_approve")]
    pub auto_approve: bool,

    /// Discovery
    #[serde(default = "default_mdns_enabled")]
    pub mdns_enabled: bool,
    #[serde(default)]
    pub trusted_node_ids: Vec<String>,
    #[serde(default)]
    pub bootstrap_nodes: Vec<String>,

    /// Embedding model
    #[serde(default = "default_model_name")]
    pub embedding_model: String,

    /// Rate limiting
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_minute: u32,

    /// Task settings
    #[serde(default = "default_max_workers")]
    pub max_task_workers: usize,
}

fn default_node_name() -> String { hostname::get().map(|h| h.to_string_lossy().to_string()).unwrap_or_else(|_| "k2k-node".to_string()) }
fn default_host() -> String { "127.0.0.1".to_string() }
fn default_port() -> u16 { 19850 }
fn default_data_dir() -> String { dirs_data_dir() }
fn default_auto_approve() -> bool { false }
fn default_mdns_enabled() -> bool { true }
fn default_model_name() -> String { "all-MiniLM-L6-v2".to_string() }
fn default_rate_limit() -> u32 { 60 }
fn default_max_workers() -> usize { 4 }

fn dirs_data_dir() -> String {
    dirs::data_local_dir()
        .map(|p| p.join("k2k-node").to_string_lossy().to_string())
        .unwrap_or_else(|| ".k2k-node".to_string())
}

impl K2KNodeConfig {
    pub fn load(path: &str) -> Result<Self> {
        if Path::new(path).exists() {
            let contents = std::fs::read_to_string(path)?;
            let config: Self = serde_yaml::from_str(&contents)?;
            Ok(config)
        } else {
            tracing::info!("Config file not found at '{}', using defaults", path);
            Ok(Self::default())
        }
    }

    pub fn db_path(&self) -> String {
        format!("{}/k2k-node.db", self.data_dir)
    }

    pub fn keys_dir(&self) -> String {
        format!("{}/keys", self.data_dir)
    }

    pub fn models_dir(&self) -> String {
        format!("{}/models", self.data_dir)
    }
}

impl Default for K2KNodeConfig {
    fn default() -> Self {
        Self {
            node_name: default_node_name(),
            node_id: None,
            host: default_host(),
            port: default_port(),
            data_dir: default_data_dir(),
            index_paths: Vec::new(),
            registration_secret: None,
            allowed_clients: Vec::new(),
            auto_approve: default_auto_approve(),
            mdns_enabled: default_mdns_enabled(),
            trusted_node_ids: Vec::new(),
            bootstrap_nodes: Vec::new(),
            embedding_model: default_model_name(),
            rate_limit_per_minute: default_rate_limit(),
            max_task_workers: default_max_workers(),
        }
    }
}
