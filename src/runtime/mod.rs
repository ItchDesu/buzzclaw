use crate::{canvas, config, memory, providers, tools, workspace};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub enum ToolProfile {
    Full,
    NoSpawn,
    Heartbeat,
    Cron,
}

pub fn build_provider(provider: &str, api_key: Option<String>) -> Arc<dyn providers::Provider> {
    match provider.trim().to_ascii_lowercase().as_str() {
        "openai" => Arc::new(providers::openai::OpenAiProvider::new(api_key.unwrap_or_default())),
        "anthropic" | "claude" => Arc::new(providers::anthropic::ClaudeProvider::new(api_key.unwrap_or_default())),
        "gemini" | "google" => Arc::new(providers::gemini::GeminiProvider::new(api_key.unwrap_or_default())),
        "kimi" | "moonshot" => Arc::new(providers::kimi::KimiProvider::new(api_key.unwrap_or_default())),
        "deepseek" => Arc::new(providers::deepseek::DeepseekProvider::new(api_key.unwrap_or_default())),
        "buzzster" => Arc::new(providers::buzzster::BuzzsterProvider::new(api_key.unwrap_or_default())),
        _ => Arc::new(providers::deepseek::DeepseekProvider::new(api_key.unwrap_or_default())),
    }
}

pub fn build_tool_registry(
    profile: ToolProfile,
    _cfg: &config::Config,
    memory: Arc<memory::MemoryStore>,
    ws: Arc<workspace::Workspace>,
    canvas_store: Option<Arc<canvas::CanvasStore>>,
) -> tools::ToolRegistry {
    let mut registry = tools::ToolRegistry::new();

    let include_shell = matches!(profile, ToolProfile::Full | ToolProfile::NoSpawn | ToolProfile::Heartbeat);
    let include_files = matches!(profile, ToolProfile::Full | ToolProfile::NoSpawn);
    let include_web = matches!(profile, ToolProfile::Full | ToolProfile::NoSpawn | ToolProfile::Heartbeat | ToolProfile::Cron);
    let include_canvas = matches!(profile, ToolProfile::Full | ToolProfile::NoSpawn);
    let _include_spawn = matches!(profile, ToolProfile::Full);

    if include_shell {
        registry.register(tools::shell::ShellTool);
        registry.register(tools::adb::AdbTool);
    }
    if include_files {
        registry.register(tools::file_read::FileReadTool);
        registry.register(tools::file_write::FileWriteTool);
    }
    if include_web {
        registry.register(tools::web_fetch::WebFetchTool::new());
    }

    registry.register(tools::memory_recall::MemoryRecallTool::new(Arc::clone(&memory)));
    registry.register(tools::memory_store::MemoryStoreTool::new(Arc::clone(&memory)));
    registry.register(tools::workspace_read::WorkspaceReadTool::new(Arc::clone(&ws)));
    registry.register(tools::workspace_write::WorkspaceWriteTool::new(Arc::clone(&ws)));
    registry.register(tools::workspace_list::WorkspaceListTool::new(
        Arc::clone(&ws),
        config::canvas_dir(),
    ));

    if include_canvas {
        if let Some(canvas) = canvas_store.as_ref() {
            registry.register(tools::canvas_read::CanvasReadTool::new(Arc::clone(canvas)));
            registry.register(tools::canvas_write::CanvasWriteTool::new(Arc::clone(canvas)));
            registry.register(tools::canvas_list::CanvasListTool::new(Arc::clone(canvas)));
        }
    }

    registry
}
