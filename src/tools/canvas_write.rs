use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use std::sync::Arc;

pub struct CanvasWriteTool {
    store: Arc<crate::canvas::CanvasStore>,
}

impl CanvasWriteTool {
    pub fn new(store: Arc<crate::canvas::CanvasStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for CanvasWriteTool {
    fn name(&self) -> &str {
        "canvas_write"
    }

    fn description(&self) -> &str {
        "Write or append to a canvas document. Canvas documents are persistent markdown artifacts \
         stored at ~/.buzzclaw/canvas/<name>.md. Great for drafts, reports, code, or any artifact \
         that the user wants to keep and edit."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Canvas document name (without .md extension)"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the canvas"
                },
                "append": {
                    "type": "boolean",
                    "description": "Append to existing content instead of overwriting. Default: false."
                }
            },
            "required": ["name", "content"]
        })
    }

    async fn run(&self, args: serde_json::Value) -> ToolResult {
        let name = match args.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return ToolResult::err("missing 'name' argument"),
        };
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::err("missing 'content' argument"),
        };
        let append = args.get("append").and_then(|v| v.as_bool()).unwrap_or(false);

        let result = if append {
            self.store.append(name, content)
        } else {
            self.store.write(name, content)
        };

        match result {
            Ok(_) => ToolResult::ok(format!("Canvas '{name}' saved")),
            Err(e) => ToolResult::err(format!("canvas_write error: {e}")),
        }
    }
}
