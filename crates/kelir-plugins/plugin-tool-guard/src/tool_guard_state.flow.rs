// see: cans/plugins/tool-guard.md#Plugin-tool-guard
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    ConsoleInterposer, HCons, HNil, KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope,
    PipelineBinder, PluginDeps, VirtioDevice,
};
use kelir_interface::protocol_wire::{Errno, StreamKind, Verdict};
use plugin_mcp::{McpCtx, McpEndpointService};

use crate::service_tool_guard::{
    McpFrame, McpMethod, ToolRegistry, ToolRegistryService, ToolSchema, ToolsListResult,
    decode_tools_list,
};
use crate::{ToolGuardConfig, ToolSpec};

impl Default for ToolGuardConfig {
    fn default() -> Self {
        Self {
            max_invocations_per_trace: 50,
            pin_schemas: true,
        }
    }
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
const TOOLS_LIST_QUERY: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"tools/list\"}";

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
    mcp: McpEndpointService,
    host_specs: Vec<ToolSpec>,
}

impl ToolGuardPlugin {
    // see: cans/plugins/tool-guard.md#Total-tool-call-interposition
    fn interposes_port(&self, port: u32) -> bool {
        u16::try_from(port)
            .ok()
            .and_then(StreamKind::from_u16)
            .is_some_and(|kind| self.default_scope().allows_stream_kind(kind))
    }
    pub fn new() -> Self {
        Self {
            config: ToolGuardConfig::default(),
            registry: Arc::new(ToolRegistry::new()),
            state: Arc::new(ToolGuardState {
                traces: Mutex::new(HashMap::new()),
            }),
            mcp: McpEndpointService::new(),
            host_specs: Vec::new(),
        }
    }

    // see: cans/plugins/tool-guard.md#Crate-dependencies-(tool-guard)
    pub fn with_mcp_service(mut self, service: McpEndpointService) -> Self {
        self.mcp = service;
        self
    }

    pub fn with_config(mut self, config: ToolGuardConfig) -> Self {
        self.config = config;
        self
    }

    // see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
    pub fn tool(mut self, spec: ToolSpec) -> Self {
        self.host_specs.push(spec);
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
        // TODO (L3) [gt_04_token_bucket_call_quota_etoolquota]: token-bucket cadence clamp per tool per cans/plugins/tool-guard.md; slop: bucket never charged, burst passes, ETOOLQUOTA never fires
        quota.invocations = quota.invocations.saturating_add(1);
        Ok(true)
    }

    // see: cans/plugins/tool-guard.md#Immutable-schema-locking
    pub async fn ingest_advertisement(
        &self,
        advertisement: &ToolsListResult,
    ) -> Result<(), PluginError> {
        self.registry.ingest_tools_list(advertisement).await?;
        if self.config.pin_schemas && self.registry.locked_digest()?.is_none() {
            self.registry.lock_catalog().await?;
        }
        Ok(())
    }

