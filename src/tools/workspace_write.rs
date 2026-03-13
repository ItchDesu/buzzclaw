use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use std::sync::Arc;

pub struct WorkspaceWriteTool {
    workspace: Arc<crate::workspace::Workspace>,
}

impl WorkspaceWriteTool {
    pub fn new(workspace: Arc<crate::workspace::Workspace>) -> Self {
        Self { workspace }
    }
}

#[async_trait]
impl Tool for WorkspaceWriteTool {
    fn name(&self) -> &str {
        "workspace_write"
    }

    fn description(&self) -> &str {
        "Write or append content to a workspace markdown file. \
         Use 'MEMORY.md' for long-term facts, 'today' for today's daily log, \
         'SOUL.md' for identity rules, or 'USER.md' for user preferences."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Workspace-relative path. Special value 'today' writes to memory/YYYY-MM-DD.md."
                },
                "content": {
                    "type": "string",
                    "description": "Content to write."
                },
                "append": {
                    "type": "boolean",
                    "description": "Append to existing content instead of overwriting. Default: false."
                }
            },
            "required": ["file", "content"]
        })
    }

    async fn run(&self, args: serde_json::Value) -> ToolResult {
        let file = match args.get("file").and_then(|v| v.as_str()) {
            Some(f) => f.to_string(),
            None => return ToolResult::err("missing 'file' argument"),
        };
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c.to_string(),
            None => return ToolResult::err("missing 'content' argument"),
        };
        let append = args.get("append").and_then(|v| v.as_bool()).unwrap_or(false);

        let result = if append {
            self.workspace.append(&file, &content)
        } else {
            self.workspace.write(&file, &content)
        };

        match result {
            Ok(_) => ToolResult::ok(format!("Written to {file}")),
            Err(e) => ToolResult::err(format!("workspace_write error: {e}")),
        }
    }
}
