use crate::{agent, canvas, config, memory, providers, runtime, sessions, tools, workspace};
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct AppOptions {
    pub tools_enabled: bool,
    pub tool_profile: runtime::ToolProfile,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self { tools_enabled: true, tool_profile: runtime::ToolProfile::NoSpawn }
    }
}

pub struct BuzzClawApp {
    cfg: config::Config,
    provider: Arc<dyn providers::Provider>,
    tools: Arc<tools::ToolRegistry>,
    _memory: Arc<memory::MemoryStore>,
    workspace: Arc<workspace::Workspace>,
    sessions: sessions::SessionStore,
    canvas: Arc<canvas::CanvasStore>,
    rt: tokio::runtime::Runtime,
}

impl BuzzClawApp {
    /// Open BuzzClaw using the on-disk config + env overrides.
    pub fn open_default(options: AppOptions) -> Result<Self> {
        let cfg = config::Config::load()?.with_env_overrides();
        Self::open_with_config(cfg, options)
    }

    /// Open BuzzClaw with a provided config (useful for Android).
    pub fn open_with_config(cfg: config::Config, options: AppOptions) -> Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;

        let provider: Arc<dyn providers::Provider> =
            runtime::build_provider(&cfg.provider, cfg.api_key.clone());

        // Memory
        let db_path = config::data_dir().join("memory.db");
        let memory = Arc::new(memory::MemoryStore::open(&db_path)?);

        // Workspace
        let workspace = Arc::new(workspace::Workspace::open(&config::workspace_dir())?);

        // Canvas
        let canvas = Arc::new(canvas::CanvasStore::open(&config::canvas_dir())?);

        // Tools
        let registry = if options.tools_enabled && cfg.tools_enabled {
            runtime::build_tool_registry(
                options.tool_profile,
                &cfg,
                Arc::clone(&memory),
                Arc::clone(&workspace),
                Some(Arc::clone(&canvas)),
            )
        } else {
            tools::ToolRegistry::new()
        };

        // Sessions
        let sessions = sessions::SessionStore::open(&config::sessions_dir())?;

