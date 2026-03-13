use anyhow::Result;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone)]
pub struct ExternalToolResult {
    pub success: bool,
    pub output: String,
}

pub trait ExternalToolRunner: Send + Sync {
    fn web_fetch(&self, url: &str, max_chars: usize) -> Result<ExternalToolResult>;
    fn file_read(&self, path: &str) -> Result<ExternalToolResult>;
    fn file_write(&self, path: &str, content: &str, append: bool) -> Result<ExternalToolResult>;
}

static RUNNER: OnceLock<Arc<dyn ExternalToolRunner>> = OnceLock::new();

pub fn set_external_tool_runner(runner: Arc<dyn ExternalToolRunner>) -> Result<()> {
    if RUNNER.set(runner).is_err() {
        anyhow::bail!("external tool runner already set");
    }
    Ok(())
}

pub fn external_tool_runner() -> Option<Arc<dyn ExternalToolRunner>> {
    RUNNER.get().map(Arc::clone)
}
