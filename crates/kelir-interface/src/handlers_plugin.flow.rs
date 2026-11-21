// see: cans/artifacts/code/plugin-trait.rs#KelirPlugin
// see: cans/03-sdk-architecture.md#Rust-SDK
// see: cans/02-plugin-model.md#Core-context-keys
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};

use serde::de::DeserializeOwned;

use crate::errno_types::Errno;
use crate::handles_lifecycle::{FsHandle, StepStack, TerminalHandle, VmHandle};
use crate::plugin_model::PluginError;
use crate::protocol_wire::{
    EXEC_STREAM_STDIN, PatchPayload, ProcessLimits, SuspendUntil, TrapEvent, Verdict,
};
use crate::telemetry::ExecutionReceipt;

// see: cans/03-sdk-architecture.md#Rust-SDK
pub trait Has<T, Index = ()> {
    fn get(&self) -> &T;
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
pub trait Capability<P, Index = ()> {
    fn capability(&self) -> Option<&P>;
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
impl<P: 'static> Capability<P> for () {
    fn capability(&self) -> Option<&P> {
        None
    }
}

// see: cans/03-sdk-architecture.md#Session-surfaces
// TODO (L3) [plugin_engine]: surface real session state into SessionContext at the dispatch boundary.; slop: SessionContext carries no live session state, predicates decide blind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionContext {
    pub session_id: [u8; 16],
    pub name: String,
}

// see: cans/02-plugin-model.md#Unified-verdict
type ConditionPredicate = Arc<dyn Fn(&SessionContext) -> bool + Send + Sync>;

// see: cans/02-plugin-model.md#Unified-verdict
// TODO (L2): the condition registry is process-global until the engine owns it per-session.
static CONDITION_PREDICATES: OnceLock<RwLock<HashMap<u64, ConditionPredicate>>> = OnceLock::new();

// see: cans/02-plugin-model.md#Unified-verdict
static NEXT_CONDITION_HANDLE: AtomicU64 = AtomicU64::new(1);

// see: cans/02-plugin-model.md#Unified-verdict
pub fn register_condition_predicate(predicate: ConditionPredicate) -> u64 {
    let handle = NEXT_CONDITION_HANDLE.fetch_add(1, Ordering::Relaxed);
    let registry = CONDITION_PREDICATES.get_or_init(|| RwLock::new(HashMap::new()));
    if let Ok(mut guard) = registry.write() {
        guard.insert(handle, predicate);
    }
    handle
}

// see: cans/02-plugin-model.md#Unified-verdict
pub fn condition_predicate(handle: u64) -> Option<ConditionPredicate> {
    let registry = CONDITION_PREDICATES.get_or_init(|| RwLock::new(HashMap::new()));
    registry
        .read()
        .ok()
        .and_then(|guard| guard.get(&handle).cloned())
}

// see: cans/02-plugin-model.md#Core-context-keys
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FsContext {
    pub inode: u64,
    pub path: PathBuf,
    pub journal: u64,
    // TODO (L2): FUSE read dispatch plumbs the requested offset here — cans/05-storage-plane.md#In-flight-read-masking
    pub offset: u64,
}

// see: cans/02-plugin-model.md#Core-context-keys
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetContext {
    pub frame: Vec<u8>,
    pub stream_id: u32,
    pub socket: u32,
    pub proto: u8,
    pub ip: [u8; 4],
    pub port: u16,
    pub domain: String,
    pub qtype: u16,
}

// see: cans/02-plugin-model.md#Core-context-keys
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostContext {
    pub lsm_event: String,
    pub host_pid: u32,
    pub key: String,
    pub breach_path: PathBuf,
    pub breach_kind: String,
}

// see: cans/02-plugin-model.md#Core-context-keys
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConsoleContext {
    pub pty: u32,
    pub cols: u16,
    pub rows: u16,
}

// see: cans/11-protocol.md#Signal-and-fork-interception
// on_signal traps kill, tkill, tgkill, rt_sigqueueinfo, rt_tgsigqueueinfo,
// and pidfd_send_signal; `syscall_nr` carries the trapping syscall number and
// `target_relation` classifies the target so rules can gate peer/shim kills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalTargetRelation {
    Self_,
    Children,
    Session,
    ShimPid1,
    Peer,
}

