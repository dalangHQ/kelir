// see: cans/artifacts/code/plugin-trait.rs#KelirPlugin
// see: cans/03-sdk-architecture.md#Rust-SDK
// see: cans/02-plugin-model.md#Core-context-keys
use std::fmt;
use std::marker::PhantomData;
use std::path::PathBuf;

use serde::de::DeserializeOwned;

use crate::errno_types::Errno;
use crate::handles_lifecycle::{FsHandle, StepStack, TerminalHandle, VmHandle};
use crate::plugin_model::PluginError;
use crate::protocol_wire::{
    FilterAction, PatchPayload, ProcessLimits, SuspendUntil, TrapEvent, Verdict,
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
}

// see: cans/02-plugin-model.md#Core-context-keys
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostContext {
    pub lsm_event: String,
    pub host_pid: u32,
    pub key: String,
}

// see: cans/02-plugin-model.md#Core-context-keys
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConsoleContext {
    pub pty: u32,
    pub cols: u16,
    pub rows: u16,
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
    pub receipt: Option<ExecutionReceipt>,
    plugins: Plugins,
    params_buffer: Vec<u8>,
    _params: PhantomData<fn() -> Params>,
}

impl<Plugins, Params> HandlerCtx<Plugins, Params> {
    // TODO (L3) [plugin_engine]: wire the live session handles into the dispatch view per cans/03-sdk-architecture.md#Session-surfaces.; slop: empty dispatch view, live handles never wired
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
            receipt: None,
            plugins,
            params_buffer,
            _params: PhantomData,
        }
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
            receipt: self.receipt,
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
            receipt: self.receipt,
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
        if let Some(receipt) = &self.receipt {
            erased = erased.with_receipt(receipt.clone());
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

    pub fn drop(&self, errno: Option<Errno>, reason: String) -> Verdict {
        Verdict::Drop { errno, reason }
    }

    pub fn suspend(&self, until: SuspendUntil) -> Verdict {
        Verdict::Suspend { until }
    }

    pub fn abort(&self, reason: String) -> Verdict {
        Verdict::Abort { reason }
    }

    // see: cans/11-protocol.md#Verdict-method-mapping
    pub fn forward(&self) -> FilterAction {
        FilterAction::Forward
    }

    pub fn filter_drop(&self, errno: Option<Errno>, reason: String) -> FilterAction {
        FilterAction::Drop { errno, reason }
    }

    pub fn filter_suspend(&self, until: SuspendUntil) -> FilterAction {
        FilterAction::Suspend { until }
    }

    // see: cans/10-attestation.md#Execution-receipts
    // TODO (L3) [ar_01_execution_receipt_blake3_binding]: no live producer mints receipts into the store yet; dispatch hydrates once the trap/exit boundary stores one; slop: receipt() Err, stored receipt never minted.
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
