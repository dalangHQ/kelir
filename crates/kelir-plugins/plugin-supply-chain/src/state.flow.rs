// see: cans/plugins/supply-chain.md#Plugin-supply-chain
use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::driver_interface::Stream;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    HandlerCtx, KelirPlugin, NoDeps, PipelineBinder, PluginDeps, TrapInterposer,
};
use kelir_interface::protocol_wire::{PROT_EXEC, TrapEvent, TrappedSyscall, Verdict};
use kelir_interface::vsock_mesh::{VsockPeer, VsockServiceHandler, VsockServiceSpec};

use crate::wx_token::{WxAllocToken, WxTokenChannel, WxTokenLedger};
use crate::{
    JitValidationService, PackageGuardConfig, SupplyChainConfig, WX_ALLOC_VSOCK_PORT,
    SCRIPT_DENY_ERRNO, WX_VIOLATION_ERRNO,
};

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

// see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
struct SupplyChainState {
    config: SupplyChainConfig,
    ledger: WxTokenLedger,
    channel: WxTokenChannel,
}

impl SupplyChainState {
    fn new(config: SupplyChainConfig) -> Self {
        Self {
            config,
            ledger: WxTokenLedger::new(),
            channel: WxTokenChannel::new(),
        }
    }

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    fn is_authorized(&self, binary_blake3: &[u8; 32]) -> bool {
        // TODO (L3) [supply_chain_security]: catalog storage internals; resolve allowed_jit_binaries disk paths to BLAKE3 digests for the verified JIT engine catalog (Node, Bun, V8); slop: paths resolved by file name, swapped binary passes catalog
        self.config
            .wx_jit
            .allowed_jit_binaries
            .iter()
            .any(|engine| engine.binary_blake3 == *binary_blake3)
    }
}

// see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
#[derive(Clone)]
pub struct SupplyChainCtx {
    state: Arc<SupplyChainState>,
}

impl SupplyChainCtx {
    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    pub fn consume_wx_token(&self, nonce: u128) -> Result<WxAllocToken, PluginError> {
        self.state.ledger.consume(nonce)
    }
}

// see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
impl JitValidationService for SupplyChainCtx {
    fn is_authorized(&self, binary_blake3: &[u8; 32]) -> bool {
        self.state.is_authorized(binary_blake3)
    }
}

#[derive(Clone)]
pub struct SupplyChainPlugin {
    state: Arc<SupplyChainState>,
}

impl SupplyChainPlugin {
    pub fn new(config: SupplyChainConfig) -> Self {
        Self {
            state: Arc::new(SupplyChainState::new(config)),
        }
    }

