use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[async_trait]
pub trait TaskHandler: Send + Sync {
    async fn execute(&self, input: serde_json::Value) -> Result<serde_json::Value>;
}

pub struct TaskQueue {
    handlers: Mutex<HashMap<String, Arc<dyn TaskHandler>>>,
    max_workers: usize,
}

impl TaskQueue {
    pub fn new(max_workers: usize) -> Self {
        Self {
            handlers: Mutex::new(HashMap::new()),
            max_workers,
        }
    }

    pub async fn register_handler(&self, capability_id: &str, handler: Arc<dyn TaskHandler>) {
        self.handlers.lock().await.insert(capability_id.to_string(), handler);
    }

    pub async fn get_handler(&self, capability_id: &str) -> Option<Arc<dyn TaskHandler>> {
        self.handlers.lock().await.get(capability_id).cloned()
    }

    pub fn max_workers(&self) -> usize {
        self.max_workers
    }
}
