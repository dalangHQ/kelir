// see: cans/plugins/tool-guard.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_tool_guard.rs per cans/artifacts/config/project-structure.txt.
pub mod jsonrpc;
pub mod service;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    HandlerCtx, KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope, PipelineBinder,
    StreamInterposer, VirtioDevice,
};
use kelir_interface::protocol::{StreamDirection, StreamKind, Verdict};

use crate::jsonrpc::{McpFrame, McpMethod};
use crate::service::{ToolRegistry, ToolRegistryService, ToolSchema};

// see: cans/plugins/tool-guard.md
#[derive(Clone, Debug)]
pub struct ToolGuardConfig {
    pub max_invocations_per_trace: u32,
}

impl Default for ToolGuardConfig {
    fn default() -> Self {
        Self {
            max_invocations_per_trace: 50,
        }
    }
}

// see: cans/plugins/tool-guard.md
struct TraceQuota {
    invocations: u32,
}

// see: cans/plugins/tool-guard.md
struct ToolGuardState {
    traces: Mutex<HashMap<u32, TraceQuota>>,
}

// see: cans/plugins/tool-guard.md
#[derive(Clone)]
pub struct ToolGuardPlugin {
    config: ToolGuardConfig,
    registry: Arc<ToolRegistry>,
    state: Arc<ToolGuardState>,
}

impl ToolGuardPlugin {
    pub fn new() -> Self {
        Self {
            config: ToolGuardConfig::default(),
            registry: Arc::new(ToolRegistry::new()),
            state: Arc::new(ToolGuardState {
                traces: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn with_config(mut self, config: ToolGuardConfig) -> Self {
        self.config = config;
        self
    }

    fn charge_quota(&self, stream_id: u32) -> Result<bool, PluginError> {
        let mut traces = self
            .state
            .traces
            .lock()
            .map_err(|_| PluginError::Halt("tool-guard trace table poisoned".into()))?;
        let quota = traces
            .entry(stream_id)
            .or_insert(TraceQuota { invocations: 0 });
        if quota.invocations >= self.config.max_invocations_per_trace {
            return Ok(false);
        }
        // TODO (L3): token-bucket cadence clamp per tool per cans/plugins/tool-guard.md.
        quota.invocations = quota.invocations.saturating_add(1);
        Ok(true)
    }

    fn release_trace(&self, stream_id: u32) {
        if let Ok(mut traces) = self.state.traces.lock() {
            traces.remove(&stream_id);
        }
    }

    async fn guard_tool_call(
        &self,
        stream_id: u32,
        payload: &mut [u8],
    ) -> Result<Verdict, PluginError> {
        let frame = McpFrame::decode(payload)?;
        if frame.method != McpMethod::ToolsCall {
            return Ok(Verdict::Pass);
        }
        let Some(tool) = frame.tool_name.clone() else {
            return Ok(Verdict::Drop {
                errno: Some("EPERM".into()),
                reason: "tools/call frame carries no tool name".into(),
            });
        };
        if !self.charge_quota(stream_id)? {
            return Ok(Verdict::Drop {
                errno: Some("ETOOLQUOTA".into()),
                reason: format!("tool {tool} exceeded max_invocations_per_trace"),
            });
        }
        let schema = match self.registry.lookup_schema(&tool).await {
            Ok(Some(schema)) => schema,
            Ok(None) => {
                return Ok(Verdict::Drop {
                    errno: Some("EPERM".into()),
                    reason: format!("tool {tool} is not pinned in the locked catalog"),
                });
            }
            Err(err) => return Err(err),
        };
        self.evaluate_tool_call(&frame, &schema).await
    }

    async fn evaluate_tool_call(
        &self,
        frame: &McpFrame,
        schema: &ToolSchema,
    ) -> Result<Verdict, PluginError> {
        let _ = (frame, schema);
        // TODO (L3): taint-gated approval (Suspend until Approval), secret-pattern sanitization (Drop with ESecurityViolation), cyclic-rewrite rejection (ECYCLICREWRITE), and ExecveMutation serialization (Patch) per cans/plugins/tool-guard.md.
        Ok(Verdict::Pass)
    }

    async fn scrub_tool_response(
        &self,
        stream_id: u32,
        payload: &mut [u8],
    ) -> Result<Verdict, PluginError> {
        let _ = (stream_id, payload);
        // TODO (L3): strip host secrets from tool responses before model context ingestion per cans/plugins/tool-guard.md.
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
        // see: cans/11-protocol.md#Control-channel
        MountScope::default().with_devices([VirtioDevice::Vsock])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        let stage: Arc<dyn StreamInterposer> = Arc::new(self.clone());
        pipeline.bind_stream_scoped(StreamKind::McpConsole, stage, self.default_scope());
        let registry: Arc<dyn ToolRegistryService> = self.registry.clone();
        pipeline.service_registry().register(registry);
    }
}

#[async_trait]
impl LifecycleParticipant for ToolGuardPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        // see: cans/plugins/tool-guard.md
        self.registry.lock_catalog().await
    }
}

#[async_trait]
impl StreamInterposer for ToolGuardPlugin {
    async fn on_stream_open(
        &self,
        stream_id: u32,
        tags: &mut Vec<u8>,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let _ = (stream_id, tags, ctx);
        // TODO (L3): validate MCP_CONSOLE open tags per cans/11-protocol.md#Stream-mux-framing.
        Ok(Verdict::Pass)
    }

    async fn on_stream_data(
        &self,
        direction: StreamDirection,
        stream_id: u32,
        payload: &mut Vec<u8>,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let _ = ctx;
        match direction {
            StreamDirection::GuestToHost => self.guard_tool_call(stream_id, payload).await,
            StreamDirection::HostToGuest => self.scrub_tool_response(stream_id, payload).await,
        }
    }

    async fn on_stream_close(&self, stream_id: u32, ctx: &HandlerCtx) -> Result<(), PluginError> {
        let _ = ctx;
        self.release_trace(stream_id);
        Ok(())
    }
}
