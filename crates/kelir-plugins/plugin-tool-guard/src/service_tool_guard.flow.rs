// see: cans/plugins/tool-guard.md#Plugin-tool-guard
use std::collections::{BTreeMap, HashMap};
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
