use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use std::sync::Arc;

pub struct WorkspaceReadTool {
    workspace: Arc<crate::workspace::Workspace>,
}

impl WorkspaceReadTool {
    pub fn new(workspace: Arc<crate::workspace::Workspace>) -> Self {
        Self { workspace }
    }
}

#[async_trait]
impl Tool for WorkspaceReadTool {
    fn name(&self) -> &str {
        "workspace_read"
    }

    fn description(&self) -> &str {
        "Read a workspace markdown file. Use to inspect MEMORY.md, SOUL.md, USER.md, \
         HEARTBEAT.md, or any daily log (memory/YYYY-MM-DD.md). \
         Special value 'today' reads today's daily log."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Workspace-relative path, e.g. 'MEMORY.md', 'today', 'memory/2026-03-11.md'."
                }
            },
            "required": ["file"]
        })
    }

    async fn run(&self, args: serde_json::Value) -> ToolResult {
        let file = match args.get("file").and_then(|v| v.as_str()) {
            Some(f) => f.to_string(),
            None => return ToolResult::err("missing 'file' argument"),
        };

        match self.workspace.read(&file) {
            Some(content) => ToolResult::ok(content),
            None => ToolResult::ok(format!("{file} is empty or does not exist.")),
        }
    }
}
