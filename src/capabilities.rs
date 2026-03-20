use k2k_common::{AgentCapability, CapabilityCategory};
use std::sync::RwLock;

pub struct CapabilityRegistry {
    capabilities: RwLock<Vec<AgentCapability>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        let mut caps = Vec::new();

        caps.push(AgentCapability {
            id: "semantic_search".to_string(),
            name: "Semantic Search".to_string(),
            category: CapabilityCategory::Knowledge,
            description: "Search indexed files using semantic similarity".to_string(),
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "top_k": { "type": "integer", "default": 10 }
                },
                "required": ["query"]
            })),
            version: "1.0.0".to_string(),
            min_protocol_version: None,
        });

        Self {
            capabilities: RwLock::new(caps),
        }
    }

    pub fn list(&self) -> Vec<AgentCapability> {
        self.capabilities.read().unwrap().clone()
    }

    pub fn list_ids(&self) -> Vec<String> {
        self.capabilities.read().unwrap().iter().map(|c| c.id.clone()).collect()
    }

    pub fn register(&self, capability: AgentCapability) {
        self.capabilities.write().unwrap().push(capability);
    }
}
