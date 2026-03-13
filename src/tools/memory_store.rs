use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use std::sync::Arc;

pub struct MemoryStoreTool {
    memory: Arc<crate::memory::MemoryStore>,
}

impl MemoryStoreTool {
    pub fn new(memory: Arc<crate::memory::MemoryStore>) -> Self {
        Self { memory }
    }
}

#[async_trait]
impl Tool for MemoryStoreTool {
    fn name(&self) -> &str {
        "memory_store"
    }

    fn description(&self) -> &str {
        "Persist a key-value memory for later retrieval. \
         Use to remember facts, user preferences, or important information."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "key": {
                    "type": "string",
                    "description": "A short descriptive key (e.g. 'user_name', 'project_goal')."
                },
                "value": {
                    "type": "string",
                    "description": "The content to store."
                },
                "category": {
                    "type": "string",
                    "description": "Optional category tag (e.g. 'core', 'session'). Default: 'general'."
                }
            },
            "required": ["key", "value"]
        })
    }

    async fn run(&self, args: serde_json::Value) -> ToolResult {
        let key = match args.get("key").and_then(|v| v.as_str()) {
            Some(k) => k.to_string(),
            None => return ToolResult::err("missing 'key' argument"),
        };
        let value = match args.get("value").and_then(|v| v.as_str()) {
            Some(v) => v.to_string(),
            None => return ToolResult::err("missing 'value' argument"),
        };
        let category = args
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("general")
            .to_string();

        match self.memory.store(&key, &value, &category) {
            Ok(_) => ToolResult::ok(format!("Stored memory: {key}")),
            Err(e) => ToolResult::err(format!("memory store error: {e}")),
        }
    }
}
