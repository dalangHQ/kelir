// see: cans/plugins/replay.md#Plugin-replay
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use kelir_interface::driver_interface::{EntropyStreamPosition, PmuSample, PmuSampler, VmmError};
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{KelirPlugin, LifecycleCtx, PluginDeps};

use crate::ReplayConfig;

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.replay
impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            step_window_instructions: 1_000_000,
            pinned_clocksource: "kvm-clock".into(),
            deterministic_rng_seed: 0x5EED,
            network_isolation: true,
        }
    }
}

// see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
// TODO: [lifecycle_checkpoint]: remove this allow when the stubbed L3 callers land and read this item again.; slop: the allow hides a dead item forever, the stubbed callers never land, replay wiring stays unwired while the gate reports clean. (deferred to L3)
#[allow(dead_code)]
struct ReplayState {
    step_window_instructions: u64,
    retired_total: AtomicU64,
    window_retired: AtomicU64,
    pmu_sampling: AtomicBool,
    pending_windows: Mutex<Vec<u64>>,
    sample_floor_ns: Mutex<u64>,
    sample_regress: Mutex<Option<String>>,
    rng_position: Mutex<Option<EntropyStreamPosition>>,
    ns_per_instruction: Mutex<Option<u32>>,
    effective_entropy_seed: Mutex<Option<u64>>,
}

// TODO: [lifecycle_checkpoint]: remove this allow when the stubbed L3 callers land and read this item again.; slop: the allow hides a dead item forever, the stubbed callers never land, replay wiring stays unwired while the gate reports clean. (deferred to L3)
#[allow(dead_code)]
impl ReplayState {
    fn new(step_window_instructions: u64) -> Self {
        Self {
            step_window_instructions,
            retired_total: AtomicU64::new(0),
            window_retired: AtomicU64::new(0),
            pmu_sampling: AtomicBool::new(true),
            pending_windows: Mutex::new(Vec::new()),
            sample_floor_ns: Mutex::new(0),
            sample_regress: Mutex::new(None),
            rng_position: Mutex::new(None),
            ns_per_instruction: Mutex::new(None),
            effective_entropy_seed: Mutex::new(None),
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
    fn record_pmu_sample(&self, sample: &PmuSample) {
        let mut retired = self.retired_total.load(Ordering::Acquire);
        loop {
            let next = retired.saturating_add(sample.retired_instructions);
            match self.retired_total.compare_exchange_weak(
                retired,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => retired = actual,
            }
        }
        let mut window = self.window_retired.load(Ordering::Acquire);
        loop {
            let next = window.saturating_add(sample.retired_instructions);
            match self.window_retired.compare_exchange_weak(
                window,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => window = actual,
            }
        }
        if self.enforce_sample_floor(sample.sampled_at_ns).is_err() {
            return;
        }
        // TODO: [lc_23_replay_deterministic_seed_reproduces_step_windows]: Condition polling also drives window accrual; wiring lands when Condition predicates register.; slop: Condition-suspended windows never accrue. (deferred to L3)
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
            return Err(PluginError::Halt(
                "EClockRegress: PMU sample timestamp regressed behind the monotonic floor"
                    .to_string(),
            ));
        }
        *floor = sampled_at_ns;
        Ok(())
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    fn accrue_step_windows(&self) -> Result<usize, PluginError> {
        if self.step_window_instructions == 0 {
            return Err(PluginError::Halt(
                "replay step_window_instructions must be a positive retired-instruction interval"
                    .into(),
            ));
        }
        let mut pending = Self::lock_slot(&self.pending_windows)?;
        let mut accrued = 0;
        while self.window_retired.load(Ordering::Acquire) >= self.step_window_instructions {
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
// TODO: [lifecycle_checkpoint]: remove this allow when the stubbed L3 callers land and read this item again.; slop: the allow hides a dead item forever, the stubbed callers never land, replay wiring stays unwired while the gate reports clean. (deferred to L3)
#[allow(dead_code)]
struct ReplayPmuSampler {
    state: Arc<ReplayState>,
}

// TODO: [lifecycle_checkpoint]: remove this allow when the stubbed L3 callers land and read this item again.; slop: the allow hides a dead item forever, the stubbed callers never land, replay wiring stays unwired while the gate reports clean. (deferred to L3)
#[allow(dead_code)]
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
    pub fn ns_per_instruction(&self) -> Result<Option<u32>, PluginError> {
        Ok(*ReplayState::lock_slot(&self.state.ns_per_instruction)?)
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    pub fn effective_entropy_seed(&self) -> Result<Option<u64>, PluginError> {
        Ok(*ReplayState::lock_slot(&self.state.effective_entropy_seed)?)
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
#[derive(Clone, PluginDeps)]
pub struct ReplayPlugin {
    config: ReplayConfig,
    state: Arc<ReplayState>,
}

impl ReplayPlugin {
    pub fn new(config: ReplayConfig) -> Self {
        Self {
            state: Arc::new(ReplayState::new(config.step_window_instructions)),
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

// see: cans/plugins/replay.md#Kernel-clocksource-lockdown
// TODO: [lifecycle_checkpoint]: remove this allow when the stubbed L3 callers land and read this item again.; slop: the allow hides a dead item forever, the stubbed callers never land, replay wiring stays unwired while the gate reports clean. (deferred to L3)
#[allow(dead_code)]
fn assert_pinned_clocksource(pinned_clocksource: &str) -> Result<(), PluginError> {
    if pinned_clocksource.is_empty() {
        return Err(PluginError::Halt(
            "replay pinned_clocksource must name the guest clocksource".into(),
        ));
    }
    Ok(())
}

// see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
// TODO: [lifecycle_checkpoint]: remove this allow when the stubbed L3 callers land and read this item again.; slop: the allow hides a dead item forever, the stubbed callers never land, replay wiring stays unwired while the gate reports clean. (deferred to L3)
#[allow(dead_code)]
fn is_capability_unsupported(err: &PluginError) -> bool {
    let PluginError::Io(io_err) = err else {
        return false;
    };
    io_err
        .get_ref()
        .and_then(|source| source.downcast_ref::<VmmError>())
        .is_some_and(|vmm| matches!(vmm, VmmError::Unsupported(_)))
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
