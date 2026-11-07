// see: cans/03-sdk-architecture.md#Rust-SDK
// see: cans/02-plugin-model.md#Hook-context-restriction
use std::marker::PhantomData;

use serde::de::DeserializeOwned;

use kelir_interface::plugin::HandlerCtx as IfaceCtx;
use kelir_interface::plugin::PluginError;
use kelir_interface::protocol::TrapEvent;
use kelir_interface::telemetry::ExecutionReceipt;

use crate::hlist::{HCons, HList, HNil, Has, NoParams};
use crate::session::{FsHandle, StepStack, TerminalHandle, VmHandle};

// TODO (L2): interface Has<T> is single-parameter, so implementing it on the accumulator is blocked by E0119/E0207; the indexed hlist search keeps the c.ctx::<T, _>() call infallible and compile-time until interface gains the inferred index parameter.
pub struct TrapCtx<Plugins = HNil, Params = NoParams> {
    pub session_id: [u8; 16],
    pub pid: u32,
    pub trap: TrapEvent,
    pub vm: VmHandle,
    pub fs: FsHandle,
    pub step: StepStack,
    pub terminal: TerminalHandle,
    plugins: Plugins,
    params_buffer: Vec<u8>,
    _params: PhantomData<fn() -> Params>,
}

impl<Plugins, Params> TrapCtx<Plugins, Params> {
    pub fn new(
        session_id: [u8; 16],
        pid: u32,
        trap: TrapEvent,
        plugins: Plugins,
        params_buffer: Vec<u8>,
    ) -> Self {
        Self {
            session_id,
            pid,
            trap,
            // TODO (L3): wire the live session handles into the dispatch view per cans/03-sdk-architecture.md#Session-surfaces.
            vm: VmHandle::default(),
            fs: FsHandle,
            step: StepStack::default(),
            terminal: TerminalHandle,
            plugins,
            params_buffer,
            _params: PhantomData,
        }
    }

    // see: cans/01-architecture.md#Params-ingress-contract
    pub fn base(&self) -> IfaceCtx<'_, Plugins, Params> {
        IfaceCtx::new(
            self.session_id,
            self.pid,
            &self.plugins,
            &self.params_buffer,
        )
    }

    // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    pub fn ctx<T, Index>(&self) -> T
    where
        Plugins: Has<T, Index>,
        T: Clone,
    {
        self.plugins.capability().clone()
    }

    // see: cans/03-sdk-architecture.md#Params-typed-views
    pub fn params<T: DeserializeOwned>(&self) -> Result<T, PluginError> {
        self.base().params()
    }

    pub async fn receipt(&self) -> Result<ExecutionReceipt, PluginError> {
        // TODO (L3): wire to the live per-process receipt.
        Ok(self.base().receipt())
    }

    pub(crate) fn unretag<Params2>(self) -> TrapCtx<Plugins, Params2> {
        let TrapCtx {
            session_id,
            pid,
            trap,
            vm,
            fs,
            step,
            terminal,
            plugins,
            params_buffer,
            _params,
        } = self;
        TrapCtx {
            session_id,
            pid,
            trap,
            vm,
            fs,
            step,
            terminal,
            plugins,
            params_buffer,
            _params: PhantomData,
        }
    }
}

impl<Head, Tail, Params> TrapCtx<HCons<Head, Tail>, Params>
where
    Head: Clone + Send + Sync + 'static,
    Tail: HList,
{
    pub(crate) fn narrow(self) -> TrapCtx<Tail, Params> {
        let TrapCtx {
            session_id,
            pid,
            trap,
            vm,
            fs,
            step,
            terminal,
            plugins,
            params_buffer,
            _params,
        } = self;
        TrapCtx {
            session_id,
            pid,
            trap,
            vm,
            fs,
            step,
            terminal,
            plugins: plugins.tail_value(),
            params_buffer,
            _params: PhantomData,
        }
    }
}

impl<Plugins, Params> std::fmt::Debug for TrapCtx<Plugins, Params> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrapCtx")
            .field("session_id", &self.session_id)
            .field("pid", &self.pid)
            .field("trap", &self.trap)
            .field("vm", &self.vm)
            .field("fs", &self.fs)
            .field("step", &self.step)
            .field("terminal", &self.terminal)
            .finish()
    }
}
