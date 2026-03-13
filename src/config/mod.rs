use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Provider label used by the Android model/router
    pub provider: String,

    /// API key used by the Android model/router (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Model to use (provider-specific)
    pub model: String,

    /// Sampling temperature (0.0 – 2.0)
    pub temperature: f64,

    /// Max tool-call iterations per agent turn
    pub max_tool_iterations: usize,

    /// Max messages in context before compaction
    pub max_history_messages: usize,

    /// Whether to enable tool use
    pub tools_enabled: bool,

    /// Enable debug logging
    pub debug: bool,

    /// System prompt injected before every conversation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: "deepseek".to_string(),
            api_key: None,
            model: "deepseek-chat".to_string(),
            temperature: 0.7,
            max_tool_iterations: 10,
            max_history_messages: 50,
            tools_enabled: true,
            debug: false,
            system_prompt: None,
        }
    }
}

impl Config {
    /// Load config from disk, falling back to defaults.
    pub fn load() -> Result<Self> {
        let path = config_path();

        if !path.exists() {
            return Ok(Self::default());
        }

        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config at {}", path.display()))?;
        let cfg: Self = toml::from_str(&text)
            .with_context(|| format!("parsing config at {}", path.display()))?;
        Ok(cfg)
    }

    /// Merge environment-variable overrides on top of file config.
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(model) = std::env::var("RUSTCLAW_MODEL") {
            if !model.is_empty() {
                self.model = model;
            }
        }
        self
    }

    /// Save config to disk.
    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(&path, text)?;
        Ok(())
    }

    // API keys are not used in local mode.
}

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Override the BuzzClaw data directory (useful for Android/JNI).
pub fn set_data_dir(path: impl Into<PathBuf>) -> Result<()> {
    let path = path.into();
    if path.as_os_str().is_empty() {
        anyhow::bail!("data dir cannot be empty");
    }
    if DATA_DIR.set(path).is_err() {
        anyhow::bail!("data dir already set");
    }
    Ok(())
}

fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".buzzclaw")
}

fn data_root() -> PathBuf {
    if let Some(dir) = DATA_DIR.get() {
        return dir.clone();
    }

    if let Ok(val) = std::env::var("BUZZCLAW_HOME").or_else(|_| std::env::var("RUSTCLAW_HOME")) {
        if !val.trim().is_empty() {
            return PathBuf::from(val);
        }
    }

    home_dir()
}

pub fn config_path() -> PathBuf {
    data_root().join("config.toml")
}

pub fn data_dir() -> PathBuf {
    data_root()
}

pub fn skills_dir() -> PathBuf {
    data_root().join("skills")
}

pub fn workspace_dir() -> PathBuf {
    data_root().join("workspace")
}

pub fn sessions_dir() -> PathBuf {
    data_root().join("sessions")
}

pub fn canvas_dir() -> PathBuf {
    data_root().join("canvas")
}

pub fn cron_dir() -> PathBuf {
    data_root().join("cron")
}
