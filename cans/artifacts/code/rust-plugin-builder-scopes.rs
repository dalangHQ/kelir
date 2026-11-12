// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the unified builder scope surface; the live shape is
// crates/kelir/src/builder_scopes.rs and the normative contract is
// cans/02-plugin-model.md#Unified-builder.
//
// Target shape: ONE scope struct per ring group, eight in all. Each
// scope OWNS the builder: it holds the builder by value and every
// scope method delegates to the matching flat builder verb, returning
// the scope; .end() returns the builder and closes the scope. A scope
// entry closure on the builder takes a scope and returns the builder:
// .trap_scope(|s| s.on_trap(f).end()). The scope structs here do not
// borrow the hook vecs; that borrowed shape is deleted. The builder
// itself, its flat verbs, and the closure adapters are pinned in
// cans/artifacts/code/rust-plugin-builder.rs; the two files pin one
// surface, and the scope entry points live on the builder there.
//
// Owned by: cans/02-plugin-model.md#Unified-builder
// see: cans/02-plugin-model.md#Unified-builder
// see: cans/03-sdk-architecture.md#Unified-builder-surface
// see: cans/artifacts/code/rust-plugin-builder.rs

use std::future::Future;
use std::pin::Pin;
use std::path::Path;

use kelir_interface::driver_interface::{WatchdogEvent, WatchdogVerdict};
use kelir_interface::plugin_model::{
    HandlerCtx, LifecycleCtx, PluginError, VetoEvent,
};
use kelir_interface::protocol_wire::{
    FilterAction, Heartbeat, ProcessExit, TrapEvent, Verdict,
};

// see: cans/02-plugin-model.md#Unified-builder
pub struct TrapScope {
    pub(crate) builder: KelirBuilder,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct FsScope {
    pub(crate) builder: KelirBuilder,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct NetScope {
    pub(crate) builder: KelirBuilder,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct ConsoleScope {
    pub(crate) builder: KelirBuilder,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct ExecScope {
    pub(crate) builder: KelirBuilder,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct HostScope {
    pub(crate) builder: KelirBuilder,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct LifecycleScope {
    pub(crate) builder: KelirBuilder,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct TelemetryScope {
    pub(crate) builder: KelirBuilder,
}

// see: cans/02-plugin-model.md#Unified-builder
impl TrapScope {
    pub fn on_trap<F>(mut self, f: F) -> Self
    where
        F: Fn(&TrapEvent, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_trap(f);
        self
    }

    pub fn on_process_exit<F>(mut self, f: F) -> Self
    where
        F: Fn(&ProcessExit, &HandlerCtx) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_process_exit(f);
        self
    }

    pub fn end(self) -> KelirBuilder {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Unified-builder
impl FsScope {
    pub fn on_fs_lookup<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_fs_lookup(f);
        self
    }

    pub fn on_fs_read<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_fs_read(f);
        self
    }

    pub fn on_fs_write<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, &mut [u8], u32, &HandlerCtx) -> Result<FilterAction, PluginError>
            + Send
            + Sync
            + 'static,
    {
        self.builder = self.builder.on_fs_write(f);
        self
    }

    pub fn on_fs_lock<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, i32, i16, u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_fs_lock(f);
        self
    }

    pub fn end(self) -> KelirBuilder {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Unified-builder
impl NetScope {
    pub fn on_net_outbound<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_net_outbound(f);
        self
    }

    pub fn on_net_inbound<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_net_inbound(f);
        self
    }

    pub fn end(self) -> KelirBuilder {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Unified-builder
impl ConsoleScope {
    pub fn on_console_input<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_console_input(f);
        self
    }

    pub fn on_console_output<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_console_output(f);
        self
    }

    pub fn end(self) -> KelirBuilder {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Unified-builder
impl ExecScope {
    pub fn on_exec_stdout<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_exec_stdout(f);
        self
    }

    pub fn on_exec_stderr<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_exec_stderr(f);
        self
    }

    pub fn end(self) -> KelirBuilder {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Unified-builder
// on_host_preempt is reachable via .host() but binds the lifecycle ring.
impl HostScope {
    pub fn on_host_lsm_exec<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_host_lsm_exec(f);
        self
    }

    pub fn on_host_pressure<F>(mut self, f: F) -> Self
    where
        F: Fn(u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_host_pressure(f);
        self
    }

    pub fn on_host_keyring_access<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_host_keyring_access(f);
        self
    }

    pub fn on_host_preempt<F>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(
                &'c mut LifecycleCtx<'i>,
            )
                -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
            + Send
            + Sync
            + 'static,
    {
        self.builder = self.builder.on_host_preempt(f);
        self
    }

    pub fn end(self) -> KelirBuilder {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Unified-builder
impl LifecycleScope {
    pub fn on_vm_init<F>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(
                &'c mut LifecycleCtx<'i>,
            )
                -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
            + Send
            + Sync
            + 'static,
    {
        self.builder = self.builder.on_vm_init(f);
        self
    }

    pub fn on_vm_ready<F>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(
                &'c mut LifecycleCtx<'i>,
            )
                -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
            + Send
            + Sync
            + 'static,
    {
        self.builder = self.builder.on_vm_ready(f);
        self
    }

    pub fn on_vm_shutdown<F>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(
                &'c mut LifecycleCtx<'i>,
            )
                -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
            + Send
            + Sync
            + 'static,
    {
        self.builder = self.builder.on_vm_shutdown(f);
        self
    }

    pub fn on_pre_commit<F>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(
                &'c mut LifecycleCtx<'i>,
            )
                -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
            + Send
            + Sync
            + 'static,
    {
        self.builder = self.builder.on_pre_commit(f);
        self
    }

    pub fn on_post_commit<F>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(
                &'c mut LifecycleCtx<'i>,
                bool,
            )
                -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
            + Send
            + Sync
            + 'static,
    {
        self.builder = self.builder.on_post_commit(f);
        self
    }

    pub fn register_veto(
        mut self,
        event: VetoEvent,
        callback: Box<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        self.builder.registration.vetoes.push((event, callback));
        self
    }

    pub fn end(self) -> KelirBuilder {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Unified-builder
impl TelemetryScope {
    pub fn on_memory_pressure<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(u32, u32) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_memory_pressure(f);
        self
    }

    pub fn on_heartbeat<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&Heartbeat) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_heartbeat(f);
        self
    }

    pub fn on_watchdog<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&WatchdogEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WatchdogVerdict, PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_watchdog(f);
        self
    }

    pub fn end(self) -> KelirBuilder {
        self.builder
    }
}
