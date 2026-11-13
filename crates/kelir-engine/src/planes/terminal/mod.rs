// see: cans/04-terminal-plane.md#Terminal-pipeline
// see: cans/04-terminal-plane.md#Stream-replay
use std::sync::Arc;

use kelir_interface::plugin_model::PluginError;
use kelir_interface::terminal::{
    LogStream, PromptRuleConfig, TerminalOps, TerminalWiring, TokenShieldConfig,
};
use kelir_otlp::registry_otlp::MetricRegistry;

pub struct TerminalPlane {
    session_id: [u8; 16],
    ops: Arc<dyn TerminalOps>,
    metrics: Arc<MetricRegistry>,
}
#[path = "plane.flow.rs"]
mod plane;