    async fn guard_tool_call(&self, trace: u32, payload: &[u8]) -> Result<Verdict, PluginError> {
        let frame = McpFrame::decode(payload)?;
        // see: cans/plugins/tool-guard.md#Total-tool-call-interposition
        if frame.method == McpMethod::Unparsed {
            return Ok(Verdict::Drop {
                errno: Some(Errno::Eperm),
                reason: "unparsed MCP frame fails closed pending JSON-RPC 2.0 decode".into(),
            });
        }
        if frame.method != McpMethod::ToolsCall {
            return Ok(Verdict::Pass);
        }
        let Some(tool) = frame.tool_name.clone() else {
            return Ok(Verdict::Drop {
                errno: Some(Errno::Eperm),
                reason: "tools/call frame carries no tool name".into(),
            });
        };
        if !self.charge_quota(trace)? {
            return Ok(Verdict::Drop {
                errno: Some(Errno::Etoolquota),
                reason: format!("tool {tool} exceeded max_invocations_per_trace"),
            });
        }
        let schema = match self.registry.lookup_schema(&tool).await {
            Ok(Some(schema)) => schema,
            Ok(None) => {
                return Ok(Verdict::Drop {
                    errno: Some(Errno::Eperm),
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
        // TODO (L3) [gt_03_taint_tracking_mutation_suspend]: taint-gated approval (Suspend until Approval), secret-pattern sanitization (Drop with ESecurityViolation), cyclic-rewrite rejection (ECYCLICREWRITE), and ExecveMutation serialization (Patch) per cans/plugins/tool-guard.md; slop: evaluate returns Pass unconditionally, tainted mutation executes, secrets leak
        Ok(Verdict::Pass)
    }

    async fn scrub_tool_response(&self, payload: &[u8]) -> Result<(), PluginError> {
        let _ = payload;
        // TODO (L3) [gt_06_payload_secret_pattern_scrub]: strip host secrets from tool responses before model context ingestion per cans/plugins/tool-guard.md; slop: scrub returns Ok(()) unscrubbed, host secret reaches model context
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
        request: &crate::service_tool_guard::ApprovalRequest,
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
        // see: cans/plugins/tool-guard.md#Total-tool-call-interposition
        MountScope::default()
            .with_devices([VirtioDevice::Console, VirtioDevice::Vsock])
            .with_stream_kinds([StreamKind::McpConsole])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        // see: cans/plugins/tool-guard.md#Executes-upstream-of-plugin-mcp-on-the-MCP_CONSOLE-mux-stream
        // see: cans/02-plugin-model.md#Segregated-ring-traits
        let console_stage: Arc<dyn ConsoleInterposer> = Arc::new(self.clone());
        pipeline.bind_console_scoped(console_stage, self.default_scope());
        // see: cans/plugins/tool-guard.md#Immutable-schema-locking
        let lifecycle_stage: Arc<dyn LifecycleParticipant> = Arc::new(self.clone());
        pipeline.bind_lifecycle_scoped(lifecycle_stage, self.default_scope());
    }
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
impl PluginDeps for ToolGuardPlugin {
    type Deps = HCons<McpCtx, HNil>;
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
#[async_trait]
impl LifecycleParticipant for ToolGuardPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        // see: cans/plugins/tool-guard.md#Immutable-schema-locking
        // TODO (L3) [governance_tools]: boot-time tools/list fetch+lock halts here until route_request and decode_tools_list land; an empty-catalog boot would fake the spec-mandated lock; slop: empty-catalog boot fakes the lock, unpinned tools run
        let response = self.mcp.route_request(TOOLS_LIST_QUERY).await?;
        let mut advertisement = decode_tools_list(&response)?;
        // see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
        for spec in &self.host_specs {
            advertisement.tools.push(ToolSchema {
                name: spec.name.clone(),
                description: spec.description.clone(),
                input_schema: spec.input_schema.clone(),
                signature: Vec::new(),
            });
        }
        let advertised: Vec<String> = advertisement
            .tools
            .iter()
            .map(|schema| schema.name.clone())
            .collect();
        self.ingest_advertisement(&advertisement).await?;
        self.mcp.advertise_tools(advertised)
    }
}

#[async_trait]
impl ConsoleInterposer for ToolGuardPlugin {
    async fn on_console_input(&self, port: u32, bytes: &[u8]) -> Result<Vec<u8>, PluginError> {
        // TODO (L3) [governance_tools]: trace quota eviction once the console surface grows a close hook per cans/plugins/tool-guard.md; slop: trace quota never evicted, closed-trace budget leaks into next trace
        if !self.interposes_port(port) {
            return Ok(bytes.to_vec());
        }
        match self.guard_tool_call(port, bytes).await? {
            Verdict::Pass | Verdict::Patch(_) => Ok(bytes.to_vec()),
            Verdict::Drop { errno, reason } => {
                Err(PluginError::reject(errno.unwrap_or(Errno::Eperm), reason))
            }
            // TODO (L3) [gt_09_verdict_drop_jsonrpc_error_synthesis]: synthesize the JSON-RPC error object (code -32001) and the approval suspend per cans/plugins/tool-guard.md; slop: Suspend maps to Halt, connection closes, no -32001 framed error
            Verdict::Suspend { .. } => Err(PluginError::Halt(
                "tool-guard approval suspend pending (L3)".into(),
            )),
            Verdict::Abort { reason } => Err(PluginError::Halt(reason)),
            _ => Err(PluginError::Halt(
                "tool-guard verdict mapping pending (L3)".into(),
            )),
        }
    }

    async fn on_console_output(&self, port: u32, chunk: &[u8]) -> Result<Vec<u8>, PluginError> {
        // see: cans/plugins/tool-guard.md#Total-tool-call-interposition
        if !self.interposes_port(port) {
            return Ok(chunk.to_vec());
        }
        // see: cans/plugins/tool-guard.md#Immutable-schema-locking
        let frame = McpFrame::decode(chunk)?;
        if frame.method == McpMethod::ToolsList {
            // see: cans/plugins/tool-guard.md#Immutable-schema-locking
            let advertisement: ToolsListResult = decode_tools_list(chunk)?;
            self.ingest_advertisement(&advertisement).await?;
            return Ok(chunk.to_vec());
        }
        self.scrub_tool_response(chunk).await?;
        Ok(chunk.to_vec())
    }
}
