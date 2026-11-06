// see: cans/artifacts/code/vmm-driver-trait.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Configuration error: {0}")]
    EConfig(String),
    #[error("Ambiguous mutation scope: {0}")]
    EAmbiguousMutation(String),
    #[error("Session veto triggered: {0}")]
    ESessionVeto(String),
    #[error("Security policy violation: {0}")]
    ESecurityViolation(String),
    #[error("Cyclic rewrite detected")]
    ECyclicRewrite,
    #[error("Execution halted: {0}")]
    Halt(String),
    #[error("System IO error: {0}")]
    Io(#[from] std::io::Error),
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
