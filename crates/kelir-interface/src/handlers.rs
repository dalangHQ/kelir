// see: cans/artifacts/code/plugin-trait.rs
// see: cans/03-sdk-architecture.md#Rust-SDK
use std::fmt;
use std::marker::PhantomData;

use serde::de::DeserializeOwned;

use crate::plugin::PluginError;
use crate::telemetry::{ExecutionReceipt, ExitKind};

// see: cans/03-sdk-architecture.md#Rust-SDK
pub trait Has<T> {
    fn get(&self) -> T;
}

#[derive(Clone)]
pub struct HandlerCtx<'a, Plugins = (), Params = ()> {
    pub session_id: [u8; 16],
    pub pid: u32,
    plugins: &'a Plugins,
    params_buffer: &'a [u8],
    _params: PhantomData<fn() -> Params>,
}

impl<'a, Plugins, Params> HandlerCtx<'a, Plugins, Params> {
    pub fn new(
        session_id: [u8; 16],
        pid: u32,
        plugins: &'a Plugins,
        params_buffer: &'a [u8],
    ) -> Self {
        Self {
            session_id,
            pid,
            plugins,
            params_buffer,
            _params: PhantomData,
        }
    }

    pub fn ctx<T>(&self) -> T
    where
        Plugins: Has<T>,
    {
        self.plugins.get()
    }

    pub fn params<T: DeserializeOwned>(&self) -> Result<T, PluginError> {
        ciborium::from_reader(self.params_buffer)
            .map_err(|e| PluginError::Halt(format!("EINVAL: params decode failed: {e}")))
    }

    pub fn receipt(&self) -> ExecutionReceipt {
        // TODO: wire to the live per-process receipt in Part 2.
        ExecutionReceipt {
            pid: self.pid,
            binary_path: String::new(),
            binary_blake3: String::new(),
            exit_kind: ExitKind::Normal,
            correlation_token: None,
            dirty_bytes: 0,
            syscall_count: None,
            exit_code: 0,
            wall_clock_ns: 0,
            phase_timings: None,
            egress_bytes: 0,
            egress_dropped_bytes: 0,
            extensions: Vec::new(),
        }
    }
}

impl HandlerCtx<'_, (), ()> {
    pub fn restricted(session_id: [u8; 16], pid: u32) -> Self {
        Self {
            session_id,
            pid,
            plugins: &(),
            params_buffer: &[],
            _params: PhantomData,
        }
    }
}

impl<Plugins, Params> fmt::Debug for HandlerCtx<'_, Plugins, Params> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandlerCtx")
            .field("session_id", &self.session_id)
            .field("pid", &self.pid)
            .finish()
    }
}
