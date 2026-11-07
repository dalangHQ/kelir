// see: cans/plugins/replay.md
#![forbid(unsafe_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope, PipelineBinder, VetoEvent,
};

#[derive(Debug, Clone)]
pub struct ReplayConfig {
    pub step_window_instructions: u64,
    pub deterministic_rng_seed: Option<u64>,
    pub kernel_params: Vec<String>,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            step_window_instructions: 1_000_000,
            deterministic_rng_seed: Some(0x5EED),
            kernel_params: vec![
                "clocksource=kvm-clock".to_string(),
                "tsc=reliable".to_string(),
                "nohpet".to_string(),
            ],
        }
    }
}

#[derive(Debug)]
pub struct ReplayService {
    pub config: ReplayConfig,
    last_monotonic_ns: AtomicU64,
}

impl ReplayService {
    pub fn new(config: ReplayConfig) -> Self {
        Self {
            config,
            last_monotonic_ns: AtomicU64::new(0),
        }
    }

    pub fn check_drift(&self, now_monotonic_ns: u64) -> Result<(), PluginError> {
        let last = self.last_monotonic_ns.load(Ordering::SeqCst);
        if now_monotonic_ns < last {
            return Err(PluginError::Halt(format!(
                "replay: negative time drift {now_monotonic_ns} < {last} (EClockRegress)"
            )));
        }
        self.last_monotonic_ns
            .store(now_monotonic_ns, Ordering::SeqCst);
        Ok(())
    }
}

impl Clone for ReplayService {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            last_monotonic_ns: AtomicU64::new(self.last_monotonic_ns.load(Ordering::SeqCst)),
        }
    }
}

impl Default for ReplayService {
    fn default() -> Self {
        Self::new(ReplayConfig::default())
    }
}

#[derive(Clone)]
pub struct ReplayPlugin {
    pub service: ReplayService,
}

impl ReplayPlugin {
    pub fn new(config: ReplayConfig) -> Self {
        Self {
            service: ReplayService::new(config),
        }
    }
}

impl Default for ReplayPlugin {
    fn default() -> Self {
        Self::new(ReplayConfig::default())
    }
}

impl KelirPlugin for ReplayPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "replay"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline
            .service_registry()
            .register(Arc::new(self.service.clone()));
    }
}

#[async_trait]
impl LifecycleParticipant for ReplayPlugin {
    async fn on_vm_init(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        // TODO (L3): arm host PMU retired-instruction counters and advance guest KVM_SET_CLOCK/CNTVOFF_EL2 per step window; the driver linkage is VmConfig deterministic_clock + entropy_seed + skew_clock.
        ctx.register_veto(VetoEvent::ClockSkew, Box::new(|| true));
        Ok(())
    }
}
