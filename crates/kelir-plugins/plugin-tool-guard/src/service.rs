// see: cans/plugins/tool-guard.md#Plugin-tool-guard
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;

// see: cans/plugins/mcp.md#MCP-contract
// TODO (L2): hoist the JSON-RPC frame AST into plugin-mcp so tool-guard imports it directly per cans/plugins/tool-guard.md#Crate-dependencies-(tool-guard).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum McpMethod {
    ToolsList,
    ToolsCall,
    #[default]
    Other,
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
    // see: cans/plugins/mcp.md#MCP-contract
    pub fn decode(payload: &[u8]) -> Result<Self, PluginError> {
        let _ = payload;
        // TODO (L3): decode JSON Lines frames and classify tools/call vs tools/list per cans/plugins/mcp.md; FLAG_CONTINUATION reassembly per cans/11-protocol.md#Stream-mux-framing.
        Ok(Self::default())
    }
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
#[derive(Debug, Clone, Default)]
pub struct ToolsListResult {
    pub tools: Vec<ToolSchema>,
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
pub fn decode_tools_list(payload: &[u8]) -> Result<ToolsListResult, PluginError> {
    // TODO (L3): decode the JSON Lines tools/list result envelope per cans/plugins/mcp.md.
    Err(PluginError::Halt(format!(
        "tools/list envelope decode pending (L3): {} bytes",
        payload.len()
    )))
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
    locked_digest: Mutex<Option<[u8; 32]>>,
    approvals: Mutex<Vec<ApprovalRequest>>,
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
        if self.lock_digest()?.is_some()
            && canonical_catalog(&candidate) != canonical_catalog(&schemas)
        {
            return Err(PluginError::reject(
                "ESecurityViolation",
                "tool catalog mutation after the locked advertisement",
            ));
        }
        *schemas = candidate;
        Ok(())
    }

    pub async fn lock_catalog(&self) -> Result<(), PluginError> {
        let schemas = self.lock_schemas()?;
        let canonical = canonical_catalog(&schemas);
        *self.lock_digest()? = Some(catalog_digest(&canonical));
        Ok(())
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
        // TODO (L3): verify canonical arguments and payload hash before the host approval binds per cans/plugins/tool-guard.md.
        approvals.push(ApprovalRequest {
            tool: request.tool.clone(),
            canonical_arguments: request.canonical_arguments.clone(),
            payload_hash: request.payload_hash.clone(),
        });
        Ok(())
    }
}
