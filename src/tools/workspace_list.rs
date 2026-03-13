use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct WorkspaceListTool {
    workspace: Arc<crate::workspace::Workspace>,
    canvas_dir: PathBuf,
}

impl WorkspaceListTool {
    pub fn new(workspace: Arc<crate::workspace::Workspace>, canvas_dir: PathBuf) -> Self {
        Self { workspace, canvas_dir }
    }
}

#[async_trait]
impl Tool for WorkspaceListTool {
    fn name(&self) -> &str {
        "workspace_list"
    }

    fn description(&self) -> &str {
        "List workspace files (core + daily logs) and canvas files. Returns JSON with file names and paths."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn run(&self, _args: serde_json::Value) -> ToolResult {
        let core = [
            "AGENTS.md",
            "SOUL.md",
            "USER.md",
            "MEMORY.md",
            "HEARTBEAT.md",
        ];

        let mut core_files = Vec::new();
        for name in core {
            let path = self.workspace.dir.join(name);
            if path.exists() {
                core_files.push(file_entry(name, name, &path));
            }
        }

        let mut daily_logs = Vec::new();
        for name in self.workspace.memory_files() {
            let rel = format!("memory/{name}");
            let path = self.workspace.dir.join(&rel);
            daily_logs.push(file_entry(&name, &rel, &path));
        }

        let mut canvas_files = Vec::new();
        if self.canvas_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&self.canvas_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("md") {
                        if let Some(file_name) = p.file_name().and_then(|n| n.to_str()) {
                            let rel = format!("canvas/{file_name}");
                            canvas_files.push(file_entry(file_name, &rel, &p));
                        }
                    }
                }
            }
        }

        let out = serde_json::json!({
            "core_files": core_files,
            "daily_logs": daily_logs,
            "canvas_files": canvas_files,
        });

        ToolResult::ok(out.to_string())
    }
}

fn file_entry(name: &str, rel_path: &str, path: &Path) -> serde_json::Value {
    let modified_unix = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());

    serde_json::json!({
        "name": name,
        "path": rel_path,
        "modified_unix": modified_unix,
    })
}
