use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use std::sync::Arc;

/// Allows the main agent to delegate tasks to named or ad-hoc subagents.
pub struct SpawnAgentTool {
    cfg: Arc<crate::config::Config>,
    memory: Arc<crate::memory::MemoryStore>,
    workspace: Arc<crate::workspace::Workspace>,
    canvas: Arc<crate::canvas::CanvasStore>,
}

impl SpawnAgentTool {
    pub fn new(
        cfg: Arc<crate::config::Config>,
        memory: Arc<crate::memory::MemoryStore>,
        workspace: Arc<crate::workspace::Workspace>,
        canvas: Arc<crate::canvas::CanvasStore>,
    ) -> Self {
        Self { cfg, memory, workspace, canvas }
    }
}

#[async_trait]
impl Tool for SpawnAgentTool {
    fn name(&self) -> &str {
        "spawn_agent"
    }

    fn description(&self) -> &str {
        "Spawn a subagent to handle a specific subtask autonomously. \
         The subagent runs with its own conversation history and full tool access, \
         then returns its final answer. \
         Use this to delegate research, writing, code generation, file operations, \
         or any task that benefits from isolated execution. \
         Optionally reference a named agent (from `buzzclaw agents list`) \
         to use its configured model, provider, and system prompt."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The task or question to give the subagent."
                },
                "agent": {
                    "type": "string",
                    "description": "Name of a configured agent to use (optional). \
                                   Inherits that agent's model, provider, and system prompt."
                },
                "system_prompt": {
                    "type": "string",
                    "description": "Inline system prompt for this subagent (overrides any named agent's prompt)."
                },
                "model": {
                    "type": "string",
                    "description": "Model override for this subagent (e.g. 'gpt-4o-mini' for fast tasks)."
                }
            },
            "required": ["prompt"]
        })
    }

    async fn run(&self, args: serde_json::Value) -> ToolResult {
        let prompt = match args.get("prompt").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolResult::err("missing 'prompt' argument"),
        };

        let agent_name = args.get("agent").and_then(|v| v.as_str());
        let system_prompt_override = args.get("system_prompt").and_then(|v| v.as_str());
        let model_override = args.get("model").and_then(|v| v.as_str());

        let cfg = &self.cfg;
        let agent_cfg = agent_name.and_then(|n| cfg.agents.get(n));

        // Resolve provider
        let provider_kind = agent_cfg
            .and_then(|a| a.provider.clone())
            .unwrap_or_else(|| cfg.provider.clone());

        let api_key = match crate::auth::resolve_provider_credential(cfg, &provider_kind).await {
            Ok(k) => k,
            Err(e) => return ToolResult::err(format!("API credential error: {e}")),
        };

        let provider: Arc<dyn crate::providers::Provider> =
            crate::runtime::build_provider(&provider_kind, api_key);

        let model = model_override
            .map(|m| m.to_string())
            .or_else(|| agent_cfg.and_then(|a| a.model.clone()))
            .unwrap_or_else(|| cfg.model.clone());

        let temperature = agent_cfg
            .and_then(|a| a.temperature)
            .unwrap_or(cfg.temperature);

        // System prompt: inline override > agent config > workspace context
        let base_prompt = if let Some(sp) = system_prompt_override {
            Some(sp.to_string())
        } else {
            agent_cfg
                .and_then(|a| a.system_prompt.clone())
                .or_else(|| cfg.system_prompt.clone())
        };

        // Load skills for named agent
        let skill_text = agent_cfg
            .filter(|a| !a.skills.is_empty())
            .and_then(|a| {
                crate::skills::SkillStore::open(&crate::config::skills_dir())
                    .ok()
                    .map(|s| s.load_many(&a.skills))
                    .filter(|t| !t.is_empty())
            });

        let system_prompt = match (base_prompt, skill_text) {
            (Some(p), Some(s)) => Some(format!("{p}\n\n---\n\n{s}")),
            (Some(p), None)    => Some(p),
            (None, Some(s))    => Some(s),
            (None, None)       => None,
        };

        // Inject workspace context
        let ws_ctx = self.workspace.build_context();
        let system_prompt = match (ws_ctx.as_str(), system_prompt) {
            ("", sp)         => sp,
            (ctx, None)      => Some(ctx.to_string()),
            (ctx, Some(sp))  => Some(format!("{ctx}\n\n---\n\n{sp}")),
        };

        // Build a fresh tool registry for the subagent.
        // Intentionally excludes spawn_agent to prevent unbounded recursion.
        let registry = crate::runtime::build_tool_registry(
            crate::runtime::ToolProfile::NoSpawn,
            cfg,
            Arc::clone(&self.memory),
            Arc::clone(&self.workspace),
            Some(Arc::clone(&self.canvas)),
        );

        let subagent = crate::agent::AgentLoop::new(
            provider,
            Arc::new(registry),
            &model,
            temperature,
            system_prompt,
            cfg.max_tool_iterations,
            cfg.max_history_messages,
        );

        let label = agent_name.unwrap_or("anonymous");
        tracing::info!("subagent '{label}' starting: {}", &prompt[..prompt.len().min(80)]);

        let mut history = Vec::new();
        match subagent.run_turn(&mut history, &prompt).await {
            Ok(response) => {
                tracing::info!("subagent '{label}' finished ({} chars)", response.len());
                ToolResult::ok(response)
            }
            Err(e) => ToolResult::err(format!("subagent '{label}' error: {e}")),
        }
    }
}
