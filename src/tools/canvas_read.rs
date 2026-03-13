use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use std::sync::Arc;

pub struct CanvasReadTool {
    store: Arc<crate::canvas::CanvasStore>,
}

impl CanvasReadTool {
    pub fn new(store: Arc<crate::canvas::CanvasStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for CanvasReadTool {
    fn name(&self) -> &str {
        "canvas_read"
    }

    fn description(&self) -> &str {
        "Read a canvas document by name. Canvas documents are persistent markdown artifacts \
         stored at ~/.buzzclaw/canvas/<name>.md. Use canvas_list to see available canvases."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Canvas document name (without .md extension)"
                }
            },
            "required": ["name"]
        })
    }

    async fn run(&self, args: serde_json::Value) -> ToolResult {
        let name = match args.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return ToolResult::err("missing 'name' argument"),
        };

        match self.store.get(name) {
            Some(canvas) => ToolResult::ok(canvas.content),
            None => ToolResult::err(format!("canvas '{name}' not found")),
        }
    }
}
