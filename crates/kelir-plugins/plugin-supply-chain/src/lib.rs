// see: cans/plugins/supply-chain.md#Plugin-supply-chain
#![forbid(unsafe_code)]

use kelir_interface::errno_types::Errno;

#[path = "state.flow.rs"]
pub mod state;
#[path = "wx_token.flow.rs"]
pub mod wx_token;

pub use state::{SupplyChainCtx, SupplyChainPlugin};
pub use wx_token::WxAllocToken;

// see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
pub const WX_VIOLATION_ERRNO: Errno = Errno::Ewxviolation;

// see: cans/plugins/supply-chain.md#Return-errno-and-rejection-behavior-(supply-chain)
pub const SCRIPT_DENY_ERRNO: Errno = Errno::Eacces;

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
pub const WX_ALLOC_VSOCK_PORT: u32 = 5253;

// see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizedJitEngine {
    pub binary_path: String,
    pub binary_blake3: [u8; 32],
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.supply_chain.wx_jit
#[derive(Clone, Debug, Default)]
pub struct WxJitConfig {
    pub trap_memfd: bool,
    pub trap_mprotect_exec: bool,
    pub allowed_jit_binaries: Vec<AuthorizedJitEngine>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.supply_chain.package_guard
#[derive(Clone, Debug)]
pub struct PackageGuardConfig {
    pub deny_lifecycle_scripts: bool,
    pub managers: Vec<String>,
    pub shannon_entropy_threshold_bits: f64,
    pub bypass_runners: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SupplyChainConfig {
    pub wx_jit: WxJitConfig,
    pub package_guard: PackageGuardConfig,
}

// see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
pub trait JitValidationService: Send + Sync {
    fn is_authorized(&self, binary_blake3: &[u8; 32]) -> bool;
}
