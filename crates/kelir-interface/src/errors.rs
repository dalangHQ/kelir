// see: cans/artifacts/code/vmm-driver-trait.rs
// see: cans/artifacts/code/plugin-trait.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Foreign FFI channel saturated: worker queue depth exceeded ({depth})")]
    FfiQueueSaturated { depth: usize },
    #[error("W^X execution violation detected: PID {pid} attempted {operation}")]
    WxViolation { pid: u32, operation: String },
    #[error("Ambiguous mutation detected across plugins on resource '{resource}': {conflict}")]
    AmbiguousMutation { resource: String, conflict: String },
    #[error("Internal I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("Plugin halted: {0}")]
    Halt(String),
}

impl PluginError {
    pub fn ambiguous_mutation(resource: impl Into<String>, conflict: impl Into<String>) -> Self {
        Self::AmbiguousMutation {
            resource: resource.into(),
            conflict: conflict.into(),
        }
    }

    pub fn reject(errno: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Halt(format!("{}: {}", errno.into(), reason.into()))
    }
}

#[derive(Error, Debug)]
pub enum VmmError {
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("invalid state: {0}")]
    InvalidState(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("exhausted: {0}")]
    ResourceExhausted(String),
}

pub type DriverError = VmmError;