    // see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
    pub fn ctx(&self) -> SupplyChainCtx {
        SupplyChainCtx {
            state: Arc::clone(&self.state),
        }
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
        if self.state.config.package_guard.deny_lifecycle_scripts
            && self.blocks_lifecycle_script(event)?
        {
            // see: cans/plugins/supply-chain.md#Return-errno-and-rejection-behavior-(supply-chain)
            return Ok(Verdict::Drop {
                errno: Some(SCRIPT_DENY_ERRNO),
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
            if self.state.config.wx_jit.trap_mprotect_exec && prot & PROT_EXEC != 0 {
                return self.mediate_wx_gate(event, "mprotect(PROT_EXEC)");
            }
            return Ok(Verdict::Pass);
        }
        if let Some(flags) = syscall.memfd_create_flags {
            if self.state.config.wx_jit.trap_memfd {
                return self.mediate_wx_gate(event, &format!("memfd_create(flags={flags:#x})"));
            }
            return Ok(Verdict::Pass);
        }
        // TODO (L3) [supply_chain_security]: the seccomp-unotify mask pins the trapped syscall set; payloads outside the mediated set pass; slop: mask pinned narrow, syscalls outside set pass unmediated
        Ok(Verdict::Pass)
    }

    fn mediate_wx_gate(&self, event: &TrapEvent, operation: &str) -> Result<Verdict, PluginError> {
        // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
        if self.state.is_authorized(&event.binary_blake3) {
            // TODO (L3) [supply_chain_security]: measure the live calling task beyond the execve digest (ptrace or dynamic-link injection into a verified engine); slop: digest-only measurement, live task unmeasured, injected code undetected
            let token = self.state.ledger.mint(&event.binary_blake3)?;
            self.state.channel.deliver(token)?;
            return Ok(Verdict::Pass);
        }
        // see: cans/plugins/supply-chain.md#Return-errno-and-rejection-behavior-(supply-chain)
        Ok(Verdict::Drop {
            errno: Some(WX_VIOLATION_ERRNO),
            reason: format!("PID {} attempted {operation}", event.pid),
        })
    }

    fn flags_obfuscated_argv(&self, argv: &[String]) -> Result<bool, PluginError> {
        // TODO (L3) [sc_02_argv_high_entropy_obfuscation_kill]: Shannon entropy of argv[1:] above shannon_entropy_threshold_bits flags encrypted or base64-obfuscated commands; slop: flags_obfuscated_argv stays Ok(false), high-entropy payload Passes
        let _ = argv;
        Ok(false)
    }

    fn blocks_lifecycle_script(&self, event: &TrapEvent) -> Result<bool, PluginError> {
        // TODO (L3) [sc_01_lifecycle_script_trap_exit126]: package-manager process-tree monitoring over the declared managers; dependency-directory lifecycle-script detection; nested npm→sh→node traps at the sh ancestor; slop: blocks_lifecycle_script stays Ok(false), lifecycle scripts Pass, exit never 126
        let _ = event;
        Ok(false)
    }

    fn is_bypass_runner(&self, binary_path: &str) -> bool {
        // TODO (L3) [supply_chain_security]: bypass runner matching covers language toolchain layouts beyond file-name equality; slop: file-name matching only, toolchain layouts bypass the runner check
        std::path::Path::new(binary_path)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                self.state
                    .config
                    .package_guard
                    .bypass_runners
                    .iter()
                    .any(|runner| runner.as_str() == name)
            })
    }
}

// see: cans/plugins/supply-chain.md#Plugin-supply-chain
impl KelirPlugin for SupplyChainPlugin {
    // see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
    type Context = SupplyChainCtx;
    fn context(&self) -> Self::Context {
        self.ctx()
    }
    fn name(&self) -> &'static str {
        "supply-chain"
    }
    // see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
    fn fork_state(
        &self,
        _child_session_id: &str,
    ) -> Box<dyn kelir_interface::plugin_model::AnyPlugin> {
        // see: cans/08-lifecycle.md#CoW-fork
        Box::new(Self::new(self.state.config.clone()))
    }

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline.bind_trap_scoped(Arc::new(self.clone()), self.default_scope());
        let service: Arc<dyn VsockServiceHandler> = Arc::new(WxTokenVsockService {
            state: Arc::clone(&self.state),
        });
        pipeline.bind_vsock_service_scoped(
            VsockServiceSpec {
                port: WX_ALLOC_VSOCK_PORT,
                service: "wx-alloc".into(),
                schema: None,
            },
            service,
            self.default_scope(),
        );
    }
}

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
struct WxTokenVsockService {
    state: Arc<SupplyChainState>,
}

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
#[async_trait]
impl VsockServiceHandler for WxTokenVsockService {
    async fn on_connect(
        &self,
        peer: &VsockPeer,
        stream: Box<dyn Stream>,
        _ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        // TODO (L3) [supply_chain_security]: read the guest shim handshake on this channel without blocking the async worker; slop: shim handshake read blocks worker, async loop stalls
        self.state.channel.register(peer.clone(), stream)
    }
}

// see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
#[async_trait]
impl TrapInterposer for SupplyChainPlugin {
    async fn on_trap(&self, event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        // see: cans/plugins/supply-chain.md#Entropy-analysis
        if !self.is_bypass_runner(&event.binary_path) && self.flags_obfuscated_argv(&event.argv)? {
            // TODO (L3) [sc_02_argv_high_entropy_obfuscation_kill]: flag-to-verdict policy for encrypted or base64-obfuscated argv above the entropy threshold; slop: flag-to-verdict unimplemented, obfuscated argv Passes despite entropy
            return Ok(Verdict::Drop {
                errno: Some(SCRIPT_DENY_ERRNO),
                reason: format!("obfuscated argv denied: {}", event.binary_path),
            });
        }
        match &event.syscall {
            None => self.mediate_execve(event),
            Some(syscall) => self.mediate_memory_syscall(event, syscall),
        }
    }
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
impl PluginDeps for SupplyChainPlugin {
    type Deps = NoDeps;
}
