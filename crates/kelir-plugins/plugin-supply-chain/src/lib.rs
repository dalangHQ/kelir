// see: cans/plugins/supply-chain.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_supply_chain.rs per cans/artifacts/config/project-structure.txt.
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    HandlerCtx, KelirPlugin, MountScope, PipelineBinder, TrapInterposer,
};
use kelir_interface::protocol::{PROT_EXEC, TrapEvent, TrappedSyscall, Verdict};

// see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
pub const WX_VIOLATION_ERRNO: &str = "EWXVIOLATION";

// see: cans/plugins/supply-chain.md#Return-errno-and-rejection-behavior-(supply-chain)
pub const SCRIPT_DENY_ERRNO: &str = "EACCES";

// see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizedJitEngine {
    pub binary_path: String,
    pub binary_blake3: String,
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

impl Default for PackageGuardConfig {
    fn default() -> Self {
        Self {
            deny_lifecycle_scripts: true,
            managers: vec!["npm".into(), "pip".into(), "cargo".into()],
            shannon_entropy_threshold_bits: 5.2,
            bypass_runners: vec!["node".into(), "python".into()],
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SupplyChainConfig {
    pub wx_jit: WxJitConfig,
    pub package_guard: PackageGuardConfig,
}

// see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
pub trait JitValidationService: Send + Sync {
    fn is_authorized(&self, binary_blake3: &str) -> bool;
}

#[derive(Clone, Debug)]
pub struct SupplyChainPlugin {
    pub config: SupplyChainConfig,
}

impl SupplyChainPlugin {
    pub fn new(config: SupplyChainConfig) -> Self {
        Self { config }
    }
}

impl Default for SupplyChainPlugin {
    fn default() -> Self {
        Self::new(SupplyChainConfig::default())
    }
}

impl SupplyChainPlugin {
    fn mediate_execve(&self, event: &TrapEvent) -> Result<Verdict, PluginError> {
        // see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
        if self.config.package_guard.deny_lifecycle_scripts
            && self.blocks_lifecycle_script(event)?
        {
            // see: cans/plugins/supply-chain.md#Return-errno-and-rejection-behavior-(supply-chain)
            return Ok(Verdict::Drop {
                errno: Some(SCRIPT_DENY_ERRNO.into()),
                reason: format!("lifecycle script denied: {}", event.binary_path),
            });
        }
        Ok(Verdict::Pass)
    }

    fn mediate_memory_syscall(
        &self,
        event: &TrapEvent,
        syscall: &TrappedSyscall,
    ) -> Result<Verdict, PluginError> {
        // see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
        if let Some(prot) = syscall.mprotect_prot {
            if self.config.wx_jit.trap_mprotect_exec && prot & PROT_EXEC != 0 {
                return self.mediate_wx_gate(event, "mprotect(PROT_EXEC)");
            }
            return Ok(Verdict::Pass);
        }
        if let Some(flags) = syscall.memfd_create_flags {
            if self.config.wx_jit.trap_memfd {
                return self.mediate_wx_gate(event, &format!("memfd_create(flags={flags:#x})"));
            }
            return Ok(Verdict::Pass);
        }
        // TODO (L3): the seccomp-unotify mask pins the trapped syscall set; payloads outside the mediated set pass.
        Ok(Verdict::Pass)
    }

    fn mediate_wx_gate(&self, event: &TrapEvent, operation: &str) -> Result<Verdict, PluginError> {
        if self.is_authorized(&event.binary_blake3) {
            // TODO (L3): mint the dynamic one-time MMU allocation token and deliver it over vsock to the verified runtime.
            return Ok(Verdict::Pass);
        }
        // see: cans/plugins/supply-chain.md#Return-errno-and-rejection-behavior-(supply-chain)
        Ok(Verdict::Drop {
            errno: Some(WX_VIOLATION_ERRNO.into()),
            reason: format!("PID {} attempted {operation}", event.pid),
        })
    }

    fn flags_obfuscated_argv(&self, argv: &[String]) -> Result<bool, PluginError> {
        // TODO (L3): Shannon entropy of argv[1:] above shannon_entropy_threshold_bits flags encrypted or base64-obfuscated commands.
        let _ = argv;
        Ok(false)
    }

    fn blocks_lifecycle_script(&self, event: &TrapEvent) -> Result<bool, PluginError> {
        // TODO (L3): package-manager process-tree monitoring over the declared managers; dependency-directory lifecycle-script detection; nested npm→sh→node traps at the sh ancestor.
        let _ = event;
        Ok(false)
    }

    fn is_bypass_runner(&self, binary_path: &str) -> bool {
        // TODO (L3): bypass runner matching covers language toolchain layouts beyond file-name equality.
        Path::new(binary_path)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                self.config
                    .package_guard
                    .bypass_runners
                    .iter()
                    .any(|runner| runner.as_str() == name)
            })
    }
}

// see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
impl JitValidationService for SupplyChainPlugin {
    fn is_authorized(&self, binary_blake3: &str) -> bool {
        // TODO (L3): catalog storage internals; resolve allowed_jit_binaries disk paths to BLAKE3 digests for the verified JIT engine catalog (Node, Bun, V8).
        self.config
            .wx_jit
            .allowed_jit_binaries
            .iter()
            .any(|engine| engine.binary_blake3 == binary_blake3)
    }
}

impl KelirPlugin for SupplyChainPlugin {
    // see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
    type Context = Arc<dyn JitValidationService>;
    fn context(&self) -> Self::Context {
        Arc::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "supply-chain"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
    // see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline.bind_trap(Arc::new(self.clone()));
        pipeline
            .service_registry()
            .register::<dyn JitValidationService>(Arc::new(self.clone()));
    }
}

#[async_trait]
impl TrapInterposer for SupplyChainPlugin {
    async fn on_trap(&self, event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        // see: cans/plugins/supply-chain.md#Entropy-analysis
        if !self.is_bypass_runner(&event.binary_path) && self.flags_obfuscated_argv(&event.argv)? {
            // TODO (L3): flag-to-verdict policy for encrypted or base64-obfuscated argv above the entropy threshold.
            return Ok(Verdict::Drop {
                errno: Some(SCRIPT_DENY_ERRNO.into()),
                reason: format!("obfuscated argv denied: {}", event.binary_path),
            });
        }
        match &event.syscall {
            None => self.mediate_execve(event),
            Some(syscall) => self.mediate_memory_syscall(event, syscall),
        }
    }
}