        Ok(Self {
            cfg,
            provider,
            tools: Arc::new(registry),
            _memory: memory,
            workspace,
            sessions,
            canvas,
            rt,
        })
    }

    /// Run a single turn in a named session (creates if missing).
    pub fn send_message(&self, session: &str, message: &str) -> Result<String> {
        let provider = self.cfg.provider.trim().to_ascii_lowercase();
        if provider != "local" {
            let key = self.cfg.api_key.as_deref().unwrap_or("");
            if key.trim().is_empty() {
                anyhow::bail!("api_key is required for provider '{}'", self.cfg.provider);
            }
        }
        let mut history = self.sessions.load(session);
        let agent = self.build_agent();
        let answer = self.rt.block_on(agent.run_turn(&mut history, message))?;
        self.sessions.save(session, &history)?;
        Ok(answer)
    }

    pub fn reset_session(&self, session: &str) -> Result<bool> {
        self.sessions.delete(session)
    }

    pub fn list_sessions(&self) -> Vec<String> {
        self.sessions.list()
    }

    pub fn config(&self) -> &config::Config {
        &self.cfg
    }

    pub fn config_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self.cfg)?)
    }

    pub fn set_config_json(&mut self, json: &str) -> Result<()> {
        let cfg: config::Config = serde_json::from_str(json)?;
        self.cfg = cfg;
        self.cfg.save()?;
        self.rebuild_provider()?;
        Ok(())
    }

    pub fn set_model(&mut self, model: &str) -> Result<()> {
        self.cfg.model = model.to_string();
        self.cfg.save()?;
        Ok(())
    }

    pub fn set_provider(&mut self, provider: &str) -> Result<()> {
        self.cfg.provider = provider.to_string();
        self.cfg.save()?;
        self.rebuild_provider()?;
        Ok(())
    }

    pub fn set_api_key(&mut self, api_key: Option<&str>) -> Result<()> {
        self.cfg.api_key = api_key.map(|s| s.to_string()).filter(|s| !s.is_empty());
        self.cfg.save()?;
        self.rebuild_provider()?;
        Ok(())
    }

    pub fn set_system_prompt(&mut self, prompt: Option<&str>) -> Result<()> {
        self.cfg.system_prompt = prompt.map(|s| s.to_string());
        self.cfg.save()?;
        Ok(())
    }

    pub fn set_temperature(&mut self, temperature: f64) -> Result<()> {
        self.cfg.temperature = temperature;
        self.cfg.save()?;
        Ok(())
    }

    pub fn set_tools_enabled(&mut self, enabled: bool) -> Result<()> {
        self.cfg.tools_enabled = enabled;
        self.cfg.save()?;
        Ok(())
    }

    fn rebuild_provider(&mut self) -> Result<()> {
        self.provider = runtime::build_provider(&self.cfg.provider, self.cfg.api_key.clone());
        Ok(())
    }

    pub fn workspace_list_json(&self) -> Result<String> {
        let core = ["AGENTS.md", "SOUL.md", "USER.md", "MEMORY.md", "HEARTBEAT.md"];
        let mut core_files = Vec::new();
        for name in core {
            let path = self.workspace.dir.join(name);
            if path.exists() {
                core_files.push(name);
            }
        }
        let daily_logs = self.workspace.memory_files();
        let mut canvas_files = Vec::new();
        for name in self.canvas.list() {
            canvas_files.push(format!("{name}.md"));
        }
        let out = serde_json::json!({
            "core_files": core_files,
            "daily_logs": daily_logs,
            "canvas_files": canvas_files,
        });
        Ok(out.to_string())
    }

    pub fn workspace_read(&self, file: &str) -> Result<String> {
        Ok(self.workspace.read(file).unwrap_or_default())
    }

    pub fn workspace_write(&self, file: &str, content: &str, append: bool) -> Result<()> {
        if append {
            self.workspace.append(file, content)?;
        } else {
            self.workspace.write(file, content)?;
        }
        Ok(())
    }

    pub fn canvas_read(&self, name: &str) -> Result<String> {
        Ok(self.canvas.get(name).map(|c| c.content).unwrap_or_default())
    }

    pub fn canvas_write(&self, name: &str, content: &str, append: bool) -> Result<()> {
        if append {
            self.canvas.append(name, content)?;
        } else {
            self.canvas.write(name, content)?;
        }
        Ok(())
    }

    pub fn session_history_json(&self, session: &str) -> Result<String> {
        let mut history = self.sessions.load(session);
        
        // Filter out embedded JSON tool_calls from assistant messages
        for msg in history.iter_mut() {
            if msg.role == "assistant" {
                let trimmed = msg.content.trim();
                if trimmed.starts_with('{') && trimmed.contains("\"tool_calls\"") {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
                        if parsed.get("tool_calls").is_some() {
                            let extracted = parsed.get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            msg.content = extracted.to_string();
                        }
                    }
                }
            }
        }
        
        Ok(serde_json::to_string(&history)?)
    }

    pub fn set_session_history_json(&self, session: &str, json: &str) -> Result<()> {
        let history: Vec<crate::providers::ChatMessage> = serde_json::from_str(json)?;
        self.sessions.save(session, &history)?;
        Ok(())
    }

    fn build_agent(&self) -> agent::AgentLoop {
        // Merge workspace context + user system_prompt
        let ws_ctx = self.workspace.build_context();
        let system_prompt = match (ws_ctx.as_str(), self.cfg.system_prompt.as_deref()) {
            ("", None)       => None,
            ("", Some(sp))   => Some(sp.to_string()),
            (ctx, None)      => Some(ctx.to_string()),
            (ctx, Some(sp))  => Some(format!("{ctx}\n\n---\n\n{sp}")),
        };

        agent::AgentLoop::new(
            Arc::clone(&self.provider),
            Arc::clone(&self.tools),
            &self.cfg.model,
            self.cfg.temperature,
            system_prompt,
            self.cfg.max_tool_iterations,
            self.cfg.max_history_messages,
        )
    }
}
