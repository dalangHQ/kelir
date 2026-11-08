// see: cans/plugins/replay.md#Plugin-replay
#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use async_trait::async_trait;
use kelir_interface::driver::{ClockStepControl, EntropyBackend, EntropyStreamPosition};
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope, PipelineBinder,
};

// see: cans/plugins/replay.md#Error-and-return-semantics-(replay)
const ECLOCKREGRESS: &str = "EClockRegress";

// see: cans/artifacts/examples/replay.rs#ReplayConfig
// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.replay
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    pub step_window: u64,
    pub kernel_params: Vec<String>,
    pub deterministic_rng_seed: u64,
    pub network_isolation: bool,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.replay
impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            step_window: 1_000_000,
            kernel_params: vec![
                "clocksource=kvm-clock".into(),
                "tsc=reliable".into(),
                "nohpet".into(),
            ],
            deterministic_rng_seed: 0x5EED,
            network_isolation: true,
        }
    }
}

// see: cans/plugins/replay.md#Service-exports-(replay)
#[async_trait]
pub trait ReplayService: Send + Sync {
    async fn step_clock_window(&self, retired_instructions: u64) -> Result<(), PluginError>;
}

// see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
struct ReplayState {
    clock: Mutex<Option<Arc<dyn ClockStepControl>>>,
    rng_position: Mutex<Option<EntropyStreamPosition>>,
    monotonic_floor_ns: Mutex<Option<u64>>,
    step_window: u64,
}

