mod traits;
pub mod canvas_list;
pub mod canvas_read;
pub mod canvas_write;
pub mod file_read;
pub mod file_write;
pub mod external_cmd;
pub mod external_tools;
pub mod memory_recall;
pub mod memory_store;
pub mod adb;
pub mod shell;
pub mod web_fetch;
pub mod workspace_read;
pub mod workspace_list;
pub mod workspace_write;

pub use traits::{Tool, ToolResult, ToolSpec};

use std::collections::HashMap;
use std::sync::Arc;

/// Registry that maps tool names → boxed tool implementations.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register(&mut self, tool: impl Tool + 'static) {
        self.tools.insert(tool.name().to_string(), Arc::new(tool));
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        let mut specs: Vec<ToolSpec> = self.tools.values().map(|t| t.spec()).collect();
        specs.sort_by(|a, b| a.name.cmp(&b.name));
        specs
    }
}
