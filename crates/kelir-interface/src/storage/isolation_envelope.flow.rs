// see: cans/01-architecture.md#Isolation-envelope
// see: cans/05-storage-plane.md#Storage-architecture
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

// see: cans/01-architecture.md#Isolation-envelope
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrgScope {
    pub tenant_id: String,
    pub org_id: String,
}

impl OrgScope {
    // see: cans/01-architecture.md#Isolation-envelope
    pub fn new(tenant_id: impl Into<String>, org_id: impl Into<String>) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            org_id: org_id.into(),
        }
    }
}

// see: cans/01-architecture.md#Isolation-envelope
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IsolationError {
    EscapesTenantRoot { path: String },
    CrossTenant { tenant_id: String },
}

impl std::fmt::Display for IsolationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EscapesTenantRoot { path } => {
                write!(f, "path `{path}` escapes the tenant root")
            }
            Self::CrossTenant { tenant_id } => {
                write!(f, "partition belongs to tenant `{tenant_id}`")
            }
        }
    }
}

impl std::error::Error for IsolationError {}

// see: cans/01-architecture.md#Isolation-envelope
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolationEnvelope {
    pub scope: OrgScope,
    pub tenant_root: PathBuf,
    pub enforce_openat2_beneath: bool,
    pub cgroup_parent: String,
}

impl IsolationEnvelope {
    // see: cans/01-architecture.md#Isolation-envelope
    pub fn new(scope: OrgScope, tenant_root: PathBuf, cgroup_parent: impl Into<String>) -> Self {
        Self {
            scope,
            tenant_root,
            enforce_openat2_beneath: true,
            cgroup_parent: cgroup_parent.into(),
        }
    }

    // see: cans/01-architecture.md#Isolation-envelope
    pub fn partition_key(&self, session_id: &str) -> (String, String) {
        (self.scope.tenant_id.clone(), session_id.to_string())
    }

    // see: cans/01-architecture.md#Isolation-envelope
    pub fn partition_dir(&self, session_id: &str) -> PathBuf {
        self.tenant_root
            .join(&self.scope.tenant_id)
            .join(session_id)
    }

    // see: cans/01-architecture.md#Isolation-envelope
    pub fn assert_partition(&self, tenant_id: &str) -> Result<(), IsolationError> {
        if tenant_id != self.scope.tenant_id {
            return Err(IsolationError::CrossTenant {
                tenant_id: tenant_id.to_string(),
            });
        }
        Ok(())
    }

    // see: cans/01-architecture.md#Isolation-envelope
    pub fn resolve_beneath(&self, path: &Path) -> Result<PathBuf, IsolationError> {
        let mut resolved = PathBuf::new();
        for component in path.components() {
            match component {
                Component::RootDir => continue,
                Component::CurDir => continue,
                Component::ParentDir => {
                    if !resolved.pop() {
                        return Err(IsolationError::EscapesTenantRoot {
                            path: path.display().to_string(),
                        });
                    }
                }
                Component::Normal(part) => resolved.push(part),
                Component::Prefix(_) => {
                    return Err(IsolationError::EscapesTenantRoot {
                        path: path.display().to_string(),
                    });
                }
            }
        }
        // TODO (L3) [storage_plane]: open the resolved path with openat2(RESOLVE_BENEATH|RESOLVE_NO_MAGICLINKS) so a swapped symlink cannot escape the tenant root.; slop: resolve race open, symlink escape unchecked
        Ok(self.tenant_root.join(resolved))
    }
}

// see: cans/01-architecture.md#Isolation-envelope
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoragePartition {
    pub tenant_id: String,
    pub session_id: String,
}

impl StoragePartition {
    // see: cans/01-architecture.md#Isolation-envelope
    pub fn new(envelope: &IsolationEnvelope, session_id: &str) -> Self {
        let (tenant_id, session_id) = envelope.partition_key(session_id);
        Self {
            tenant_id,
            session_id,
        }
    }
}
