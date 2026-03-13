use super::traits::{Tool, ToolResult};
use crate::tools::external_tools::external_tool_runner;
use async_trait::async_trait;
use std::path::Path;

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Returns the text content of the file at the given path."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file."
                },
                "start_line": {
                    "type": "integer",
                    "description": "Optional 1-based line to start reading from."
                },
                "end_line": {
                    "type": "integer",
                    "description": "Optional 1-based line to stop reading at (inclusive)."
                }
            },
            "required": ["path"]
        })
    }

    async fn run(&self, args: serde_json::Value) -> ToolResult {
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolResult::err("missing 'path' argument"),
        };

        if let Some(runner) = external_tool_runner() {
            return match runner.file_read(&path) {
                Ok(res) => {
                    if res.success { ToolResult::ok(res.output) } else { ToolResult::err(res.output) }
                }
                Err(e) => ToolResult::err(format!("external file_read error: {e}")),
            };
        }

        if !Path::new(&path).exists() {
            return ToolResult::err(format!("file not found: {path}"));
        }

        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("cannot read file: {e}")),
        };

        let start = args
            .get("start_line")
            .and_then(|v| v.as_u64())
            .map(|n| n.saturating_sub(1) as usize)
            .unwrap_or(0);
        let end = args
            .get("end_line")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);

        let result: String = content
            .lines()
            .enumerate()
            .filter(|(i, _)| *i >= start && end.map(|e| *i < e).unwrap_or(true))
            .map(|(i, line)| format!("{:>6} {line}", i + 1))
            .collect::<Vec<_>>()
            .join("\n");

        ToolResult::ok(result)
    }
}
