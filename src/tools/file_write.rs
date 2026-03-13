use super::traits::{Tool, ToolResult};
use crate::tools::external_tools::external_tool_runner;
use async_trait::async_trait;
use std::path::Path;

pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Write or overwrite a file with the given content. \
         Parent directories are created automatically."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to write the file to."
                },
                "content": {
                    "type": "string",
                    "description": "Content to write."
                },
                "append": {
                    "type": "boolean",
                    "description": "If true, append instead of overwrite. Default: false."
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn run(&self, args: serde_json::Value) -> ToolResult {
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolResult::err("missing 'path' argument"),
        };
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c.to_string(),
            None => return ToolResult::err("missing 'content' argument"),
        };
        let append = args
            .get("append")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if let Some(runner) = external_tool_runner() {
            return match runner.file_write(&path, &content, append) {
                Ok(res) => {
                    if res.success { ToolResult::ok(res.output) } else { ToolResult::err(res.output) }
                }
                Err(e) => ToolResult::err(format!("external file_write error: {e}")),
            };
        }

        // Create parent dirs
        if let Some(parent) = Path::new(&path).parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return ToolResult::err(format!("cannot create directories: {e}"));
            }
        }

        let result = if append {
            use tokio::io::AsyncWriteExt;
            match tokio::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&path)
                .await
            {
                Ok(mut f) => f.write_all(content.as_bytes()).await,
                Err(e) => return ToolResult::err(format!("cannot open file: {e}")),
            }
        } else {
            tokio::fs::write(&path, &content).await
        };

        match result {
            Ok(_) => ToolResult::ok(format!("written {} bytes to {path}", content.len())),
            Err(e) => ToolResult::err(format!("write failed: {e}")),
        }
    }
}
