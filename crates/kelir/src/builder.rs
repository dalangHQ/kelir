use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use kelir_interface::driver::{VmConfig, WatchdogAction};
use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{AnyPlugin, MountScope, VetoEvent};
use kelir_interface::protocol::TrapEvent;

use crate::HandlerCtx;
use crate::planes::storage::LockBrokerConfig;
use crate::planes::terminal::PtyConfig;
use crate::planes::wire::WireConfig;
use crate::plugin::KelirPlugin;
use crate::session::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostExecAction {
    Proceed,
    RollbackAndHalt,
}

#[derive(Debug, Clone)]
pub struct SyntheticFile {
    pub path: PathBuf,
}

impl SyntheticFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

pub trait CompositePlugin {
    fn unpack(self: Box<Self>, parent: &MountScope) -> Vec<(Box<dyn AnyPlugin>, MountScope)>;
}

#[derive(Debug, Clone, Default)]
pub struct TrapFilter {
    pub binary: Option<String>,
    pub args: Vec<String>,
}

impl From<&str> for TrapFilter {
    fn from(value: &str) -> Self {
        Self {
            binary: if value == "*" {
                None
            } else {
                Some(value.to_string())
            },
            args: Vec::new(),
        }
    }
}

impl TrapFilter {
    pub fn binary(binary: &str) -> Self {
        Self {
            binary: Some(binary.to_string()),
            args: Vec::new(),
        }
    }

    pub fn with_arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }
}

pub fn handler<F, Fut, A, O>(f: F) -> F
where
    F: Fn(A) -> Fut,
    Fut: Future<Output = Result<O, Box<dyn std::error::Error + Send + Sync>>>,
{
    f
}

#[derive(Debug, Clone, Default)]
pub struct DriverHandle;

impl DriverHandle {
    pub async fn set_watchdog(
        &self,
        _timeout: Duration,
        _action: WatchdogAction,
    ) -> Result<(), PluginError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct LifecycleMut;

impl LifecycleMut {
    pub fn register_veto(
        &mut self,
        _event: VetoEvent,
        _callback: Box<dyn Fn() -> bool + Send + Sync>,
    ) {
    }
}

#[derive(Debug, Default)]
pub struct KelirBuilder;

impl KelirBuilder {
    pub fn new(_config: VmConfig) -> Self {
        Self
    }

    pub fn terminal(self, _config: PtyConfig) -> Self {
        self
    }

    pub fn storage(self, _config: crate::planes::storage::StorageConfig) -> Self {
        self
    }

    pub fn wire(self, _config: WireConfig) -> Self {
        self
    }

    pub fn lock_broker(self, _config: LockBrokerConfig) -> Self {
        self
    }

    pub fn plugin<P: KelirPlugin>(self, _plugin: P) -> Self {
        self
    }

    pub fn trap<F, Fut, O>(self, _filter: impl Into<TrapFilter>, _handler: F) -> Self
    where
        F: Fn(HandlerCtx) -> Fut,
        Fut: Future<Output = Result<O, Box<dyn std::error::Error + Send + Sync>>>,
    {
        self
    }

    pub fn on_vm_init<F, Fut>(self, _f: F) -> Self
    where
        F: Fn(DriverHandle) -> Fut,
        Fut: Future<Output = Result<(), PluginError>>,
    {
        self
    }

    pub fn on_vm_ready<F, Fut, T>(self, _f: F) -> Self
    where
        F: Fn(&Session) -> Fut,
        Fut: Future<Output = Result<T, PluginError>>,
    {
        self
    }

    pub fn on_vm_shutdown<F, Fut, T>(self, _f: F) -> Self
    where
        F: Fn(&Session) -> Fut,
        Fut: Future<Output = Result<T, PluginError>>,
    {
        self
    }

    pub fn on_trap_deny<F, Fut>(self, _f: F) -> Self
    where
        F: Fn(&TrapEvent, &Session) -> Fut,
        Fut: Future<Output = Result<(), PluginError>>,
    {
        self
    }

    pub fn register_custom_action<F, Fut>(
        self,
        _name: &str,
        _schema: serde_json::Value,
        _handler: F,
    ) -> Self
    where
        F: Fn(ciborium::value::Value) -> Fut,
        Fut: Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    {
        self
    }

    pub fn register_vsock_service<F, Fut>(
        self,
        _kind: u16,
        _schema: serde_json::Value,
        _handler: F,
    ) -> Self
    where
        F: Fn(Vec<u8>) -> Fut,
        Fut: Future<Output = Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>>,
    {
        self
    }

    pub fn lifecycle_mut(&mut self) -> LifecycleMut {
        LifecycleMut
    }

    pub fn params<T: 'static>(self) -> Self {
        let _ = std::any::type_name::<T>();
        self
    }

    pub fn ctx<T: 'static>(&self) -> Option<&T> {
        let _ = self;
        // see: cans/03-sdk-architecture.md#Rust-SDK
        None
    }

    pub fn with_composite<C: CompositePlugin + 'static>(
        self,
        _bundle: C,
        _scope: MountScope,
    ) -> Self {
        self
    }

    pub fn seal(self) -> Self {
        self
    }

    pub async fn boot(self) -> Result<Session, PluginError> {
        Ok(Session::default())
    }
}
