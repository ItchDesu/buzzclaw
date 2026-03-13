use super::traits::{Tool, ToolResult};
use crate::tools::external_cmd::{external_cmd_runner, ExternalCmdKind};
use async_trait::async_trait;
use std::time::Duration;

pub struct ShellTool;

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return stdout + stderr. \
         Use for running scripts, checking system state, or any OS-level operation."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute."
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

        if let Some(runner) = external_cmd_runner() {
            match runner.run(ExternalCmdKind::Shell, &command, working_dir.as_deref()) {
                Ok(res) => {
                    if res.success {
                        return ToolResult::ok(res.output);
                    }
                    return ToolResult::err(res.output);
                }
                Err(e) => return ToolResult::err(format!("external shell error: {e}")),
            }
        }

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = tokio::process::Command::new("cmd");
            c.args(["/C", &command]);
            c
        } else {
            let mut c = tokio::process::Command::new("sh");
            c.args(["-c", &command]);
            c
        };

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let output = tokio::time::timeout(Duration::from_secs(30), cmd.output()).await;
        match output {
            Err(_) => return ToolResult::err("command timed out"),
            Ok(Err(e)) => return ToolResult::err(format!("failed to spawn command: {e}")),
            Ok(Ok(out)) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let mut combined = if stderr.is_empty() {
                    stdout.to_string()
                } else {
                    format!("{stdout}\n[stderr]\n{stderr}")
                };
                let max = 64 * 1024;
                if combined.len() > max {
                    combined.truncate(max);
                    combined.push_str("\n…[truncated]");
                }
                let success = out.status.success();
                if success {
                    ToolResult::ok(combined)
                } else {
                    ToolResult::err(combined)
                }
            }
        }
    }
}
