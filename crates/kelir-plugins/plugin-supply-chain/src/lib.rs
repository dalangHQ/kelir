// see: cans/plugins/supply-chain.md#Plugin-supply-chain
#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use kelir_interface::driver::Stream;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    HandlerCtx, KelirPlugin, PipelineBinder, TrapInterposer,
};
use kelir_interface::protocol::{PROT_EXEC, TrapEvent, TrappedSyscall, Verdict};
use kelir_interface::vsock::{VsockPeer, VsockServiceHandler, VsockServiceSpec};

// see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
pub const WX_VIOLATION_ERRNO: &str = "EWXVIOLATION";

// see: cans/plugins/supply-chain.md#Return-errno-and-rejection-behavior-(supply-chain)
pub const SCRIPT_DENY_ERRNO: &str = "EACCES";

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
pub const WX_ALLOC_VSOCK_PORT: u32 = 5253;

// see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizedJitEngine {
    pub binary_path: String,
    pub binary_blake3: [u8; 32],
}

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WxAllocToken {
    pub nonce: u128,
    pub issued_at_ns: u64,
    pub binary_blake3: [u8; 32],
}

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
impl WxAllocToken {
    pub const WIRE_LEN: usize = 16 + 8 + 32;

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    pub fn wire_bytes(&self) -> [u8; Self::WIRE_LEN] {
        let mut wire = [0u8; Self::WIRE_LEN];
        wire[..16].copy_from_slice(&self.nonce.to_le_bytes());
        wire[16..24].copy_from_slice(&self.issued_at_ns.to_le_bytes());
        wire[24..].copy_from_slice(&self.binary_blake3);
        wire
    }
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
    fn is_authorized(&self, binary_blake3: &[u8; 32]) -> bool;
}

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
struct WxTokenLedger {
    next_nonce: AtomicU64,
    minted: Mutex<Vec<WxAllocToken>>,
}

impl WxTokenLedger {
    fn new() -> Self {
        Self {
            next_nonce: AtomicU64::new(1),
            minted: Mutex::new(Vec::new()),
        }
    }

    fn lock_minted(&self) -> Result<MutexGuard<'_, Vec<WxAllocToken>>, PluginError> {
        self.minted
            .lock()
            .map_err(|_| PluginError::Halt("W^X token ledger poisoned".into()))
    }

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    fn mint(&self, binary_blake3: &[u8; 32]) -> Result<WxAllocToken, PluginError> {
        // TODO (L3): mint the nonce from the host CSPRNG and sign the token before vsock delivery.
        let nonce = u128::from(self.next_nonce.fetch_add(1, Ordering::AcqRel));
        let issued_at_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| PluginError::Halt(format!("host clock before epoch: {e}")))?
            .as_nanos()
            .try_into()
            .map_err(|_| PluginError::Halt("host clock exceeds 64-bit nanoseconds".into()))?;
        let token = WxAllocToken {
            nonce,
            issued_at_ns,
            binary_blake3: *binary_blake3,
        };
        self.lock_minted()?.push(token.clone());
        Ok(token)
    }

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    fn consume(&self, nonce: u128) -> Result<WxAllocToken, PluginError> {
        let mut minted = self.lock_minted()?;
        let index = minted
            .iter()
            .position(|token| token.nonce == nonce)
            .ok_or_else(|| {
                PluginError::reject(
                    WX_VIOLATION_ERRNO,
                    format!("W^X allocation token {nonce:#x} is unknown or already consumed"),
                )
            })?;
        Ok(minted.remove(index))
    }
}

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
struct WxTokenChannel {
    peer: Mutex<Option<VsockPeer>>,
    stream: Mutex<Option<Box<dyn Stream>>>,
    pending: Mutex<Vec<WxAllocToken>>,
}

impl WxTokenChannel {
    fn new() -> Self {
        Self {
            peer: Mutex::new(None),
            stream: Mutex::new(None),
            pending: Mutex::new(Vec::new()),
        }
    }

    fn lock_opt<T>(slot: &Mutex<T>) -> Result<MutexGuard<'_, T>, PluginError> {
        slot.lock()
            .map_err(|_| PluginError::Halt("W^X token channel poisoned".into()))
    }

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    fn register(&self, peer: VsockPeer, stream: Box<dyn Stream>) -> Result<(), PluginError> {
        *Self::lock_opt(&self.peer)? = Some(peer);
        *Self::lock_opt(&self.stream)? = Some(stream);
        self.flush_pending()
    }

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    fn deliver(&self, token: WxAllocToken) -> Result<(), PluginError> {
        Self::lock_opt(&self.pending)?.push(token);
        self.flush_pending()
    }

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    fn flush_pending(&self) -> Result<(), PluginError> {
        let mut pending = Self::lock_opt(&self.pending)?;
        if pending.is_empty() {
            return Ok(());
        }
        let mut stream = Self::lock_opt(&self.stream)?;
        let Some(stream) = stream.as_mut() else {
            return Ok(());
        };
        // TODO (L3): correlate the channel with the trapped task and move token delivery onto the async transport loop.
        for token in pending.iter() {
            let written = stream.write(&token.wire_bytes())?;
            if written != WxAllocToken::WIRE_LEN {
                return Err(PluginError::Halt(format!(
                    "W^X token vsock delivery wrote {written} of {} bytes",
                    WxAllocToken::WIRE_LEN
                )));
            }
        }
        pending.clear();
        Ok(())
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
        // TODO (L3): catalog storage internals; resolve allowed_jit_binaries disk paths to BLAKE3 digests for the verified JIT engine catalog (Node, Bun, V8).
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
        // TODO (L3): the seccomp-unotify mask pins the trapped syscall set; payloads outside the mediated set pass.
        Ok(Verdict::Pass)
    }

    fn mediate_wx_gate(&self, event: &TrapEvent, operation: &str) -> Result<Verdict, PluginError> {
        // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
        if self.state.is_authorized(&event.binary_blake3) {
            // TODO (L3): measure the live calling task beyond the execve digest (ptrace or dynamic-link injection into a verified engine).
            let token = self.state.ledger.mint(&event.binary_blake3)?;
            self.state.channel.deliver(token)?;
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
    fn fork_state(&self, _child_session_id: &str) -> Box<dyn kelir_interface::plugin::AnyPlugin> {
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
        // TODO (L3): read the guest shim handshake on this channel without blocking the async worker.
        self.state.channel.register(peer.clone(), stream)
    }
}

// see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
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