// see: cans/11-protocol.md#Signal-and-fork-interception
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignalContext {
    pub sender: u32,
    pub target: u32,
    pub sig: i32,
    pub syscall_nr: i64,
    pub target_relation: Option<SignalTargetRelation>,
}

// see: cans/11-protocol.md#Signal-and-fork-interception
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForkContext {
    pub parent: u32,
    pub child: u32,
}

// see: cans/02-plugin-model.md#Core-context-keys
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecContext {
    pub pid: u32,
    pub exec_id: u64,
    pub stream_id: u8,
    pub chunk: Vec<u8>,
    pub rlimits: Option<ProcessLimits>,
}

#[derive(Clone)]
pub struct HandlerCtx<Plugins = (), Params = ()> {
    pub session_id: [u8; 16],
    pub pid: u32,
    pub trap: TrapEvent,
    pub vm: VmHandle,
    pub fs: FsHandle,
    pub step: StepStack,
    pub terminal: TerminalHandle,
    pub fs_ctx: Option<FsContext>,
    pub net_ctx: Option<NetContext>,
    pub host_ctx: Option<HostContext>,
    pub console_ctx: Option<ConsoleContext>,
    pub exec_ctx: Option<ExecContext>,
    pub signal_ctx: Option<SignalContext>,
    pub fork_ctx: Option<ForkContext>,
    pub receipt: Option<ExecutionReceipt>,
    pub operator_lease: Option<String>,
    plugins: Plugins,
    params_buffer: Vec<u8>,
    _params: PhantomData<fn() -> Params>,
}

impl<Plugins, Params> HandlerCtx<Plugins, Params> {
    // L3-GAP: [plugin_engine]: wire the live session handles into the dispatch view per cans/03-sdk-architecture.md#Session-surfaces.; slop: empty dispatch view, live handles never wired
    pub fn new(session_id: [u8; 16], pid: u32, plugins: Plugins, params_buffer: Vec<u8>) -> Self {
        Self {
            session_id,
            pid,
            trap: TrapEvent::default(),
            vm: VmHandle::default(),
            fs: FsHandle,
            step: StepStack::default(),
            terminal: TerminalHandle,
            fs_ctx: None,
            net_ctx: None,
            host_ctx: None,
            console_ctx: None,
            exec_ctx: None,
            signal_ctx: None,
            fork_ctx: None,
            receipt: None,
            operator_lease: None,
            plugins,
            params_buffer,
            _params: PhantomData,
        }
    }

    pub fn with_operator_lease(mut self, token: impl Into<String>) -> Self {
        self.operator_lease = Some(token.into());
        self
    }

    // TODO (L3) [governance_tools]: verify the ed25519 signature on the stored operator lease token; presence-only until then; slop: unverified lease tokens count as valid; presence-only check.
    pub fn has_valid_operator_lease(&self) -> bool {
        self.operator_lease.is_some()
    }

    pub fn with_trap(mut self, trap: TrapEvent) -> Self {
        self.trap = trap;
        self
    }

    pub fn with_vm(mut self, vm: VmHandle) -> Self {
        self.vm = vm;
        self
    }

    pub fn with_fs_ctx(mut self, ctx: FsContext) -> Self {
        self.fs_ctx = Some(ctx);
        self
    }

    pub fn with_net_ctx(mut self, ctx: NetContext) -> Self {
        self.net_ctx = Some(ctx);
        self
    }

    pub fn with_host_ctx(mut self, ctx: HostContext) -> Self {
        self.host_ctx = Some(ctx);
        self
    }

    pub fn with_console_ctx(mut self, ctx: ConsoleContext) -> Self {
        self.console_ctx = Some(ctx);
        self
    }

    pub fn with_exec_ctx(mut self, ctx: ExecContext) -> Self {
        self.exec_ctx = Some(ctx);
        self
    }

    // see: cans/11-protocol.md#Signal-and-fork-interception
    pub fn with_signal(mut self, ctx: SignalContext) -> Self {
        self.signal_ctx = Some(ctx);
        self
    }

    // see: cans/11-protocol.md#Signal-and-fork-interception
    pub fn with_signal_syscall(
        mut self,
        sender: u32,
        target: u32,
        sig: i32,
        syscall_nr: i64,
        target_relation: SignalTargetRelation,
    ) -> Self {
        self.signal_ctx = Some(SignalContext {
            sender,
            target,
            sig,
            syscall_nr,
            target_relation: Some(target_relation),
        });
        self
    }

