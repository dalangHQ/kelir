// see: cans/plugins/tool-guard.md#Plugin-tool-guard
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kelir_interface::driver_interface::{ExternalSuspendKind, SuspendReason};
use kelir_interface::errno_types::Errno;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    ConsoleInterposer, HCons, HNil, KelirPlugin, LifecycleParticipant, MountScope, PipelineBinder,
    PluginDeps, VirtioDevice,
};
use kelir_interface::protocol_wire::{StreamKind, SuspendUntil, Verdict};
use plugin_mcp::{McpCtx, McpEndpointService};

use crate::service_tool_guard::{
    ApprovalHold, AuditEvent, CrossServerDetector, McpFrame, McpMethod, ToolRegistry,
    ToolRegistryService, ToolSchema, ToolTaint, ToolsListResult, classify_tool_capabilities,
    confusable_tool_names, decode_tools_list, levenshtein, scan_tool_result,
};
use crate::{ToolGuardConfig, ToolSpec};
use kelir_interface::byte_range::ByteRange;

impl Default for ToolGuardConfig {
    fn default() -> Self {
        Self {
            max_invocations_per_trace: 50,
            pin_schemas: true,
            max_frame_bytes: 16_777_216,
            detect_semantic_rug_pull: true,
            strict_ast_parameter_validation: true,
            propagate_byte_range_taints: true,
            detect_cross_server_exfil: true,
            detect_confusable_names: true,
            scan_tool_results: true,
            approval_auth: crate::ApprovalAuthConfig::default(),
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
    cross_server: Mutex<CrossServerDetector>,
    seq: Mutex<u64>,
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
                cross_server: Mutex::new(CrossServerDetector::new()),
                seq: Mutex::new(0),
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
        // L3-GAP: [gt_04_token_bucket_call_quota_etoolquota]: token-bucket cadence clamp per tool per cans/plugins/tool-guard.md; slop: bucket never charged, burst passes, ETOOLQUOTA never fires
        quota.invocations = quota.invocations.saturating_add(1);
        Ok(true)
    }

    // see: cans/plugins/tool-guard.md#Immutable-schema-locking
    pub async fn ingest_advertisement(
        &self,
        advertisement: &ToolsListResult,
    ) -> Result<(), PluginError> {
        if self.config.detect_confusable_names {
            let known = self.registry.tool_names()?;
            for tool in &advertisement.tools {
                let hits = confusable_tool_names(&tool.name, &known);
                if !hits.is_empty() {
                    return Err(PluginError::Halt(format!(
                        "ESecurityViolation: confusable tool name {} shadows {}",
                        tool.name,
                        hits.join(", ")
                    )));
                }
            }
            for (i, first) in advertisement.tools.iter().enumerate() {
                for second in advertisement.tools.iter().skip(i + 1) {
                    if levenshtein(&first.name, &second.name) <= 2 {
                        return Err(PluginError::Halt(format!(
                            "ESecurityViolation: confusable tool names {} and {} in one advertisement",
                            first.name, second.name
                        )));
                    }
                }
            }
        }
        self.registry.ingest_tools_list(advertisement).await?;
        if self.config.pin_schemas && self.registry.locked_digest()?.is_none() {
            self.registry.lock_catalog().await?;
        }
        Ok(())
    }

    // see: cans/plugins/tool-guard.md#Plugin-tool-guard
    pub fn check_cross_server(&self, tool: &str, seq: u64) -> Result<bool, PluginError> {
        let server = tool.split("__").next().unwrap_or(tool);
        let capabilities = classify_tool_capabilities(tool);
        let mut detector = self
            .state
            .cross_server
            .lock()
            .map_err(|_| PluginError::Halt("tool-guard cross-server table poisoned".into()))?;
        Ok(detector.record(server, &capabilities, seq))
    }

    // see: cans/plugins/tool-guard.md#Plugin-tool-guard
    pub fn next_seq(&self) -> Result<u64, PluginError> {
        let mut seq = self
            .state
            .seq
            .lock()
            .map_err(|_| PluginError::Halt("tool-guard sequence table poisoned".into()))?;
        *seq = seq.saturating_add(1);
        Ok(*seq)
    }

    // see: cans/plugins/tool-guard.md#Plugin-tool-guard
    pub fn validate_approval_hold(
        &self,
        hold: &ApprovalHold,
        pending_holds: u32,
        now_unix: u64,
    ) -> Result<(), PluginError> {
        self.config
            .approval_auth
            .validate_hold(hold, pending_holds, now_unix)
    }

    async fn guard_tool_call(&self, trace: u32, payload: &[u8]) -> Result<Verdict, PluginError> {
        let frame = McpFrame::decode(payload)?;
        // see: cans/plugins/tool-guard.md#Total-tool-call-interposition
        if frame.method == McpMethod::Unparsed {
            return Ok(Verdict::Drop {
                errno: Errno::Eperm,
                reason: "unparsed MCP frame fails closed pending JSON-RPC 2.0 decode".into(),
            });
        }
        if frame.method != McpMethod::ToolsCall {
            return Ok(Verdict::Pass);
        }
        let Some(tool) = frame.tool_name.clone() else {
            return Ok(Verdict::Drop {
                errno: Errno::Eperm,
                reason: "tools/call frame carries no tool name".into(),
            });
        };
        if !self.charge_quota(trace)? {
            return Ok(Verdict::Drop {
                errno: Errno::Etoolquota,
                reason: format!("tool {tool} exceeded max_invocations_per_trace"),
            });
        }
        let schema = match self.registry.lookup_schema(&tool).await {
            Ok(Some(schema)) => schema,
            Ok(None) => {
                return Ok(Verdict::Drop {
                    errno: Errno::Eperm,
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
        // see: cans/plugins/tool-guard.md#Taint-tracking-and-mutation-gating
        if self.registry.taint(&schema.name)? == ToolTaint::Tainted {
            return Ok(Verdict::Suspend {
                until: SuspendUntil::Approval,
            });
        }
        if self.config.detect_cross_server_exfil {
            let seq = self.next_seq()?;
            if self.check_cross_server(&schema.name, seq)? {
                return Ok(Verdict::Drop {
                    errno: Errno::Eperm,
                    reason: format!(
                        "tool {} reads on one server then sends on another",
                        schema.name
                    ),
                });
            }
        }
        // see: cans/plugins/tool-guard.md#MCP-schema-matching-surface-(tool-guard)
        if !self
            .registry
            .match_with_params(&schema.name, &frame.arguments)
            .await?
        {
            return Ok(Verdict::Drop {
                errno: Errno::Eperm,
                reason: format!(
                    "tool {} arguments fail the pinned with_params schema",
                    schema.name
                ),
            });
        }
        // TODO (L3) [governance_tools]: secret-pattern sanitization (Drop with ESecurityViolation), cyclic-rewrite rejection (ECYCLICREWRITE), and ExecveMutation serialization (Patch); slop: tainted mutation executes, secrets leak.
        Ok(Verdict::Pass)
    }

    // see: cans/plugins/tool-guard.md#Suspend-reason-ownership-(tool-guard)
    pub fn human_authorization_hold() -> SuspendReason {
        SuspendReason::External(ExternalSuspendKind::External(
            "human-authorization-hold".to_string(),
        ))
    }

    async fn scrub_tool_response(&self, payload: &[u8]) -> Result<(), PluginError> {
        scan_tool_result(payload, self.config.scan_tool_results)?;
        // L3-GAP: [gt_06_payload_secret_pattern_scrub]: strip host secrets from tool responses before model context ingestion per cans/plugins/tool-guard.md; slop: scrub returns Ok(()) unscrubbed, host secret reaches model context
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

    // see: cans/plugins/tool-guard.md#MCP-schema-matching-surface-(tool-guard)
    pub async fn match_with_params(
        &self,
        tool: &str,
        arguments: &[u8],
    ) -> Result<bool, PluginError> {
        self.registry.match_with_params(tool, arguments).await
    }

    // see: cans/plugins/tool-guard.md#Taint-tracking-and-mutation-gating
    pub fn propagate_byte_range_taints(
        &self,
        tool: &str,
        from: &[ByteRange],
        into_len: usize,
    ) -> Result<Vec<ByteRange>, PluginError> {
        self.registry
            .propagate_byte_range_taints(tool, from, into_len)
    }

    // see: cans/plugins/tool-guard.md#Immutable-schema-locking
    pub fn drain_audit_events(&self) -> Vec<AuditEvent> {
        self.registry.drain_audit_events()
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

#[async_trait]
impl ConsoleInterposer for ToolGuardPlugin {
    async fn on_console_input(&self, port: u32, bytes: &[u8]) -> Result<Vec<u8>, PluginError> {
        // L3-GAP: [governance_tools]: trace quota eviction once the console surface grows a close hook per cans/plugins/tool-guard.md; slop: trace quota never evicted, closed-trace budget leaks into next trace
        if !self.interposes_port(port) {
            return Ok(bytes.to_vec());
        }
        match self.guard_tool_call(port, bytes).await? {
            Verdict::Pass | Verdict::Patch(_) => Ok(bytes.to_vec()),
            Verdict::Drop { errno, reason } => {
                Err(PluginError::Halt(format!("{}: {}", errno.as_str(), reason)))
            }
            // L3-GAP: [gt_09_verdict_drop_jsonrpc_error_synthesis]: synthesize the JSON-RPC error object (code -32001) and the approval suspend per cans/plugins/tool-guard.md; slop: Suspend maps to Halt, connection closes, no -32001 framed error
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