impl ReplayState {
    fn new(step_window: u64) -> Self {
        Self {
            clock: Mutex::new(None),
            rng_position: Mutex::new(None),
            monotonic_floor_ns: Mutex::new(None),
            step_window,
        }
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    fn clock_handle(&self) -> Result<Arc<dyn ClockStepControl>, PluginError> {
        let guard = lock_slot(&self.clock)?;
        guard
            .clone()
            .ok_or_else(|| PluginError::Halt("replay clock capability not armed".into()))
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    fn arm(
        &self,
        clock: Arc<dyn ClockStepControl>,
        initial_offset: Duration,
    ) -> Result<(), PluginError> {
        *lock_slot(&self.clock)? = Some(clock);
        *lock_slot(&self.monotonic_floor_ns)? = Some(duration_ns(initial_offset)?);
        Ok(())
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    fn save_rng_position(&self, position: EntropyStreamPosition) -> Result<(), PluginError> {
        *lock_slot(&self.rng_position)? = Some(position);
        Ok(())
    }

    // see: cans/08-lifecycle.md#Rollback
    fn take_rng_position(&self) -> Result<Option<EntropyStreamPosition>, PluginError> {
        Ok(lock_slot(&self.rng_position)?.take())
    }

    // see: cans/plugins/replay.md#Error-and-return-semantics-(replay)
    fn enforce_monotonic_step(&self, before: Duration, after: Duration) -> Result<(), PluginError> {
        let before_ns = duration_ns(before)?;
        let after_ns = duration_ns(after)?;
        let mut floor = lock_slot(&self.monotonic_floor_ns)?;
        let floor_ns = floor.get_or_insert(before_ns);
        if after_ns < *floor_ns {
            return Err(PluginError::reject(
                ECLOCKREGRESS,
                format!("guest clock regressed: {after_ns}ns behind floor {floor_ns}ns"),
            ));
        }
        *floor_ns = after_ns;
        Ok(())
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    fn elapsed_for_window(&self, retired_instructions: u64) -> Result<Duration, PluginError> {
        // TODO (L3): PMU instruction-count substitution converts the retired-instruction window to guest clock nanoseconds.
        Err(PluginError::Halt(format!(
            "PMU instruction-count substitution pending (L3): {retired_instructions} retired instructions in a {} instruction window",
            self.step_window
        )))
    }
}

// see: cans/plugins/replay.md#Plugin-replay
#[derive(Clone)]
pub struct ReplayPlugin {
    config: ReplayConfig,
    state: Arc<ReplayState>,
}

impl ReplayPlugin {
    pub fn new(config: ReplayConfig) -> Self {
        Self {
            state: Arc::new(ReplayState::new(config.step_window)),
            config,
        }
    }
}

impl Default for ReplayPlugin {
    fn default() -> Self {
        Self::new(ReplayConfig::default())
    }
}

// see: cans/plugins/replay.md#Plugin-replay
impl KelirPlugin for ReplayPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "replay"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }

    // see: cans/plugins/replay.md#Service-exports-(replay)
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        let service: Arc<dyn ReplayService> = self.state.clone();
        pipeline.service_registry().register(service);
    }
}

// see: cans/plugins/replay.md#Service-exports-(replay)
#[async_trait]
impl ReplayService for ReplayState {
    async fn step_clock_window(&self, retired_instructions: u64) -> Result<(), PluginError> {
        let delta = self.elapsed_for_window(retired_instructions)?;
        let clock = self.clock_handle()?;
        let before = clock
            .clock_offset()
            .await
            .map_err(|e| PluginError::Halt(format!("guest clock offset read failed: {e}")))?;
        clock
            .step_clock(delta)
            .await
            .map_err(|e| PluginError::Halt(format!("deterministic clock step failed: {e}")))?;
        let after = clock
            .clock_offset()
            .await
            .map_err(|e| PluginError::Halt(format!("guest clock offset read failed: {e}")))?;
        self.enforce_monotonic_step(before, after)
    }
}

// see: cans/plugins/replay.md#Invariants-and-policy-(replay)
#[async_trait]
impl LifecycleParticipant for ReplayPlugin {
    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    async fn on_vm_init(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        // see: cans/plugins/replay.md#Kernel-clocksource-lockdown
        assert_lockdown_params(&self.config.kernel_params)?;
        // see: cans/plugins/replay.md#Invariants-and-policy-(replay)
        if !self.config.network_isolation {
            return Err(PluginError::Halt(
                "replay mode requires network isolation (network: off)".into(),
            ));
        }
        let clock = ctx.clock_step_control()?;
        let entropy = ctx.entropy_control()?;
        entropy
            .set_backend(EntropyBackend::SeededChaCha20 {
                seed: self.config.deterministic_rng_seed,
            })
            .await
            .map_err(|e| {
                PluginError::Halt(format!("seeded entropy backend substitution failed: {e}"))
            })?;
        let initial = clock
            .clock_offset()
            .await
            .map_err(|e| PluginError::Halt(format!("guest clock offset read failed: {e}")))?;
        self.state.arm(clock, initial)?;
        // TODO (L3): capture the deterministic RNG seed in the snapshot manifest at replay start.
        Ok(())
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn on_vm_ready(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        let position =
            ctx.entropy_control()?.position().await.map_err(|e| {
                PluginError::Halt(format!("entropy stream position read failed: {e}"))
            })?;
        self.state.save_rng_position(position)
    }

    // see: cans/08-lifecycle.md#Rollback
    async fn on_pre_commit(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        let position =
            ctx.entropy_control()?.position().await.map_err(|e| {
                PluginError::Halt(format!("entropy stream position read failed: {e}"))
            })?;
        self.state.save_rng_position(position)
    }

    // see: cans/08-lifecycle.md#Rollback
    async fn on_post_commit(
        &self,
        ctx: &mut LifecycleCtx,
        committed: bool,
    ) -> Result<(), PluginError> {
        let saved = self.state.take_rng_position()?;
        if committed {
            return Ok(());
        }
        if let Some(position) = saved {
            ctx.entropy_control()?
                .restore_position(position)
                .await
                .map_err(|e| {
                    PluginError::Halt(format!("entropy stream position restore failed: {e}"))
                })?;
        }
        Ok(())
    }
}

fn lock_slot<T>(slot: &Mutex<Option<T>>) -> Result<MutexGuard<'_, Option<T>>, PluginError> {
    slot.lock()
        .map_err(|_| PluginError::Halt("replay capability slot lock poisoned".into()))
}

// see: cans/plugins/replay.md#Kernel-clocksource-lockdown
fn assert_lockdown_params(params: &[String]) -> Result<(), PluginError> {
    if !params.iter().any(|param| param.starts_with("clocksource=")) {
        return Err(PluginError::Halt(
            "replay kernel lockdown missing clocksource= boot parameter".into(),
        ));
    }
    for required in ["tsc=reliable", "nohpet"] {
        if !params.iter().any(|param| param == required) {
            return Err(PluginError::Halt(format!(
                "replay kernel lockdown missing {required} boot parameter"
            )));
        }
    }
    Ok(())
}

// see: cans/plugins/replay.md#Error-and-return-semantics-(replay)
fn duration_ns(duration: Duration) -> Result<u64, PluginError> {
    u64::try_from(duration.as_nanos())
        .map_err(|_| PluginError::Halt("guest clock offset exceeds 64-bit nanoseconds".into()))
}
