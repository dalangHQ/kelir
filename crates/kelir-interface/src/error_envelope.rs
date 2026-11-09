// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
// see: cans/02-plugin-model.md#Declarative-error-mapping
use serde::{Deserialize, Serialize};

use crate::plugin_model::PluginError;

// see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
pub const ECONFIG_CODE: &str = "EConfig";

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
}

// see: cans/02-plugin-model.md#Declarative-error-mapping
// TODO (L3): dispatch-time failures populate driver/ring/pid/binary/errno from the failing hook context.
impl From<PluginError> for ErrorEnvelope {
    fn from(error: PluginError) -> Self {
        Self::econfig(error.to_string())
    }
}
