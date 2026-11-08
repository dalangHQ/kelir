// see: cans/plugins/tool-guard.md#Plugin-tool-guard
#![forbid(unsafe_code)]

// TODO (L3): tests/test_tool_guard.rs per cans/artifacts/config/project-structure.txt.
pub mod jsonrpc;
pub mod service;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    ConsoleInterposer, KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope, PipelineBinder,
    VirtioDevice,
};
use kelir_interface::protocol::Verdict;

use crate::jsonrpc::{McpFrame, McpMethod, ToolsListResult, decode_tools_list};
use crate::service::{ToolRegistry, ToolRegistryService, ToolSchema};

// see: cans/plugins/tool-guard.md#Call-quota
#[derive(Clone, Debug)]
pub struct ToolGuardConfig {
    pub max_invocations_per_trace: u32,
    pub pin_schemas: bool,
}

impl Default for ToolGuardConfig {
    fn default() -> Self {
        Self {
            max_invocations_per_trace: 50,
            pin_schemas: true,
        }
    }
}

// see: cans/plugins/tool-guard.md#Call-quota
struct TraceQuota {
    invocations: u32,
}

// see: cans/plugins/tool-guard.md#Call-quota
struct ToolGuardState {
    traces: Mutex<HashMap<u32, TraceQuota>>,
}

// see: cans/plugins/tool-guard.md#Plugin-tool-guard
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

    fn charge_quota(&self, trace: u32) -> Result<bool, PluginError> {
        let mut traces = self
            .state
            .traces
            .lock()
            .map_err(|_| PluginError::Halt("tool-guard trace table poisoned".into()))?;
        let quota = traces.entry(trace).or_insert(TraceQuota { invocations: 0 });
        if quota.invocations >= self.config.max_invocations_per_trace {
            return Ok(false);
        }
        // TODO (L3): token-bucket cadence clamp per tool per cans/plugins/tool-guard.md.
        quota.invocations = quota.invocations.saturating_add(1);
        Ok(true)
    }

    async fn guard_tool_call(
        &self,
        trace: u32,
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
        if !self.charge_quota(trace)? {
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

    async fn scrub_tool_response(&self, payload: &mut [u8]) -> Result<(), PluginError> {
        let _ = payload;
        // TODO (L3): strip host secrets from tool responses before model context ingestion per cans/plugins/tool-guard.md.
        Ok(())
    }
}

impl Default for ToolGuardPlugin {
    fn default() -> Self {
        Self::new()
    }
}

// see: cans/plugins/tool-guard.md#Service-exports-(tool-guard)
#[derive(Clone)]
pub struct ToolGuardCtx {
    registry: Arc<ToolRegistry>,
}

impl ToolGuardCtx {
    pub fn registry(&self) -> Arc<ToolRegistry> {
        Arc::clone(&self.registry)
    }

    pub async fn lookup_schema(&self, tool: &str) -> Result<Option<ToolSchema>, PluginError> {
        self.registry.lookup_schema(tool).await
    }

    pub async fn bind_approval(
        &self,
        request: &crate::service::ApprovalRequest,
    ) -> Result<(), PluginError> {
        self.registry.bind_approval(request).await
    }
}

impl KelirPlugin for ToolGuardPlugin {
    type Context = ToolGuardCtx;
    fn context(&self) -> ToolGuardCtx {
        ToolGuardCtx {
            registry: Arc::clone(&self.registry),
        }
    }
    fn name(&self) -> &'static str {
        "tool-guard"
    }
    fn default_scope(&self) -> MountScope {
        // see: cans/11-protocol.md#Control-channel
        MountScope::default().with_devices([VirtioDevice::Console, VirtioDevice::Vsock])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        // see: cans/plugins/tool-guard.md#Executes-upstream-of-plugin-mcp-on-the-MCP_CONSOLE-mux-stream
        // see: cans/02-plugin-model.md#Segregated-ring-traits
        let console_stage: Arc<dyn ConsoleInterposer> = Arc::new(self.clone());
        pipeline.bind_console_scoped(console_stage, self.default_scope());
    }
}

#[async_trait]
impl LifecycleParticipant for ToolGuardPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        // see: cans/plugins/tool-guard.md#Immutable-schema-locking
        if self.config.pin_schemas {
            return self.registry.lock_catalog().await;
        }
        Ok(())
    }
}

#[async_trait]
impl ConsoleInterposer for ToolGuardPlugin {
    async fn on_console_input(&self, port: u32, bytes: &mut Vec<u8>) -> Result<(), PluginError> {
        // TODO (L3): trace quota eviction once the console surface grows a close hook per cans/plugins/tool-guard.md.
        match self.guard_tool_call(port, bytes).await? {
            Verdict::Pass | Verdict::Patch(_) => Ok(()),
            Verdict::Drop { errno, reason } => Err(PluginError::reject(
                errno.unwrap_or_else(|| "EPERM".into()),
                reason,
            )),
            // TODO (L3): synthesize the JSON-RPC error object (code -32001) and the approval suspend per cans/plugins/tool-guard.md.
            Verdict::Suspend { .. } => Err(PluginError::Halt(
                "tool-guard approval suspend pending (L3)".into(),
            )),
            Verdict::Abort { reason } => Err(PluginError::Halt(reason)),
            _ => Err(PluginError::Halt(
                "tool-guard verdict mapping pending (L3)".into(),
            )),
        }
    }

    async fn on_console_output(&self, _port: u32, chunk: &mut Vec<u8>) -> Result<(), PluginError> {
        // see: cans/plugins/tool-guard.md#Immutable-schema-locking
        let frame = McpFrame::decode(chunk)?;
        if frame.method == McpMethod::ToolsList {
            let advertisement: ToolsListResult = decode_tools_list(chunk)?;
            return self.registry.ingest_tools_list(&advertisement).await;
        }
        self.scrub_tool_response(chunk).await
    }
}
