// see: cans/04-terminal-plane.md#Terminal-pipeline
use std::sync::Arc;

use kelir_interface::terminal::TerminalOps;
use kelir_otlp::registry_otlp::MetricRegistry;

use crate::real_terminal::build_terminal_ops;

pub fn terminal_ops(metrics: Arc<MetricRegistry>) -> Arc<dyn TerminalOps> {
    build_terminal_ops(metrics)
}
