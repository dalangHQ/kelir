// see: cans/plugins/tool-guard.md
use std::sync::Mutex;

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;

// see: cans/plugins/tool-guard.md
#[derive(Debug, Clone)]
pub struct ToolSchema {
    pub name: String,
    pub blake3: String,
}

// see: cans/plugins/tool-guard.md
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub tool: String,
    pub canonical_arguments: Vec<u8>,
    pub payload_hash: String,
}

// see: cans/plugins/tool-guard.md
#[async_trait]
pub trait ToolRegistryService: Send + Sync {
    async fn lookup_schema(&self, tool: &str) -> Result<Option<ToolSchema>, PluginError>;
    async fn bind_approval(&self, request: &ApprovalRequest) -> Result<(), PluginError>;
}

// see: cans/plugins/tool-guard.md
#[derive(Debug, Default)]
pub struct ToolRegistry {
    locked_digest: Mutex<Option<String>>,
    approvals: Mutex<Vec<ApprovalRequest>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn lock_catalog(&self) -> Result<(), PluginError> {
        let mut locked = self
            .locked_digest
            .lock()
            .map_err(|_| PluginError::Halt("tool catalog lock poisoned".into()))?;
        // TODO (L3): compute BLAKE3 over all advertised tool schemas, signatures, and descriptions per cans/plugins/tool-guard.md.
        *locked = Some(String::new());
        Ok(())
    }
}

#[async_trait]
impl ToolRegistryService for ToolRegistry {
    async fn lookup_schema(&self, tool: &str) -> Result<Option<ToolSchema>, PluginError> {
        let locked = self
            .locked_digest
            .lock()
            .map_err(|_| PluginError::Halt("tool catalog lock poisoned".into()))?;
        let _ = (tool, locked);
        // TODO (L3): resolve pinned schemas against the locked catalog digest; post-lock mutation halts with ESecurityViolation per cans/plugins/tool-guard.md.
        Ok(None)
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
