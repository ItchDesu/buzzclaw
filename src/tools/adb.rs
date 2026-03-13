use super::traits::{Tool, ToolResult};
use crate::tools::external_cmd::{external_cmd_runner, ExternalCmdKind};
use async_trait::async_trait;

pub struct AdbTool;

#[async_trait]
impl Tool for AdbTool {
    fn name(&self) -> &str {
        "adb"
    }

    fn description(&self) -> &str {
        "Execute an ADB-like command via external runner (e.g., Shizuku)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The adb subcommand to execute."
                },
                "working_dir": {
                    "type": "string",
                    "description": "Optional working directory for the command."
                }
            },
            "required": ["command"]
        })
    }

    async fn run(&self, args: serde_json::Value) -> ToolResult {
        let command = match args.get("command").and_then(|v| v.as_str()) {
            Some(c) => c.to_string(),
            None => return ToolResult::err("missing 'command' argument"),
        };

        let working_dir = args
            .get("working_dir")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let Some(runner) = external_cmd_runner() else {
            return ToolResult::err("adb tool unavailable (no external runner configured)");
        };

        match runner.run(ExternalCmdKind::Adb, &command, working_dir.as_deref()) {
            Ok(res) => {
                if res.success {
                    ToolResult::ok(res.output)
                } else {
                    ToolResult::err(res.output)
                }
            }
            Err(e) => ToolResult::err(format!("external adb error: {e}")),
        }
    }
}
