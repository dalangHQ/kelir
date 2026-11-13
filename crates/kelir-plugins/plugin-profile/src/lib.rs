// see: cans/plugins/profile.md#Plugin-profile
#![forbid(unsafe_code)]

// TODO (L3) [governance_vault]: tests/test_profile.rs per cans/artifacts/config/project-structure.txt; slop: test_profile.rs missing, profile hydration untested, placeholders unresolved silently
use kelir_interface::plugin_model::PluginError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[path = "patch.flow.rs"]
pub mod patch;

#[path = "profile_state.flow.rs"]
pub mod profile_state;

pub use profile_state::{NamespaceRouter, ProfileCtx, ProfilePlugin};

pub trait ConfigurablePlugin: Send + Sync {
    fn configure(&mut self, fragment: &[u8]) -> Result<(), PluginError>;
}

// see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatchOp {
    Add,
    Remove,
    Replace,
    Move,
    Copy,
    Test,
}

// see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonPatch {
    pub op: PatchOp,
    pub path: String,
    pub from: Option<String>,
    pub value: Option<Value>,
}

// see: cans/plugins/profile.md#Container-Device-Interface-(CDI)-device-mapping
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CdiConfig {
    pub enabled: bool,
    pub spec_dir: String,
}

// see: cans/plugins/profile.md#Plugin-profile
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileConfig {
    pub rfc6902_dynamic_patching: bool,
    pub enforce_monotonic_patches: bool,
    pub cdi: Option<CdiConfig>,
}
