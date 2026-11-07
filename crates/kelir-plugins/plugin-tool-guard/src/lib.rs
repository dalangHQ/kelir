// see: cans/plugins/tool-guard.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_tool_guard.rs per cans/artifacts/config/project-structure.txt.
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    ConsoleInterposer, KelirPlugin, MountScope, PipelineBinder, VirtioDevice,
};
use kelir_interface::protocol::{StreamKind, SuspendUntil, Verdict};

#[derive(Clone)]
pub struct ToolRegistryService;

#[derive(Clone)]
pub struct ToolGuardPlugin {
    pub registry: ToolRegistryService,
    tainted_traces: Arc<Mutex<HashSet<String>>>,
    call_counts: Arc<Mutex<HashMap<String, u32>>>,
}

impl ToolGuardPlugin {
    pub fn new() -> Self {
        Self {
            registry: ToolRegistryService,
            tainted_traces: Arc::new(Mutex::new(HashSet::new())),
            call_counts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn mark_tainted(&self, trace_id: &str) {
        if let Ok(mut tainted) = self.tainted_traces.lock() {
            tainted.insert(trace_id.to_string());
        }
    }

    pub fn is_tainted(&self, trace_id: &str) -> bool {
        match self.tainted_traces.lock() {
            Ok(tainted) => tainted.contains(trace_id),
            Err(_) => false,
        }
    }

    pub fn evaluate_tools_call(
        &self,
        payload: &[u8],
        trace_id: &str,
    ) -> Result<Verdict, PluginError> {
        if !is_tools_call(payload) {
            return Ok(Verdict::Pass);
        }
        let mut counts = match self.call_counts.lock() {
            Ok(counts) => counts,
            Err(_) => {
                return Err(PluginError::Halt(
                    "tool-guard: call-count lock poisoned".into(),
                ));
            }
        };
        let count = counts.entry(trace_id.to_string()).or_insert(0);
        *count += 1;
        if *count > 50 {
            return Ok(Verdict::Drop {
                errno: Some("ECYCLICREWRITE".to_string()),
                reason: "tool-guard: max_invocations_per_trace exceeded".to_string(),
            });
        }
        if self.is_tainted(trace_id) {
            return Ok(Verdict::Suspend {
                until: SuspendUntil::Approval,
            });
        }
        Ok(Verdict::Pass)
    }
}

impl Default for ToolGuardPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for ToolGuardPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "tool-guard"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default().with_devices([VirtioDevice::Console])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline
            .service_registry()
            .register(Arc::new(self.registry.clone()));
        pipeline.bind_console(Arc::new(self.clone()));
    }
}

#[async_trait]
impl ConsoleInterposer for ToolGuardPlugin {
    async fn on_console_input(&self, _port: u32, buf: &mut Vec<u8>) -> Result<(), PluginError> {
        // TODO (L3): return the Verdict directly once kelir-interface gains a Verdict-returning MCP_CONSOLE hook; ConsoleInterposer can only rewrite the frame in place.
        let verdict = self.evaluate_tools_call(buf, "default")?;
        if let Verdict::Drop { errno, reason } = verdict {
            let code = errno.as_deref().unwrap_or("EPERM");
            *buf = format!(
                "{{\"jsonrpc\":\"2.0\",\"error\":{{\"code\":-32001,\"message\":\"Policy violation: tool execution rejected ({code}: {reason})\"}},\"id\":null}}",
            )
            .into_bytes();
            return Ok(());
        }
        if let Verdict::Suspend { .. } = verdict {
            return Err(PluginError::Halt(
                "tool-guard: Suspend needs a Verdict-returning MCP_CONSOLE hook (L3)".into(),
            ));
        }
        if let Verdict::Abort { reason } = verdict {
            return Err(PluginError::Halt(reason));
        }
        Ok(())
    }
}

pub fn mcp_console_kind() -> StreamKind {
    StreamKind::McpConsole
}

fn is_tools_call(payload: &[u8]) -> bool {
    let needle = b"\"tools/call\"";
    payload.windows(needle.len()).any(|window| window == needle)
}
