// see: cans/plugins/replay.md#Plugin-replay
#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use kelir_interface::driver_interface::{
    EntropyBackend, EntropyStreamPosition, PmuSample, PmuSampler, VmmError,
};
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{KelirPlugin, LifecycleCtx, LifecycleParticipant};

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

// see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
struct ReplayState {
    step_window: u64,
    retired_total: AtomicU64,
    window_retired: AtomicU64,
    pmu_sampling: AtomicBool,
    pending_windows: Mutex<Vec<u64>>,
    sample_floor_ns: Mutex<u64>,
    sample_regress: Mutex<Option<String>>,
    rng_position: Mutex<Option<EntropyStreamPosition>>,
}

impl ReplayState {
    fn new(step_window: u64) -> Self {
        Self {
            step_window,
            retired_total: AtomicU64::new(0),
            window_retired: AtomicU64::new(0),
            pmu_sampling: AtomicBool::new(true),
            pending_windows: Mutex::new(Vec::new()),
            sample_floor_ns: Mutex::new(0),
            sample_regress: Mutex::new(None),
            rng_position: Mutex::new(None),
        }
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    fn mark_pmu_degraded(&self) {
        self.pmu_sampling.store(false, Ordering::Release);
    }

    fn lock_slot<T>(slot: &Mutex<T>) -> Result<MutexGuard<'_, T>, PluginError> {
        slot.lock()
            .map_err(|_| PluginError::Halt("replay capability slot lock poisoned".into()))
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    #[allow(deprecated)]
    fn record_pmu_sample(&self, sample: &PmuSample) {
        self.retired_total
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |total| {
                Some(total.saturating_add(sample.retired_instructions))
            })
            .ok();
        self.window_retired
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |window| {
                Some(window.saturating_add(sample.retired_instructions))
            })
            .ok();
        if self.enforce_sample_floor(sample.sampled_at_ns).is_err() {
            return;
        }
        let _ = self.accrue_step_windows();
    }

    // see: cans/plugins/replay.md#Error-and-return-semantics-(replay)
    fn enforce_sample_floor(&self, sampled_at_ns: u64) -> Result<(), PluginError> {
        let mut floor = Self::lock_slot(&self.sample_floor_ns)?;
        if sampled_at_ns < *floor {
            let mut regress = Self::lock_slot(&self.sample_regress)?;
            *regress = Some(format!(
                "PMU sample timestamp regressed: {sampled_at_ns}ns behind floor {floor}ns"
            ));
            return Err(PluginError::reject(
                ECLOCKREGRESS,
                "PMU sample timestamp regressed behind the monotonic floor",
            ));
        }
        *floor = sampled_at_ns;
        Ok(())
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    fn accrue_step_windows(&self) -> Result<usize, PluginError> {
        if self.step_window == 0 {
            return Err(PluginError::Halt(
                "replay step_window must be a positive retired-instruction interval".into(),
            ));
        }
        let mut pending = Self::lock_slot(&self.pending_windows)?;
        let mut accrued = 0;
        while self.window_retired.load(Ordering::Acquire) >= self.step_window {
            let retired_in_window = self.window_retired.swap(0, Ordering::AcqRel);
            pending.push(retired_in_window);
            accrued += 1;
        }
        Ok(accrued)
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    fn take_due_windows(&self) -> Result<Vec<u64>, PluginError> {
        if let Some(regress) = Self::lock_slot(&self.sample_regress)?.take() {
            return Err(PluginError::Halt(regress));
        }
        let mut pending = Self::lock_slot(&self.pending_windows)?;
        let due = pending.clone();
        pending.clear();
        Ok(due)
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    fn restore_pending_windows(&self, windows: Vec<u64>) -> Result<(), PluginError> {
        let mut pending = Self::lock_slot(&self.pending_windows)?;
        let mut restored = windows;
        restored.append(&mut pending);
        *pending = restored;
        Ok(())
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    fn save_rng_position(&self, position: EntropyStreamPosition) -> Result<(), PluginError> {
        *Self::lock_slot(&self.rng_position)? = Some(position);
        Ok(())
    }

    // see: cans/08-lifecycle.md#Rollback
    fn take_rng_position(&self) -> Result<Option<EntropyStreamPosition>, PluginError> {
        Ok(Self::lock_slot(&self.rng_position)?.take())
    }
}

// see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
struct ReplayPmuSampler {
    state: Arc<ReplayState>,
}

impl ReplayPmuSampler {
    fn new(state: Arc<ReplayState>) -> Self {
        Self { state }
    }
}

// see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
impl PmuSampler for ReplayPmuSampler {
    fn on_pmu_sample(&self, sample: &PmuSample) {
        self.state.record_pmu_sample(sample);
    }
}

// see: cans/plugins/replay.md#Plugin-replay
#[derive(Clone)]
pub struct ReplayCtx {
    state: Arc<ReplayState>,
}

impl ReplayCtx {
    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    pub fn retired_instructions(&self) -> u64 {
        self.state.retired_total.load(Ordering::Acquire)
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    pub fn pmu_sampling_active(&self) -> bool {
        self.state.pmu_sampling.load(Ordering::Acquire)
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    pub fn pending_step_windows(&self) -> Result<usize, PluginError> {
        Ok(ReplayState::lock_slot(&self.state.pending_windows)?.len())
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    pub async fn step_due_windows(&self, ctx: &mut LifecycleCtx<'_>) -> Result<u64, PluginError> {
        let due = self.state.take_due_windows()?;
        for (index, retired_instructions) in due.iter().enumerate() {
            if let Err(err) = ctx.step_clock_window(*retired_instructions).await {
                let unstepped: Vec<u64> = due.iter().skip(index).copied().collect();
                self.state.restore_pending_windows(unstepped)?;
                return Err(err);
            }
        }
        Ok(due.len() as u64)
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

    // see: cans/plugins/replay.md#Service-exports-(replay)
    pub fn ctx(&self) -> ReplayCtx {
        ReplayCtx {
            state: Arc::clone(&self.state),
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
    type Context = ReplayCtx;
    fn context(&self) -> Self::Context {
        self.ctx()
    }
    fn name(&self) -> &'static str {
        "replay"
    }
    // see: cans/plugins/replay.md#Service-exports-(replay)
    fn fork_state(
        &self,
        _child_session_id: &str,
    ) -> Box<dyn kelir_interface::plugin_model::AnyPlugin> {
        // see: cans/08-lifecycle.md#CoW-fork
        Box::new(Self::new(self.config.clone()))
    }
}

// see: cans/plugins/replay.md#Invariants-and-policy-(replay)
#[async_trait]
impl LifecycleParticipant for ReplayPlugin {
    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    async fn on_vm_init(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        // see: cans/plugins/replay.md#Kernel-clocksource-lockdown
        assert_lockdown_params(&self.config.kernel_params)?;
        if self.config.step_window == 0 {
            return Err(PluginError::Halt(
                "replay step_window must be a positive retired-instruction interval".into(),
            ));
        }
        // see: cans/plugins/replay.md#Invariants-and-policy-(replay)
        if !self.config.network_isolation {
            return Err(PluginError::Halt(
                "replay mode requires network isolation (network: off)".into(),
            ));
        }
        // see: cans/plugins/replay.md#Deterministic-RNG
        if let Err(err) = ctx
            .set_entropy_backend(EntropyBackend::SeededChaCha20 {
                seed: self.config.deterministic_rng_seed,
            })
            .await
        {
            return Err(PluginError::Halt(format!(
                "replay seeded entropy backend unavailable: {err}"
            )));
        }
        // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
        if let Err(err) =
            ctx.register_pmu_sampler(Arc::new(ReplayPmuSampler::new(Arc::clone(&self.state))))
        {
            if !is_capability_unsupported(&err) {
                return Err(err);
            }
            self.state.mark_pmu_degraded();
        }
        // TODO (L3): capture the deterministic RNG seed in the snapshot manifest at replay start.
        Ok(())
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn on_vm_ready(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        let position = ctx.entropy_position().await?;
        self.state.save_rng_position(position)
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    async fn on_host_preempt(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        self.ctx().step_due_windows(ctx).await?;
        Ok(())
    }

    // see: cans/08-lifecycle.md#Rollback
    async fn on_pre_commit(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        self.ctx().step_due_windows(ctx).await?;
        let position = ctx.entropy_position().await?;
        self.state.save_rng_position(position)
    }

    // see: cans/08-lifecycle.md#Rollback
    async fn on_post_commit(
        &self,
        ctx: &mut LifecycleCtx<'_>,
        committed: bool,
    ) -> Result<(), PluginError> {
        let saved = self.state.take_rng_position()?;
        if committed {
            return Ok(());
        }
        if let Some(position) = saved {
            ctx.restore_entropy_position(position).await?;
        }
        Ok(())
    }
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

// see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
fn is_capability_unsupported(err: &PluginError) -> bool {
    let PluginError::Io(io_err) = err else {
        return false;
    };
    io_err
        .get_ref()
        .and_then(|source| source.downcast_ref::<VmmError>())
        .is_some_and(|vmm| matches!(vmm, VmmError::Unsupported(_)))
}