    // see: cans/11-protocol.md#Signal-and-fork-interception
    pub fn with_fork(mut self, ctx: ForkContext) -> Self {
        self.fork_ctx = Some(ctx);
        self
    }

    // see: cans/07-wire-plane.md#Hook-points:-on-net-outbound,-on-net-inbound,-on-socket-connect
    pub fn with_socket_connect(mut self, proto: u8, ip: [u8; 4], port: u16) -> Self {
        self.net_ctx = Some(NetContext {
            proto,
            ip,
            port,
            ..NetContext::default()
        });
        self
    }

    // see: cans/07-wire-plane.md#Additional-net-hooks:-on-dns-query,-on-socket-bind
    pub fn with_dns_query(mut self, domain: impl Into<String>, qtype: u16) -> Self {
        self.net_ctx = Some(NetContext {
            domain: domain.into(),
            qtype,
            ..NetContext::default()
        });
        self
    }

    // see: cans/07-wire-plane.md#Additional-net-hooks:-on-dns-query,-on-socket-bind
    pub fn with_socket_bind(mut self, proto: u8, ip: [u8; 4], port: u16) -> Self {
        self.net_ctx = Some(NetContext {
            proto,
            ip,
            port,
            ..NetContext::default()
        });
        self
    }

    // see: cans/11-protocol.md#Binary-exec-channel
    pub fn with_exec_stdin(mut self, pid: u32, exec_id: u64, chunk: Vec<u8>) -> Self {
        self.exec_ctx = Some(ExecContext {
            pid,
            exec_id,
            stream_id: EXEC_STREAM_STDIN,
            chunk,
            rlimits: None,
        });
        self
    }

    // see: cans/11-protocol.md#Host-containment-verification
    pub fn with_host_containment_breach(
        mut self,
        pid: u32,
        path: impl Into<PathBuf>,
        kind: impl Into<String>,
    ) -> Self {
        self.host_ctx = Some(HostContext {
            host_pid: pid,
            breach_path: path.into(),
            breach_kind: kind.into(),
            ..HostContext::default()
        });
        self
    }

    // see: cans/10-attestation.md#Execution-receipts
    pub fn with_receipt(mut self, receipt: ExecutionReceipt) -> Self {
        self.receipt = Some(receipt);
        self
    }

    pub fn retag<Params2>(self) -> HandlerCtx<Plugins, Params2> {
        HandlerCtx {
            session_id: self.session_id,
            pid: self.pid,
            trap: self.trap,
            vm: self.vm,
            fs: self.fs,
            step: self.step,
            terminal: self.terminal,
            fs_ctx: self.fs_ctx,
            net_ctx: self.net_ctx,
            host_ctx: self.host_ctx,
            console_ctx: self.console_ctx,
            exec_ctx: self.exec_ctx,
            signal_ctx: self.signal_ctx,
            fork_ctx: self.fork_ctx,
            receipt: self.receipt,
            operator_lease: self.operator_lease,
            plugins: self.plugins,
            params_buffer: self.params_buffer,
            _params: PhantomData,
        }
    }

    pub fn map_plugins<Plugins2>(
        self,
        f: impl FnOnce(Plugins) -> Plugins2,
    ) -> HandlerCtx<Plugins2, Params> {
        HandlerCtx {
            session_id: self.session_id,
            pid: self.pid,
            trap: self.trap,
            vm: self.vm,
            fs: self.fs,
            step: self.step,
            terminal: self.terminal,
            fs_ctx: self.fs_ctx,
            net_ctx: self.net_ctx,
            host_ctx: self.host_ctx,
            console_ctx: self.console_ctx,
            exec_ctx: self.exec_ctx,
            signal_ctx: self.signal_ctx,
            fork_ctx: self.fork_ctx,
            receipt: self.receipt,
            operator_lease: self.operator_lease,
            plugins: f(self.plugins),
            params_buffer: self.params_buffer,
            _params: PhantomData,
        }
    }

    pub fn cap<P: 'static>(&self) -> Option<&P>
    where
        Plugins: Capability<P>,
    {
        self.plugins.capability()
    }

    // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    pub fn cap_infallible<P: 'static>(&self) -> &P
    where
        Plugins: Has<P>,
    {
        self.plugins.get()
    }

    // see: cans/02-plugin-model.md#Static-dispatch
    pub fn erased(&self) -> HandlerCtx<(), ()> {
        let mut erased = HandlerCtx::new(self.session_id, self.pid, (), self.params_buffer.clone())
            .with_trap(self.trap.clone());
        if let Some(fs_ctx) = &self.fs_ctx {
            erased = erased.with_fs_ctx(fs_ctx.clone());
        }
        if let Some(net_ctx) = &self.net_ctx {
            erased = erased.with_net_ctx(net_ctx.clone());
        }
        if let Some(host_ctx) = &self.host_ctx {
            erased = erased.with_host_ctx(host_ctx.clone());
        }
        if let Some(console_ctx) = &self.console_ctx {
            erased = erased.with_console_ctx(console_ctx.clone());
        }
        if let Some(exec_ctx) = &self.exec_ctx {
            erased = erased.with_exec_ctx(exec_ctx.clone());
        }
        if let Some(signal_ctx) = &self.signal_ctx {
            erased = erased.with_signal(signal_ctx.clone());
        }
        if let Some(fork_ctx) = &self.fork_ctx {
            erased = erased.with_fork(fork_ctx.clone());
        }
        if let Some(receipt) = &self.receipt {
            erased = erased.with_receipt(receipt.clone());
        }
        if let Some(lease) = &self.operator_lease {
            erased = erased.with_operator_lease(lease.clone());
        }
        erased
    }

    pub fn plugins(&self) -> &Plugins {
        &self.plugins
    }

    pub fn params<T: DeserializeOwned>(&self) -> Result<T, PluginError> {
        ciborium::from_reader(self.params_buffer.as_slice())
            .map_err(|e| PluginError::Halt(format!("EINVAL: params decode failed: {e}")))
    }

    // see: cans/03-sdk-architecture.md#Params-typed-views
    pub fn with_params_buffer(mut self, buffer: Vec<u8>) -> Self {
        self.params_buffer = buffer;
        self
    }

    // see: cans/03-sdk-architecture.md#Unified-verdict-surface
    pub fn pass(&self) -> Verdict {
        Verdict::Pass
    }

    pub fn patch(&self, payload: PatchPayload) -> Verdict {
        Verdict::Patch(payload)
    }

    pub fn drop(&self, errno: Errno, reason: String) -> Verdict {
        Verdict::Drop {
            errno,
            reason,
            guidance: None,
        }
    }

    pub fn suspend(&self, until: SuspendUntil) -> Verdict {
        Verdict::Suspend { until }
    }

    // see: cans/02-plugin-model.md#Unified-verdict
    pub fn suspend_until_condition(
        &self,
        poll_interval_ms: u64,
        predicate: impl Fn(&SessionContext) -> bool + Send + Sync + 'static,
    ) -> Verdict {
        let handle = register_condition_predicate(Arc::new(predicate));
        Verdict::Suspend {
            until: SuspendUntil::Condition {
                poll_interval_ms,
                handle,
            },
        }
    }

    pub fn abort(&self, reason: String) -> Verdict {
        Verdict::Abort {
            reason,
            guidance: None,
        }
    }

    // see: cans/10-attestation.md#Execution-receipts
    // L3-GAP: [ar_01_execution_receipt_blake3_binding]: no live producer mints receipts into the store yet; dispatch hydrates once the trap/exit boundary stores one; slop: receipt() Err, stored receipt never minted.
    pub async fn receipt(&self) -> Result<ExecutionReceipt, PluginError> {
        match &self.receipt {
            Some(receipt) => Ok(receipt.clone()),
            None => Err(PluginError::Halt(
                "ENOENT: no execution receipt bound to this handler context".into(),
            )),
        }
    }
}

impl HandlerCtx<(), ()> {
    pub fn restricted(session_id: [u8; 16], pid: u32) -> Self {
        Self::new(session_id, pid, (), Vec::new())
    }
}

impl<Plugins, Params> fmt::Debug for HandlerCtx<Plugins, Params> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandlerCtx")
            .field("session_id", &self.session_id)
            .field("pid", &self.pid)
            .field("trap", &self.trap)
            .finish()
    }
}
