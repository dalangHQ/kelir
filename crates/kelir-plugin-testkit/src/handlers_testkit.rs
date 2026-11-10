// see: cans/02-plugin-model.md#Plugin-testkit
use std::time::Duration;

use async_trait::async_trait;
use kelir_interface::driver_interface::{
    DriverCapabilities, DriverError, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver,
    WatchdogAction, default_vm_config,
};
use kelir_interface::plugin_model::{HandlerCtx, LifecycleCtx, VetoList};
use kelir_interface::protocol_wire::TrapEvent;

pub fn handler_ctx(session_id: [u8; 16], pid: u32) -> HandlerCtx<(), ()> {
    HandlerCtx::restricted(session_id, pid)
}

pub fn handler_ctx_for_trap(session_id: [u8; 16], event: &TrapEvent) -> HandlerCtx<(), ()> {
    HandlerCtx::restricted(session_id, event.pid).with_trap(event.clone())
}

// see: cans/02-plugin-model.md#Plugin-testkit
// TODO (L3): set_watchdog records on the double; every other primitive stays Unsupported.
pub struct FixtureVmmDriver {
    armed_watchdog: Option<(Duration, WatchdogAction)>,
}

impl FixtureVmmDriver {
    pub fn new() -> Self {
        Self {
            armed_watchdog: None,
        }
    }

    pub fn armed_watchdog(&self) -> Option<(Duration, WatchdogAction)> {
        self.armed_watchdog.clone()
    }
}

impl Default for FixtureVmmDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VmmDriver for FixtureVmmDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "fixture",
            cold_boot_budget_ms: 60,
            rollback_budget_ms: 8,
            fork_budget_ms: 10,
            requires_hardware_vmm: false,
            supports_cow_fork: false,
        }
    }

    async fn boot(&mut self, _config: VmConfig) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("fixture driver boot (L3)".into()))
    }

    async fn set_entropy_backend(&mut self, _backend: EntropyBackend) -> Result<(), DriverError> {
        Err(DriverError::Unsupported(
            "fixture driver set_entropy_backend (L3)".into(),
        ))
    }

    async fn entropy_position(&self) -> Result<EntropyStreamPosition, DriverError> {
        Err(DriverError::Unsupported(
            "fixture driver entropy_position (L3)".into(),
        ))
    }

    async fn restore_entropy_position(
        &mut self,
        _position: EntropyStreamPosition,
    ) -> Result<(), DriverError> {
        Err(DriverError::Unsupported(
            "fixture driver restore_entropy_position (L3)".into(),
        ))
    }

    async fn set_watchdog(
        &mut self,
        timeout: Duration,
        action: WatchdogAction,
    ) -> Result<(), DriverError> {
        self.armed_watchdog = Some((timeout, action));
        Ok(())
    }
}

// see: cans/02-plugin-model.md#Plugin-testkit
pub struct LifecycleCtxFixture {
    config: VmConfig,
    session_id: String,
    driver: FixtureVmmDriver,
    vetoes: VetoList,
}

impl Default for LifecycleCtxFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleCtxFixture {
    pub fn new() -> Self {
        Self {
            config: default_vm_config(None, None, None, None, None, None),
            session_id: String::new(),
            driver: FixtureVmmDriver::new(),
            vetoes: Vec::new(),
        }
    }

    pub fn ctx(&mut self) -> LifecycleCtx<'_> {
        LifecycleCtx::new(
            &self.config,
            &self.session_id,
            &mut self.driver,
            &mut self.vetoes,
        )
    }

    pub fn arm_watchdog(&mut self, action: WatchdogAction) {
        self.driver.armed_watchdog = Some((Duration::ZERO, action));
    }

    pub fn armed_watchdog(&self) -> Option<(Duration, WatchdogAction)> {
        self.driver.armed_watchdog.clone()
    }

    pub fn vetoes(&self) -> &VetoList {
        &self.vetoes
    }
}

pub fn lifecycle_ctx_fixture() -> LifecycleCtxFixture {
    LifecycleCtxFixture::new()
}

pub fn lifecycle_ctx_with_watchdog(action: WatchdogAction) -> LifecycleCtxFixture {
    let mut fixture = LifecycleCtxFixture::new();
    fixture.arm_watchdog(action);
    fixture
}

// see: cans/artifacts/code/plugin-trait.rs#LifecycleCtx
pub fn lifecycle_ctx() -> LifecycleCtxFixture {
    lifecycle_ctx_fixture()
}
