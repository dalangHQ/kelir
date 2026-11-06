// Handler context passed to trap handlers: the trap event plus the
// handles a handler may use (vm / fs / step stack / terminal).
use std::marker::PhantomData;

use kelir_interface::errors::PluginError;
use kelir_interface::protocol::TrapEvent;
use kelir_interface::telemetry::ExecutionReceipt;

use crate::session::{FsHandle, StepStack, TerminalHandle, VmHandle};

/// `HandlerCtx<P = (), C = ()>`: the `P`/`C` parameters exist so
/// call sites may write `HandlerCtx<_, _>`; the stub does not use them.
#[derive(Debug, Clone)]
pub struct HandlerCtx<P = (), C = ()> {
    pub trap: TrapEvent,
    pub vm: VmHandle,
    pub fs: FsHandle,
    pub step: StepStack,
    pub terminal: TerminalHandle,
    _phantom: PhantomData<(P, C)>,
}

impl<P, C> Default for HandlerCtx<P, C> {
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

impl<P, C> HandlerCtx<P, C> {
    pub fn ctx<T: Default>(&self) -> T {
        T::default()
    }

    /// The live per-process receipt; the stub returns the interface's
    /// default receipt (zero bytes, no extensions).
    pub async fn receipt(&self) -> Result<ExecutionReceipt, PluginError> {
        Ok(kelir_interface::plugin::HandlerCtx::new([0; 16], self.trap.pid).receipt())
    }
}
