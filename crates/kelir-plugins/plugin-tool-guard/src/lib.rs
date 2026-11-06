// see: cans/plugins/tool-guard.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_tool_guard.rs per cans/artifacts/config/project-structure.txt.
use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{ConsoleInterposer, KelirPlugin, MountScope};

#[derive(Clone)]
pub struct ToolRegistryService;

#[derive(Clone)]
pub struct ToolGuardPlugin {
    pub registry: ToolRegistryService,
}

impl ToolGuardPlugin {
    pub fn new() -> Self {
        Self {
            registry: ToolRegistryService,
        }
    }
}

impl Default for ToolGuardPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for ToolGuardPlugin {
    fn name(&self) -> &'static str {
        "tool-guard"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl ConsoleInterposer for ToolGuardPlugin {
    fn on_console_input(&self, _port: u32, _buf: &mut Vec<u8>) -> Result<(), PluginError> {
        // TODO: Intercept tools/call JSON-RPC payload and gate mutations
        Ok(())
    }
}
