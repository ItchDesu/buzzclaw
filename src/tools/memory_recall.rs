use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use std::sync::Arc;

pub struct MemoryRecallTool {
    memory: Arc<crate::memory::MemoryStore>,
}

impl MemoryRecallTool {
    pub fn new(memory: Arc<crate::memory::MemoryStore>) -> Self {
        Self { memory }
    }
}

#[async_trait]
impl Tool for MemoryRecallTool {
    fn name(&self) -> &str {
        "memory_recall"
    }

    fn description(&self) -> &str {
        "Search and retrieve stored memories. \
         Use to look up facts, past interactions, or stored information."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search term or keyword to look up in memory."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum results to return. Default: 10."
                }
            },
            "required": ["query"]
        })
    }

    async fn run(&self, args: serde_json::Value) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q.to_string(),
            None => return ToolResult::err("missing 'query' argument"),
        };
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        match self.memory.search(&query, limit) {
            Ok(entries) if entries.is_empty() => {
                ToolResult::ok("No memories found matching that query.")
            }
            Ok(entries) => {
                let text = entries
                    .into_iter()
                    .map(|e| format!("[{}] {}: {}", e.created_at.format("%Y-%m-%d"), e.key, e.value))
                    .collect::<Vec<_>>()
                    .join("\n");
                ToolResult::ok(text)
            }
            Err(e) => ToolResult::err(format!("memory recall error: {e}")),
        }
    }
}
