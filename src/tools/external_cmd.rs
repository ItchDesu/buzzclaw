use anyhow::Result;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, Copy)]
pub enum ExternalCmdKind {
    Shell,
    Adb,
}

#[derive(Debug, Clone)]
pub struct ExternalCmdResult {
    pub success: bool,
    pub output: String,
}

pub trait ExternalCmdRunner: Send + Sync {
    fn run(&self, kind: ExternalCmdKind, command: &str, working_dir: Option<&str>) -> Result<ExternalCmdResult>;
}

static RUNNER: OnceLock<Arc<dyn ExternalCmdRunner>> = OnceLock::new();

pub fn set_external_cmd_runner(runner: Arc<dyn ExternalCmdRunner>) -> Result<()> {
    if RUNNER.set(runner).is_err() {
        anyhow::bail!("external command runner already set");
    }
    Ok(())
}

pub fn external_cmd_runner() -> Option<Arc<dyn ExternalCmdRunner>> {
    RUNNER.get().map(Arc::clone)
}
