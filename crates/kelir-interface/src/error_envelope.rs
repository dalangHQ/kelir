// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
// see: cans/02-plugin-model.md#Declarative-error-mapping
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEnvelope {
    pub code: String,
    pub message: String,
    pub driver: Option<String>,
    pub ring: Option<String>,
    pub pid: Option<u32>,
    pub binary: Option<String>,
    pub errno: Option<String>,
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
            errno: None,
        }
    }

    // see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
    fn mapped(code: &str, message: String, pid: Option<u32>, errno: Option<&str>) -> Self {
        Self {
            code: code.to_string(),
            message,
            driver: None,
            ring: None,
            pid,
            binary: None,
            errno: errno.map(|errno| errno.to_string()),
        }
    }
}

// see: cans/02-plugin-model.md#Declarative-error-mapping
// TODO (L3): dispatch-time failures populate driver/ring/binary from the failing hook context; numeric io errnos map to POSIX names via the builder error registry.
impl From<PluginError> for ErrorEnvelope {
    fn from(error: PluginError) -> Self {
        let message = error.to_string();
        match error {
            PluginError::WxViolation { pid, .. } => {
                Self::mapped(EWXVIOLATION_CODE, message, Some(pid), Some("EPERM"))
            }
            PluginError::FfiQueueSaturated { .. } => {
                Self::mapped(EFFIQUEUESATURATED_CODE, message, None, Some("EBUSY"))
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
                errno: io.raw_os_error().map(|errno| errno.to_string()),
            },
            PluginError::Halt(message) => Self::econfig(message),
        }
    }
}
