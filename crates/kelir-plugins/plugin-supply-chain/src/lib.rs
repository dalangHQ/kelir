// see: cans/plugins/supply-chain.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_supply_chain.rs per cans/artifacts/config/project-structure.txt.
use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    HandlerCtx, KelirPlugin, MountScope, PipelineBinder, TrapInterposer,
};
use kelir_interface::protocol::{PROT_EXEC, SyscallTrap, TrapEvent, Verdict};
use std::sync::Arc;

pub const ENTROPY_FLAG_BITS: f64 = 5.2;

#[derive(Clone)]
pub struct JitValidationService {
    pub authorized_jit_blake3: Vec<[u8; 32]>,
}

impl JitValidationService {
    pub fn new() -> Self {
        Self {
            authorized_jit_blake3: Vec::new(),
        }
    }

    pub fn is_authorized(&self, binary_blake3: &[u8; 32]) -> bool {
        self.authorized_jit_blake3.contains(binary_blake3)
    }
}

impl Default for JitValidationService {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct SupplyChainPlugin {
    pub jit_service: JitValidationService,
}

impl SupplyChainPlugin {
    pub fn new() -> Self {
        Self {
            jit_service: JitValidationService::new(),
        }
    }

    pub fn evaluate_memory_syscall(
        &self,
        event: &TrapEvent,
        trap: &SyscallTrap,
    ) -> Result<Verdict, PluginError> {
        let wants_exec = trap
            .prot_flags
            .map(|prot| prot & PROT_EXEC != 0)
            .unwrap_or(false);
        let creates_memfd = trap.memfd_flags.is_some();
        if !wants_exec && !creates_memfd {
            return Ok(Verdict::Pass);
        }
        if self.jit_service.is_authorized(&event.binary_blake3) {
            return Ok(Verdict::Pass);
        }
        // TODO (L3): issue the dynamic one-time MMU allocation token over vsock for verified JIT runtimes; the authorized catalog is empty at L2.
        Err(PluginError::WxViolation {
            pid: event.pid,
            operation: format!("syscall {} on {}", trap.syscall_nr, event.binary_path),
        })
    }
}

impl Default for SupplyChainPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for SupplyChainPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "supply-chain"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline.bind_trap(Arc::new(self.clone()));
    }
}

#[async_trait]
impl TrapInterposer for SupplyChainPlugin {
    async fn on_trap(&self, event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        if let Some(verdict) = deny_lifecycle_script(event) {
            return Ok(verdict);
        }
        let rest: &[String] = event.argv.get(1..).unwrap_or(&[]);
        if shannon_entropy(rest) > ENTROPY_FLAG_BITS {
            return Ok(Verdict::Drop {
                errno: Some("EPERM".to_string()),
                reason: "supply-chain: argv entropy above 5.2 bits/char".to_string(),
            });
        }
        Ok(Verdict::Pass)
    }
}

fn deny_lifecycle_script(event: &TrapEvent) -> Option<Verdict> {
    let binary = event.binary_path.rsplit('/').next().unwrap_or("");
    if binary != "npm" && binary != "pip" {
        return None;
    }
    let in_dependency_dir = event
        .argv
        .iter()
        .any(|arg| arg.contains("node_modules") || arg.contains("site-packages"));
    let is_lifecycle = event
        .argv
        .iter()
        .any(|arg| matches!(arg.as_str(), "install" | "preinstall" | "postinstall"));
    if in_dependency_dir && is_lifecycle {
        return Some(Verdict::Drop {
            errno: Some("EPERM".to_string()),
            reason: format!("supply-chain: lifecycle script denied for {binary} (exit 126)"),
        });
    }
    None
}

fn shannon_entropy(argv: &[String]) -> f64 {
    let mut counts = [0u64; 256];
    let mut total = 0u64;
    for arg in argv {
        for byte in arg.as_bytes() {
            counts[*byte as usize] += 1;
            total += 1;
        }
    }
    if total == 0 {
        return 0.0;
    }
    let mut entropy = 0.0;
    for count in counts {
        if count == 0 {
            continue;
        }
        let p = count as f64 / total as f64;
        entropy -= p * p.log2();
    }
    entropy
}
