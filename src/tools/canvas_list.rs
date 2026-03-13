use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use std::sync::Arc;

pub struct CanvasListTool {
    store: Arc<crate::canvas::CanvasStore>,
}

impl CanvasListTool {
    pub fn new(store: Arc<crate::canvas::CanvasStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for CanvasListTool {
    fn name(&self) -> &str {
        "canvas_list"
    }

    fn description(&self) -> &str {
        "List all canvas documents available in ~/.buzzclaw/canvas/. \
         Use canvas_read to read a specific canvas and canvas_write to create or update one."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn run(&self, _args: serde_json::Value) -> ToolResult {
        let names = self.store.list();
        if names.is_empty() {
            ToolResult::ok("No canvas documents yet.")
        } else {
            ToolResult::ok(names.join("\n"))
        }
    }
}
