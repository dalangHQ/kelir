// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
// see: cans/02-plugin-model.md#Declarative-error-mapping
use serde::{Deserialize, Serialize};

use crate::errno_types::Errno;
use crate::plugin_model::PluginError;

// see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
pub const ECONFIG_CODE: &str = "EConfig";

// see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
pub const EWXVIOLATION_CODE: &str = "EWXVIOLATION";

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub const EFFIQUEUESATURATED_CODE: &str = "EFFIQUEUESATURATED";

// see: cans/02-plugin-model.md#Invariants-and-purity
pub const EAMBIGUOUSMUTATION_CODE: &str = "EAMBIGUOUSMUTATION";

// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
pub const EHALT_CODE: &str = "EHALT";

// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEnvelope {
    pub code: String,
    pub message: String,
    pub driver: Option<String>,
    pub ring: Option<String>,
    pub pid: Option<u32>,
    pub binary: Option<String>,
    pub errno: Option<Errno>,
}

impl ErrorEnvelope {
    // see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
    pub fn econfig(message: impl Into<String>) -> Self {
        Self {
            code: ECONFIG_CODE.to_string(),
            message: message.into(),
            driver: None,
            ring: None,
            pid: None,
            binary: None,
            errno: Some(Errno::Eperm),
        }
    }
    // see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
    fn mapped(code: &str, message: String, pid: Option<u32>, errno: Option<Errno>) -> Self {
        Self {
            code: code.to_string(),
            message,
            driver: None,
            ring: None,
            pid,
            binary: None,
            errno,
        }
    }
}

// see: cans/02-plugin-model.md#Declarative-error-mapping
// see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
pub fn config_envelope(message: impl Into<String>) -> PluginError {
    PluginError::Halt(format!("EConfig: {}", message.into()))
}

// see: cans/02-plugin-model.md#Declarative-error-mapping
// TODO (L3) [plugin_engine]: dispatch-time failures populate driver/ring/binary from the failing hook context.; slop: envelope filled with defaults, hook context never read
impl From<PluginError> for ErrorEnvelope {
    fn from(error: PluginError) -> Self {
        let message = error.to_string();
        match error {
            PluginError::WxViolation { pid, .. } => {
                Self::mapped(EWXVIOLATION_CODE, message, Some(pid), Some(Errno::Eperm))
            }
            PluginError::FfiQueueSaturated { .. } => {
                Self::mapped(EFFIQUEUESATURATED_CODE, message, None, Some(Errno::Ebusy))
            }
            PluginError::AmbiguousMutation { .. } => {
                Self::mapped(EAMBIGUOUSMUTATION_CODE, message, None, None)
            }
            PluginError::Io(io) => Self {
                code: "EIO".to_string(),
                message,
                driver: None,
                ring: None,
                pid: None,
                binary: None,
                errno: io.raw_os_error().and_then(Errno::from_raw_os_error),
            },
            PluginError::Halt(message) => {
                if message.starts_with("EConfig: ") {
                    Self::mapped(ECONFIG_CODE, message, None, Some(Errno::Econfig))
                } else {
                    Self::mapped(EHALT_CODE, message, None, None)
                }
            }
        }
    }
}
