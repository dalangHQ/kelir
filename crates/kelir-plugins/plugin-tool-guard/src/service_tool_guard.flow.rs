// see: cans/plugins/tool-guard.md#Plugin-tool-guard
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Mutex;

use async_trait::async_trait;
use kelir_interface::byte_range::{ByteRange, TaintSet};
use kelir_interface::plugin_model::PluginError;
use plugin_mcp::WithParamsPredicate;
use plugin_mcp::ast::{JsonRpcId, parse_json_rpc_request, parse_json_rpc_response};
use serde_json::Value;

// see: cans/plugins/mcp.md#MCP-contract
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum McpMethod {
    ToolsList,
    ToolsCall,
    #[default]
    Other,
    Unparsed,
}

// see: cans/11-protocol.md#Stream-mux-framing
#[derive(Debug, Clone, Default)]
pub struct McpFrame {
    pub method: McpMethod,
    pub id: u64,
    pub tool_name: Option<String>,
    pub arguments: Vec<u8>,
}

impl McpFrame {
    // see: cans/plugins/tool-guard.md#Total-tool-call-interposition
    pub fn decode(payload: &[u8]) -> Result<Self, PluginError> {
        let request = match parse_json_rpc_request(payload) {
            Ok(request) => request,
            Err(_) => {
                return Ok(Self {
                    method: McpMethod::Unparsed,
                    id: 0,
                    tool_name: None,
                    arguments: Vec::new(),
                });
            }
        };
        let id = match request.id {
            Some(JsonRpcId::Number(id)) => id,
            Some(JsonRpcId::Text(_)) | None => 0,
        };
        match request.method.as_str() {
            "tools/list" => Ok(Self {
                method: McpMethod::ToolsList,
                id,
                tool_name: None,
                arguments: Vec::new(),
            }),
            "tools/call" => {
                let Some(params) = request.params else {
                    return Ok(Self {
                        method: McpMethod::Unparsed,
                        id: 0,
                        tool_name: None,
                        arguments: Vec::new(),
                    });
                };
                let Some(tool_name) = params
                    .get("name")
                    .and_then(|name| name.as_str())
                    .map(str::to_string)
                else {
                    return Ok(Self {
                        method: McpMethod::Unparsed,
                        id: 0,
                        tool_name: None,
                        arguments: Vec::new(),
                    });
                };
                let arguments = match params.get("arguments") {
                    Some(arguments) => serde_json::to_vec(arguments).unwrap_or_default(),
                    None => Vec::new(),
                };
                Ok(Self {
                    method: McpMethod::ToolsCall,
                    id,
                    tool_name: Some(tool_name),
                    arguments,
                })
            }
            _ => Ok(Self {
                method: McpMethod::Other,
                id,
                tool_name: None,
                arguments: Vec::new(),
            }),
        }
    }
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolsListChange {
    DescriptionRewrite { tool: String },
    SmuggledParams { tool: String },
    ToolAdded { tool: String },
    ToolRemoved { tool: String },
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
pub fn diff_tools_list(
    locked: &BTreeMap<String, ToolSchema>,
    candidate: &ToolsListResult,
) -> Vec<ToolsListChange> {
    let mut changes = Vec::new();
    let mut seen: BTreeMap<String, &ToolSchema> = BTreeMap::new();
    for schema in &candidate.tools {
        seen.insert(schema.name.clone(), schema);
    }
    for (name, old) in locked {
        let Some(new) = seen.get(name) else {
            changes.push(ToolsListChange::ToolRemoved { tool: name.clone() });
            continue;
        };
        if new.description != old.description {
            changes.push(ToolsListChange::DescriptionRewrite { tool: name.clone() });
        }
        if schema_param_keys(&new.input_schema) != schema_param_keys(&old.input_schema) {
            changes.push(ToolsListChange::SmuggledParams { tool: name.clone() });
        }
    }
    for name in seen.keys() {
        if !locked.contains_key(name) {
            changes.push(ToolsListChange::ToolAdded { tool: name.clone() });
        }
    }
    changes
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
fn schema_param_keys(input_schema: &[u8]) -> Vec<String> {
    let Ok(value) = serde_json::from_slice::<Value>(input_schema) else {
        return vec!["<unparseable>".to_string()];
    };
    let Some(properties) = value
        .get("properties")
        .and_then(|properties| properties.as_object())
    else {
        return Vec::new();
    };
    let mut keys: Vec<String> = properties
        .iter()
        .map(|(name, property)| {
            let kind = property
                .get("type")
                .and_then(|kind| kind.as_str())
                .unwrap_or("?");
            format!("{name}:{kind}")
        })
        .collect();
    keys.sort();
    keys
}

// see: cans/plugins/tool-guard.md#Taint-tracking-and-mutation-gating
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    pub kind: String,
    pub tool: String,
    pub detail: String,
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
#[derive(Debug, Clone, Default)]
pub struct ToolsListResult {
    pub tools: Vec<ToolSchema>,
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
pub fn decode_tools_list(payload: &[u8]) -> Result<ToolsListResult, PluginError> {
    let response = parse_json_rpc_response(payload)?;
    let Some(result) = response.result else {
        return Err(PluginError::Halt(
            "tools/list response carries no result envelope".into(),
        ));
    };
    let Some(tools) = result.get("tools").and_then(|tools| tools.as_array()) else {
        return Err(PluginError::Halt(
            "tools/list result carries no tools array".into(),
        ));
    };
    let mut out = ToolsListResult { tools: Vec::new() };
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(|name| name.as_str())
            .ok_or_else(|| PluginError::Halt("tools/list entry carries no string name".into()))?;
        let description = tool
            .get("description")
            .and_then(|description| description.as_str())
            .ok_or_else(|| {
                PluginError::Halt("tools/list entry carries no string description".into())
            })?;
        let input_schema = match tool.get("inputSchema") {
            Some(schema) => serde_json::to_vec(schema).unwrap_or_default(),
            None => Vec::new(),
        };
        out.tools.push(ToolSchema {
            name: name.to_string(),
            description: description.to_string(),
            input_schema,
            signature: Vec::new(),
        });
    }
    Ok(out)
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub input_schema: Vec<u8>,
    pub signature: Vec<u8>,
}

// see: cans/plugins/tool-guard.md#Taint-tracking-and-mutation-gating
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ToolTaint {
    #[default]
    Clean,
    Tainted,
}

// see: cans/plugins/tool-guard.md#Taint-tracking-and-mutation-gating
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub tool: String,
    pub canonical_arguments: Vec<u8>,
    pub payload_hash: String,
}

// see: cans/plugins/tool-guard.md#Service-exports-(tool-guard)
#[async_trait]
pub trait ToolRegistryService: Send + Sync {
    async fn lookup_schema(&self, tool: &str) -> Result<Option<ToolSchema>, PluginError>;
    async fn bind_approval(&self, request: &ApprovalRequest) -> Result<(), PluginError>;
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
#[derive(Debug, Default)]
pub struct ToolRegistry {
    schemas: Mutex<BTreeMap<String, ToolSchema>>,
    taint: Mutex<HashMap<String, ToolTaint>>,
    taint_ranges: Mutex<HashMap<String, TaintSet>>,
    locked_digest: Mutex<Option<[u8; 32]>>,
    approvals: Mutex<Vec<ApprovalRequest>>,
    audit: Mutex<Vec<AuditEvent>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock_schemas(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<String, ToolSchema>>, PluginError> {
        self.schemas
            .lock()
            .map_err(|_| PluginError::Halt("tool catalog table poisoned".into()))
    }

    fn lock_digest(&self) -> Result<std::sync::MutexGuard<'_, Option<[u8; 32]>>, PluginError> {
        self.locked_digest
            .lock()
            .map_err(|_| PluginError::Halt("tool catalog lock poisoned".into()))
    }

    // see: cans/plugins/tool-guard.md#Immutable-schema-locking
    pub async fn ingest_tools_list(
        &self,
        advertisement: &ToolsListResult,
    ) -> Result<(), PluginError> {
        let mut schemas = self.lock_schemas()?;
        let mut candidate: BTreeMap<String, ToolSchema> = BTreeMap::new();
        for schema in &advertisement.tools {
            candidate.insert(schema.name.clone(), schema.clone());
        }
        if self.lock_digest()?.is_some() {
            let changes = diff_tools_list(&schemas, advertisement);
            if !changes.is_empty() {
                let detail = changes
                    .iter()
                    .map(|change| format!("{change:?}"))
                    .collect::<Vec<String>>()
                    .join(", ");
                for change in &changes {
                    let tool = match change {
                        ToolsListChange::DescriptionRewrite { tool }
                        | ToolsListChange::SmuggledParams { tool }
                        | ToolsListChange::ToolAdded { tool }
                        | ToolsListChange::ToolRemoved { tool } => tool.clone(),
                    };
                    self.record_audit(AuditEvent {
                        kind: format!("{change:?}"),
                        tool,
                        detail: detail.clone(),
                    })?;
                }
                return Err(PluginError::Halt(format!(
                    "ESecurityViolation: tool catalog mutation after the locked advertisement: {detail}"
                )));
            }
            return Ok(());
        }
        *schemas = candidate;
        Ok(())
    }

    // see: cans/plugins/tool-guard.md#Immutable-schema-locking
    pub fn record_audit(&self, event: AuditEvent) -> Result<(), PluginError> {
        let mut audit = self
            .audit
            .lock()
            .map_err(|_| PluginError::Halt("tool audit table poisoned".into()))?;
        audit.push(event);
        Ok(())
    }

    // see: cans/plugins/tool-guard.md#Immutable-schema-locking
    pub fn drain_audit_events(&self) -> Vec<AuditEvent> {
        match self.audit.lock() {
            Ok(mut audit) => std::mem::take(&mut *audit),
            Err(_) => Vec::new(),
        }
    }

    // see: cans/plugins/tool-guard.md#Taint-tracking-and-mutation-gating
    pub fn propagate_byte_range_taints(
        &self,
        tool: &str,
        from: &[ByteRange],
        into_len: usize,
    ) -> Result<Vec<ByteRange>, PluginError> {
        let source = TaintSet::from_ranges(from.to_vec());
        let propagated = source.propagate(into_len);
        if propagated.is_empty() {
            return Ok(Vec::new());
        }
        let mut ranges = self
            .taint_ranges
            .lock()
            .map_err(|_| PluginError::Halt("tool taint-range table poisoned".into()))?;
        let entry = ranges.entry(tool.to_string()).or_default();
        for range in &propagated {
            entry.mark(*range);
        }
        self.mark_tainted(tool)?;
        Ok(propagated)
    }

    // see: cans/plugins/tool-guard.md#Taint-tracking-and-mutation-gating
    pub fn taint_ranges(&self, tool: &str) -> Result<Vec<ByteRange>, PluginError> {
        let ranges = self
            .taint_ranges
            .lock()
            .map_err(|_| PluginError::Halt("tool taint-range table poisoned".into()))?;
        Ok(ranges
            .get(tool)
            .map(|set| set.propagate(usize::MAX))
            .unwrap_or_default())
    }

    pub async fn lock_catalog(&self) -> Result<(), PluginError> {
        let schemas = self.lock_schemas()?;
        let canonical = canonical_catalog(&schemas);
        *self.lock_digest()? = Some(catalog_digest(&canonical));
        Ok(())
    }

    // see: cans/plugins/tool-guard.md#MCP-schema-matching-surface-(tool-guard)
    pub async fn match_with_params(
        &self,
        tool: &str,
        arguments: &[u8],
    ) -> Result<bool, PluginError> {
        let schema = match self.lookup_schema(tool).await? {
            Some(schema) => schema,
            None => {
                return Err(PluginError::Halt(format!(
                    "EConfig: with_params for unpinned tool {tool}"
                )));
            }
        };
        if self.locked_digest()?.is_none() {
            return Err(PluginError::Halt(format!(
                "EConfig: with_params for tool {tool} before the schema lock"
            )));
        }
        WithParamsPredicate::compile(tool, &schema.input_schema)?.matches(arguments)
    }

    // see: cans/plugins/tool-guard.md#Taint-tracking-and-mutation-gating
    pub fn mark_tainted(&self, tool: &str) -> Result<(), PluginError> {
        let mut taint = self
            .taint
            .lock()
            .map_err(|_| PluginError::Halt("tool taint table poisoned".into()))?;
        taint.insert(tool.to_string(), ToolTaint::Tainted);
        Ok(())
    }

    // see: cans/plugins/tool-guard.md#Taint-tracking-and-mutation-gating
    pub fn taint(&self, tool: &str) -> Result<ToolTaint, PluginError> {
        let taint = self
            .taint
            .lock()
            .map_err(|_| PluginError::Halt("tool taint table poisoned".into()))?;
        Ok(taint.get(tool).copied().unwrap_or(ToolTaint::Clean))
    }

    // see: cans/plugins/tool-guard.md#Immutable-schema-locking
    pub fn locked_digest(&self) -> Result<Option<[u8; 32]>, PluginError> {
        Ok(*self.lock_digest()?)
    }
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
fn canonical_catalog(schemas: &BTreeMap<String, ToolSchema>) -> Vec<u8> {
    let mut canonical = Vec::new();
    for schema in schemas.values() {
        canonical.extend_from_slice(&(schema.name.len() as u64).to_le_bytes());
        canonical.extend_from_slice(schema.name.as_bytes());
        canonical.extend_from_slice(&(schema.description.len() as u64).to_le_bytes());
        canonical.extend_from_slice(schema.description.as_bytes());
        canonical.extend_from_slice(&(schema.input_schema.len() as u64).to_le_bytes());
        canonical.extend_from_slice(&schema.input_schema);
        canonical.extend_from_slice(&(schema.signature.len() as u64).to_le_bytes());
        canonical.extend_from_slice(&schema.signature);
    }
    canonical
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
fn catalog_digest(canonical: &[u8]) -> [u8; 32] {
    *blake3::hash(canonical).as_bytes()
}

// see: cans/plugins/tool-guard.md#Service-exports-(tool-guard)
#[async_trait]
impl ToolRegistryService for ToolRegistry {
    async fn lookup_schema(&self, tool: &str) -> Result<Option<ToolSchema>, PluginError> {
        let schemas = self.lock_schemas()?;
        Ok(schemas.get(tool).cloned())
    }

    async fn bind_approval(&self, request: &ApprovalRequest) -> Result<(), PluginError> {
        let mut approvals = self
            .approvals
            .lock()
            .map_err(|_| PluginError::Halt("approval ledger poisoned".into()))?;
        // L3-GAP: [governance_tools]: verify canonical arguments and payload hash before the host approval binds per cans/plugins/tool-guard.md; slop: approval binds without hash check, mutated payload rides approval
        approvals.push(ApprovalRequest {
            tool: request.tool.clone(),
            canonical_arguments: request.canonical_arguments.clone(),
            payload_hash: request.payload_hash.clone(),
        });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Capability classifier + sliding-window cross-server exfiltration detector
// (pure L2; see blindess.patch.md#C: semantic read_then_send flows that
// byte-range plugin-taint cannot see, adapted from agentsh session_analyzer).
// ---------------------------------------------------------------------------

// see: cans/plugins/tool-guard.md#Total-tool-call-interposition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCapability {
    Read,
    Write,
    Send,
    Run,
}

// see: blindess.patch.md#C: capability-prefix classification (read/get/list vs
// send/post/upload vs run/exec) over a sliding window.
pub fn classify_capability(tool_name: &str) -> ToolCapability {
    let lower = tool_name.to_ascii_lowercase();
    for prefix in READ_CAPABILITY_PREFIXES {
        if lower.starts_with(prefix) {
            return ToolCapability::Read;
        }
    }
    for prefix in SEND_CAPABILITY_PREFIXES {
        if lower.starts_with(prefix) {
            return ToolCapability::Send;
        }
    }
    for prefix in WRITE_CAPABILITY_PREFIXES {
        if lower.starts_with(prefix) {
            return ToolCapability::Write;
        }
    }
    for prefix in RUN_CAPABILITY_PREFIXES {
        if lower.starts_with(prefix) {
            return ToolCapability::Run;
        }
    }
    // Unknown verbs carry no classification; treat as Run (fail-closed default
    // matches agentsh: unrecognized verbs are the risky bucket).
    ToolCapability::Run
}

// see: blindess.patch.md#C
const READ_CAPABILITY_PREFIXES: &[&str] = &["read", "get", "list", "fetch", "query", "search"];
// see: blindess.patch.md#C
const SEND_CAPABILITY_PREFIXES: &[&str] = &["send", "post", "upload", "publish", "mail", "emit"];
// see: blindess.patch.md#C
const WRITE_CAPABILITY_PREFIXES: &[&str] = &["write", "put", "patch", "update", "delete", "edit"];
// see: blindess.patch.md#C
const RUN_CAPABILITY_PREFIXES: &[&str] = &["run", "exec", "spawn", "invoke", "apply", "eval"];

// see: blindess.patch.md#C: one observed tool invocation inside the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolInvocation {
    pub server: String,
    pub tool: String,
    pub capability: ToolCapability,
}

// see: blindess.patch.md#C: fixed-size sliding window of invocations; the
// cross-server read_then_send pattern must be visible regardless of byte identity.
#[derive(Debug, Clone)]
pub struct SlidingInvocationWindow {
    events: VecDeque<ToolInvocation>,
    capacity: usize,
}

impl SlidingInvocationWindow {
    pub fn new(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, event: ToolInvocation) {
        if self.capacity == 0 {
            return;
        }
        if self.events.len() == self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    pub fn events(&self) -> &[ToolInvocation] {
        self.events.as_slices().0
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

// see: blindess.patch.md#C: verdict of the cross-server exfiltration detector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossServerExfilVerdict {
    pub denied: bool,
    pub reason: String,
}

// see: blindess.patch.md#C: a Send on server B is denied when a Read on any
// *other* server A is still inside the window — the LLM read secret data on A,
// possibly paraphrased it, and is now exfiltrating through B.
pub fn detect_read_then_send(
    window: &SlidingInvocationWindow,
    current: &ToolInvocation,
) -> CrossServerExfilVerdict {
    if current.capability == ToolCapability::Send {
        for prior in window.events() {
            if prior.capability == ToolCapability::Read && prior.server != current.server {
                return CrossServerExfilVerdict {
                    denied: true,
                    reason: format!(
                        "read_then_send: tool {} on server {} follows a read on server {} within the window",
                        current.tool, current.server, prior.server
                    ),
                };
            }
        }
    }
    CrossServerExfilVerdict {
        denied: false,
        reason: String::new(),
    }
}

// see: blindess.patch.md#C: entry point used by the guard pipeline; records the
// invocation into the window and evaluates the pattern before the call runs.
pub fn evaluate_cross_server_exfil(
    window: &mut SlidingInvocationWindow,
    server: &str,
    tool: &str,
) -> CrossServerExfilVerdict {
    let current = ToolInvocation {
        server: server.to_string(),
        tool: tool.to_string(),
        capability: classify_capability(tool),
    };
    let verdict = detect_read_then_send(window, &current);
    window.push(current);
    verdict
}

// ---------------------------------------------------------------------------
// Confusable (typosquatted) tool names — Levenshtein distance on tools/list
// ingestion (pure L2; see blindess.patch.md#C, agentsh name_similarity.go).
// ---------------------------------------------------------------------------

// see: blindess.patch.md#C
pub const CONFUSABLE_NAME_MAX_DISTANCE: usize = 2;

// see: blindess.patch.md#C: classic bounded Levenshtein DP, length-bounded so
// comparisons stay linear in name size.
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len() == 0 {
        return b.len();
    }
    if b.len() == 0 {
        return a.len();
    }
    if (a.len() as isize - b.len() as isize).abs() as usize > CONFUSABLE_NAME_MAX_DISTANCE {
        // Fast path: length delta alone exceeds the threshold.
        return CONFUSABLE_NAME_MAX_DISTANCE + 1;
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            current[j + 1] = (current[j] + 1)
                .min(previous[j + 1] + 1)
                .min(previous[j] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

// see: blindess.patch.md#C: one flagged pair of Levenshtein-close tool names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfusableNamePair {
    pub server_a: String,
    pub tool_a: String,
    pub server_b: String,
    pub tool_b: String,
    pub distance: usize,
}

// see: blindess.patch.md#C: compares a freshly ingested tool name against the
// already-known catalog across servers; any pair within the distance threshold
// is flagged so a typosquat cannot shadow a trusted tool.
pub fn find_confusable_names(
    known: &[(String, String)],
    candidate_server: &str,
    candidate_tool: &str,
) -> Vec<ConfusableNamePair> {
    let mut hits = Vec::new();
    for (server, tool) in known {
        if *server == candidate_server && *tool == candidate_tool {
            continue;
        }
        let distance = levenshtein_distance(tool, candidate_tool);
        if distance <= CONFUSABLE_NAME_MAX_DISTANCE && tool != candidate_tool {
            hits.push(ConfusableNamePair {
                server_a: server.clone(),
                tool_a: tool.clone(),
                server_b: candidate_server.to_string(),
                tool_b: candidate_tool.to_string(),
                distance,
            });
        }
    }
    hits
}

// ---------------------------------------------------------------------------
// ApprovalAuth — role separation, self-approval denial, pending-hold cap & TTL
// (pure L2; crypto verification of WebAuthn/TOTP/OIDC assertions stays L3).
// ---------------------------------------------------------------------------

// see: blindess.patch.md#C / cans/08-lifecycle.md: roles allowed to resolve a
// human approval hold; the agent that created the hold is never an approver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalRole {
    Agent,
    Approver,
    Admin,
}

// see: blindess.patch.md#C: one pending approval hold tracked by the validator.
#[derive(Debug, Clone)]
pub struct PendingApprovalHold {
    pub hold_id: String,
    pub requester_role: ApprovalRole,
    pub requester_id: String,
    pub created_at_seconds: u64,
}

// see: blindess.patch.md#C: result of validating an approval resolution attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalAuthVerdict {
    pub allowed: bool,
    pub reason: String,
}

// see: blindess.patch.md#C: pure validator over role separation, self-approval
// denial, allowed methods, the pending-hold cap and the hold TTL.
pub struct ApprovalAuthValidator {
    pub deny_self_approval: bool,
    pub methods: Vec<String>,
    pub max_pending_holds: u32,
    pub hold_ttl_seconds: u64,
}

impl ApprovalAuthValidator {
    pub fn new(
        deny_self_approval: bool,
        methods: Vec<String>,
        max_pending_holds: u32,
        hold_ttl_seconds: u64,
    ) -> Self {
        Self {
            deny_self_approval,
            methods,
            max_pending_holds,
            hold_ttl_seconds,
        }
    }

    // see: blindess.patch.md#C: may the requester open a new hold at all —
    // pending holds must never be able to DoS the exec-slot budget.
    pub fn validate_hold_cap(&self, pending: usize) -> ApprovalAuthVerdict {
        if self.max_pending_holds == 0 {
            return ApprovalAuthVerdict {
                allowed: false,
                reason: "approval holds are disabled (max_pending_holds is zero)".to_string(),
            };
        }
        if pending >= self.max_pending_holds as usize {
            return ApprovalAuthVerdict {
                allowed: false,
                reason: format!(
                    "pending approval holds {pending} reached the cap {}",
                    self.max_pending_holds
                ),
            };
        }
        ApprovalAuthVerdict {
            allowed: true,
            reason: String::new(),
        }
    }

    // see: blindess.patch.md#C: a hold past its TTL is dead and must be
    // resolved as expired, never silently approved.
    pub fn hold_expired(&self, hold: &PendingApprovalHold, now_seconds: u64) -> bool {
        now_seconds.saturating_sub(hold.created_at_seconds) > self.hold_ttl_seconds
    }

    // see: blindess.patch.md#C: role separation + self-approval denial for a
    // resolution attempt on a (still-live) hold.
    pub fn validate_resolution(
        &self,
        hold: &PendingApprovalHold,
        resolver_role: &ApprovalRole,
        resolver_id: &str,
        method: &str,
        now_seconds: u64,
    ) -> ApprovalAuthVerdict {
        if self.hold_expired(hold, now_seconds) {
            return ApprovalAuthVerdict {
                allowed: false,
                reason: format!("hold {} expired its {}s TTL", hold.hold_id, self.hold_ttl_seconds),
            };
        }
        if matches!(resolver_role, ApprovalRole::Agent) {
            return ApprovalAuthVerdict {
                allowed: false,
                reason: "agent role may never resolve a human approval hold".to_string(),
            };
        }
        if self.deny_self_approval
            && resolver_id == hold.requester_id
            && resolver_role == &hold.requester_role
        {
            return ApprovalAuthVerdict {
                allowed: false,
                reason: format!(
                    "self-approval denied: {} resolved its own hold {}",
                    resolver_id, hold.hold_id
                ),
            };
        }
        if self.methods.is_empty() || !self.methods.iter().any(|m| m == method) {
            return ApprovalAuthVerdict {
                allowed: false,
                reason: format!("approval method {method} is not in the allowed set"),
            };
        }
        ApprovalAuthVerdict {
            allowed: true,
            reason: String::new(),
        }
    }
}

// TODO (L3) [governance_tools]: verify WebAuthn/TOTP/OIDC assertion signatures and challenge nonces for approval resolutions.; slop: unsigned approval assertions pass as authenticated, anyone resolves holds as admin.

// ---------------------------------------------------------------------------
// JSON-RPC -32001 error synthesis (see cans/plugins/tool-guard.md gt_09).
// ---------------------------------------------------------------------------

// see: cans/artifacts/test-plan/cases/governance_tools/gt_09_verdict_drop_jsonrpc_error_synthesis
pub const JSON_RPC_TOOL_GUARD_ERROR_CODE: i64 = -32001;

// see: gt_09: the error.data shape every tool-guard denial carries so the LLM
// sees errno, reason, guidance, and an optional redirect_tool steering target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolGuardErrorData {
    pub errno: String,
    pub reason: String,
    pub guidance: String,
    pub redirect_tool: Option<String>,
}

impl ToolGuardErrorData {
    pub fn new(errno: &str, reason: &str, guidance: &str, redirect_tool: Option<String>) -> Self {
        Self {
            errno: errno.to_string(),
            reason: reason.to_string(),
            guidance: guidance.to_string(),
            redirect_tool,
        }
    }

    // see: gt_09: render the JSON-RPC error object with code -32001.
    pub fn to_json_rpc_error(&self, id: Option<&JsonRpcId>) -> Value {
        let mut data = serde_json::Map::new();
        data.insert("errno".to_string(), Value::String(self.errno.clone()));
        data.insert("reason".to_string(), Value::String(self.reason.clone()));
        data.insert("guidance".to_string(), Value::String(self.guidance.clone()));
        if let Some(redirect) = &self.redirect_tool {
            data.insert(
                "redirect_tool".to_string(),
                Value::String(redirect.clone()),
            );
        }
        let mut error = serde_json::Map::new();
        error.insert(
            "code".to_string(),
            Value::Number(serde_json::Number::from(JSON_RPC_TOOL_GUARD_ERROR_CODE)),
        );
        error.insert("message".to_string(), Value::String(self.reason.clone()));
        error.insert("data".to_string(), Value::Object(data));
        let mut object = serde_json::Map::new();
        object.insert("jsonrpc".to_string(), Value::String("2.0".to_string()));
        match id {
            Some(JsonRpcId::Number(number)) => {
                object.insert(
                    "id".to_string(),
                    Value::Number(serde_json::Number::from(*number)),
                );
            }
            Some(JsonRpcId::Text(text)) => {
                object.insert("id".to_string(), Value::String(text.clone()));
            }
            None => {
                object.insert("id".to_string(), Value::Null);
            }
        }
        object.insert("error".to_string(), Value::Object(error));
        Value::Object(object)
    }
}

// TODO (L3) [governance_tools]: scan tool results for hidden-instruction / prompt-injection payloads before model context ingestion.; slop: injected instructions ride tool results into the model, guard reports clean.
