// see: cans/03-sdk-architecture.md#Rust-SDK
use std::marker::PhantomData;

use kelir_interface::plugin::PluginError;
use kelir_interface::protocol::TrapEvent;
use kelir_interface::telemetry::ExecutionReceipt;

use crate::session::{FsHandle, StepStack, TerminalHandle, VmHandle};

pub trait Has<T> {
    fn get(&self) -> &T;
}

#[derive(Debug, Clone)]
pub struct HandlerCtx<Plugins = (), Params = ()> {
    pub trap: TrapEvent,
    pub vm: VmHandle,
    pub fs: FsHandle,
    pub step: StepStack,
    pub terminal: TerminalHandle,
    _phantom: PhantomData<(Plugins, Params)>,
}

impl<Plugins, Params> Default for HandlerCtx<Plugins, Params> {
    fn default() -> Self {
        Self {
            trap: TrapEvent {
                seq: 0,
                pid: 0,
                ppid: 0,
                task_id: 0,
                binary_path: String::new(),
                binary_dev: 0,
                binary_ino: 0,
                binary_blake3: [0; 32],
                argv: Vec::new(),
                envp: std::collections::HashMap::new(),
                cwd: String::new(),
            },
            vm: VmHandle::default(),
            fs: FsHandle,
            step: StepStack::default(),
            terminal: TerminalHandle,
            _phantom: PhantomData,
        }
    }
}

impl<Plugins, Params> HandlerCtx<Plugins, Params> {
    pub fn ctx<T: Default>(&self) -> T {
        // TODO (L3): resolve T infallibly via Plugins: Has<T>; T::default() is a placeholder until the Plugins HList lands.
        T::default()
    }

    pub async fn receipt(&self) -> Result<ExecutionReceipt, PluginError> {
        Ok(kelir_interface::plugin::HandlerCtx::new([0; 16], self.trap.pid).receipt())
    }
}
