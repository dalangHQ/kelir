// see: cans/artifacts/code/vmm-driver-trait.rs
// see: cans/artifacts/code/plugin-trait.rs
use thiserror::Error;

/// Non-verdict errors only. Denial and fatal termination are verdicts,
/// not errors: Verdict::Drop { errno, reason } and Verdict::Abort
/// { reason }. Storage plane: guest write() fails with the Drop errno.
#[derive(Error, Debug)]
pub enum PluginError {
    /// Host event loop or foreign runtime thread starvation detected.
    #[error("Foreign FFI channel saturated: worker queue depth exceeded ({depth})")]
    FfiQueueSaturated { depth: usize },
    /// Trap violation against W^X memory execution invariants.
    #[error("W^X execution violation detected: PID {pid} attempted {operation}")]
    WxViolation { pid: u32, operation: String },
    /// Conflicting semantic mutations on overlapping resource targets.
    #[error("Ambiguous mutation detected across plugins on resource '{resource}': {conflict}")]
    AmbiguousMutation { resource: String, conflict: String },
    #[error("Internal I/O failure: {0}")]
    Io(#[from] std::io::Error),
    /// Host-fatal error: the plugin cannot proceed and the pipeline
    /// must halt. This is not a guest verdict (denial is Verdict::Drop,
    /// termination is Verdict::Abort); it covers host-side failures
    /// like a missing checkpoint or an unparseable harness event.
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

    /// Policy rejection in a Result<(), PluginError> context, where no
    /// Verdict is returnable. Carries errno + reason in the message;
    /// handlers that can return a Verdict use Verdict::Drop instead.
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
