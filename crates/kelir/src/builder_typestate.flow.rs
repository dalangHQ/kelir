// see: cans/artifacts/code/vm-config-defaults.rs
use std::path::PathBuf;

use kelir_interface::driver_interface::DriverPolicy;

use crate::builder_sdk::{Host, KelirBuilder, Unsealed};
use crate::hlist_sdk::HList;

impl<Plugins: HList, Params: 'static, Ctx, Deps: HList>
    KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params>
{
    pub fn vcpus(mut self, vcpus: u8) -> Self {
        if let Some(config) = self.config.as_mut() {
            config.vcpus = vcpus;
        }
        self
    }

    pub fn memory_mb(mut self, memory_mb: u32) -> Self {
        if let Some(config) = self.config.as_mut() {
            config.memory_mb = memory_mb;
        }
        self
    }

    pub fn kernel(mut self, kernel: impl Into<PathBuf>) -> Self {
        if let Some(config) = self.config.as_mut() {
            config.kernel_path = kernel.into();
        }
        self
    }

    pub fn rootfs(mut self, rootfs: impl Into<PathBuf>) -> Self {
        if let Some(config) = self.config.as_mut() {
            config.rootfs_erofs_path = rootfs.into();
        }
        self
    }

    pub fn tsc_locked(mut self, tsc_locked: bool) -> Self {
        if let Some(config) = self.config.as_mut() {
            config.tsc_locked = tsc_locked;
        }
        self
    }

    pub fn watchdog(mut self, watchdog: bool) -> Self {
        if let Some(config) = self.config.as_mut() {
            config.enable_watchdog = watchdog;
        }
        self
    }

    pub fn driver(mut self, driver: DriverPolicy) -> Self {
        if let Some(config) = self.config.as_mut() {
            config.driver_policy = driver;
        }
        self
    }

    pub fn page_size_kb(mut self, page_size_kb: u32) -> Self {
        if let Some(config) = self.config.as_mut() {
            config.page_size_kb = page_size_kb;
        }
        self
    }

    pub fn deterministic_clock(mut self, deterministic_clock: bool) -> Self {
        if let Some(config) = self.config.as_mut() {
            config.deterministic_clock = deterministic_clock;
        }
        self
    }

    pub fn pinned_clocksource(mut self, pinned_clocksource: impl Into<String>) -> Self {
        if let Some(config) = self.config.as_mut() {
            config.pinned_clocksource = Some(pinned_clocksource.into());
        }
        self
    }

    pub fn preemption_grace_ms(mut self, preemption_grace_ms: u64) -> Self {
        if let Some(config) = self.config.as_mut() {
            config.preemption_grace_ms = preemption_grace_ms;
        }
        self
    }

    pub fn network_off(mut self, network_off: bool) -> Self {
        if let Some(config) = self.config.as_mut() {
            config.network_off = network_off;
        }
        self
    }
}
