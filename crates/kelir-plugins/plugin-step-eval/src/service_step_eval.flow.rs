// see: cans/plugins/step-eval.md#Plugin-step-eval
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;

use kelir_interface::checkpoint::{CheckpointOps, PidLineage};
use kelir_interface::driver_interface::SnapshotDescriptor;
use kelir_interface::plugin_model::{HandlerCtx, KelirPlugin, MountScope, PluginError};
use kelir_interface::protocol_wire::TrapEvent;
use kelir_interface::telemetry::CausalDelta;

use crate::StepEvalPlugin;
use crate::step_dag::{StepAttribution, StepDag, StepDagConfig, StepDagError, StepDagStorage};

// see: cans/02-plugin-model.md#Facade-blindness
pub use kelir_interface::step_verdict::{PostExecAction, PostExecGate, StepVerdict};

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
pub struct StepSlot {
    slot_id: u64,
    base_descriptor: SnapshotDescriptor,
    pending_delta: Option<CausalDelta>,
}

impl StepSlot {
    pub fn new(slot_id: u64, base_descriptor: SnapshotDescriptor) -> Self {
        Self {
            slot_id,
            base_descriptor,
            pending_delta: None,
        }
    }

    pub fn slot_id(&self) -> u64 {
        self.slot_id
    }

    pub fn base_descriptor(&self) -> &SnapshotDescriptor {
        &self.base_descriptor
    }

    pub fn pending_delta(&self) -> Option<&CausalDelta> {
        self.pending_delta.as_ref()
    }
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.step_eval.step_evaluation.assertions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepEvalAssertions {
    pub assert_binary_blake3: bool,
    pub max_egress_bytes_hermetic: u64,
    pub require_non_zero_delta: bool,
    pub max_duration_seconds: u64,
}

impl Default for StepEvalAssertions {
    fn default() -> Self {
        Self {
            assert_binary_blake3: true,
            max_egress_bytes_hermetic: 0,
            require_non_zero_delta: true,
            max_duration_seconds: 120,
        }
    }
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.step_eval.step_evaluation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepEvaluationConfig {
    pub enabled: bool,
    pub default_action: PostExecAction,
    pub assertions: StepEvalAssertions,
}

impl Default for StepEvaluationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_action: PostExecAction::Proceed,
            assertions: StepEvalAssertions {
                assert_binary_blake3: true,
                max_egress_bytes_hermetic: 0,
                require_non_zero_delta: true,
                max_duration_seconds: 120,
            },
        }
    }
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.step_eval
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepEvalConfig {
    pub sentinel_path: String,
    pub pty_quiescence: bool,
    pub step_dag: StepDagConfig,
    pub step_evaluation: StepEvaluationConfig,
}

impl Default for StepEvalConfig {
    fn default() -> Self {
        Self {
            sentinel_path: "/dev/kelir-gate".to_string(),
            pty_quiescence: true,
            step_dag: StepDagConfig {
                enabled: true,
                require_signed_attribution: true,
                require_separation_of_duties: true,
                storage: StepDagStorage::ContentAddressed,
                reviewer_ttl_seconds: 300,
            },
            step_evaluation: StepEvaluationConfig {
                enabled: true,
                default_action: PostExecAction::Proceed,
                assertions: StepEvalAssertions {
                    assert_binary_blake3: true,
                    max_egress_bytes_hermetic: 0,
                    require_non_zero_delta: true,
                    max_duration_seconds: 120,
                },
            },
        }
    }
}

// see: cans/plugins/step-eval.md#Service-exports-(step-eval)
#[async_trait]
pub trait StepEvaluationService: Send + Sync {
    async fn evaluate(
        &self,
        event: &TrapEvent,
        ctx: &HandlerCtx,
    ) -> Result<StepVerdict, PluginError>;
    async fn dispatch(
        &self,
        verdict: StepVerdict,
        lineage: PidLineage,
    ) -> Result<Option<u64>, PluginError>;
}

// see: cans/plugins/step-eval.md#Service-exports-(step-eval)
pub struct StepEvaluator {
    ops: Arc<dyn CheckpointOps>,
    slots: Mutex<HashMap<PidLineage, StepSlot>>,
    next_slot_id: AtomicU64,
    gates: Vec<Arc<dyn PostExecGate>>,
    dag: Mutex<StepDag>,
    config: StepEvalConfig,
}

impl StepEvaluator {
    // see: cans/plugins/step-eval.md#Service-exports-(step-eval)
    pub fn new(
        ops: Arc<dyn CheckpointOps>,
        config: StepEvalConfig,
        gates: Vec<Arc<dyn PostExecGate>>,
    ) -> Self {
        Self {
            ops,
            slots: Mutex::new(HashMap::new()),
            next_slot_id: AtomicU64::new(0),
            gates,
            dag: Mutex::new(StepDag::new(config.step_dag.clone())),
            config,
        }
    }

    // see: cans/plugins/step-eval.md#Service-exports-(step-eval)
    pub fn config(&self) -> &StepEvalConfig {
        &self.config
    }

    // see: cans/plugins/step-eval.md#Service-exports-(step-eval)
    pub fn gates(&self) -> &[Arc<dyn PostExecGate>] {
        &self.gates
    }

    fn lock_slots(&self) -> Result<MutexGuard<'_, HashMap<PidLineage, StepSlot>>, PluginError> {
        self.slots
            .lock()
            .map_err(|_| PluginError::Halt("step slot lock poisoned".into()))
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn begin_step(
        &self,
        lineage: PidLineage,
        base: SnapshotDescriptor,
    ) -> Result<u64, PluginError> {
        let mut slots = self.lock_slots()?;
        crate::step_slot::create_step(&mut slots, &self.next_slot_id, lineage, base)
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn active_descriptor(
        &self,
        lineage: PidLineage,
    ) -> Result<Option<SnapshotDescriptor>, PluginError> {
        let slots = self.lock_slots()?;
        Ok(crate::step_slot::active_descriptor(&slots, lineage))
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn commit_step(&self, lineage: PidLineage) -> Result<u64, PluginError> {
        let mut slots = self.lock_slots()?;
        crate::step_slot::commit_step(&self.ops, &mut slots, lineage)
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn rollback_step(&self, lineage: PidLineage) -> Result<SnapshotDescriptor, PluginError> {
        let mut slots = self.lock_slots()?;
        crate::step_slot::rollback_step(&mut slots, lineage)
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn branch_from(&self, node_id: u64, lineage: PidLineage) -> Result<(), PluginError> {
        crate::step_slot::branch_from(&self.ops, node_id, lineage)
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn append_step(
        &self,
        parents: Vec<[u8; 32]>,
        actor: String,
        attribution: Option<StepAttribution>,
    ) -> Result<[u8; 32], PluginError> {
        let mut dag = self
            .dag
            .lock()
            .map_err(|_| PluginError::Halt("step dag slot poisoned".into()))?;
        dag.append(parents, actor, attribution)
            .map_err(|error| PluginError::Halt(error.to_string()))
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn step_heads(&self) -> Vec<[u8; 32]> {
        self.dag
            .lock()
            .map(|dag| dag.heads().to_vec())
            .unwrap_or_default()
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn step_merkle_root(&self) -> [u8; 32] {
        self.dag
            .lock()
            .map(|dag| dag.merkle_root())
            .unwrap_or([0; 32])
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn is_gate_path(&self, path: &str) -> bool {
        path == self.config.sentinel_path
    }

    // TODO: [lifecycle_checkpoint]: detect PTY idle quiescence for step boundaries; no case id pins the heuristic yet; slop: step boundaries never detect PTY idle; steps commit on timer instead of quiescence. (deferred to L3)
    pub fn pty_quiescent(&self) -> Result<bool, PluginError> {
        let _ = self.config.pty_quiescence;
        Ok(false)
    }

    // TODO: [lc_03_step_evaluation_taint_rollback]: verify the tainted-step delta (binary BLAKE3, dirty-byte, duration) before commit; slop: tainted step reports failure while the upper file and RAM keep the writes. (deferred to L3)
    pub fn verify_tainted_delta(
        &self,
        base: &SnapshotDescriptor,
        delta: Option<&CausalDelta>,
    ) -> Result<(), PluginError> {
        let _ = (base, delta);
        Ok(())
    }

    fn lock_dag(&self) -> Result<MutexGuard<'_, StepDag>, PluginError> {
        self.dag
            .lock()
            .map_err(|_| PluginError::Halt("step dag slot poisoned".into()))
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn dag_config(&self) -> Result<StepDagConfig, PluginError> {
        Ok(self.lock_dag()?.config().clone())
    }

    // TODO: [lifecycle_checkpoint]: verify reviewer attribution against ApprovalAuth (WebAuthn/TOTP/OIDC method proof, role separation, non-empty reviewer/TTL) per cans/plugins/tool-guard.md#Approval-authorization; slop: reviewer accepted without auth-method proof, self-approval passes (deferred to L3)
    pub fn align_reviewer_with_approval_auth(
        &self,
        attribution: &StepAttribution,
        actor: &str,
    ) -> Result<(), PluginError> {
        crate::step_dag::check_reviewer_attribution(
            attribution,
            actor,
            self.config.step_dag.reviewer_ttl_seconds,
        )
        .map_err(|error| PluginError::Halt(error.to_string()))?;
        Err(PluginError::Halt(
 "reviewer ApprovalAuth verification is deferred to L3".into(),
        ))
    }
}

#[async_trait]
impl StepEvaluationService for StepEvaluator {
    // TODO: [lc_03_step_evaluation_taint_rollback]: the step-evaluation loop calls this per committed step per cans/10-attestation.md#Step-evaluation-loop; slop: step loop runs without per-step evaluation, taint unrevoked (deferred to L3)
    async fn evaluate(
        &self,
        event: &TrapEvent,
        ctx: &HandlerCtx,
    ) -> Result<StepVerdict, PluginError> {
        for gate in &self.gates {
            let action = gate.check(event, ctx).await?;
            if action != PostExecAction::Proceed {
                return Ok(action.to_step_verdict());
            }
        }
        Ok(StepVerdict::Commit)
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    async fn dispatch(
        &self,
        verdict: StepVerdict,
        lineage: PidLineage,
    ) -> Result<Option<u64>, PluginError> {
        match verdict {
            StepVerdict::Commit => {
                let (base, delta) = {
                    let slots = self.lock_slots()?;
                    let Some(slot) = slots.get(&lineage) else {
                        return Err(PluginError::Halt(
                            "no active step slot for PID lineage".into(),
                        ));
                    };
                    (
                        slot.base_descriptor().clone(),
                        slot.pending_delta().cloned(),
                    )
                };
                self.verify_tainted_delta(&base, delta.as_ref())?;
                Ok(Some(self.commit_step(lineage)?))
            }
            StepVerdict::Rollback => {
                self.rollback_step(lineage)?;
                if let Some(head) = self.ops.head(lineage) {
                    self.ops.rollback(head, self.ops.monotonic_watermark_ns())?;
                }
                Ok(None)
            }
            StepVerdict::YieldToHost => Ok(None),
        }
    }
}

// see: cans/plugins/step-eval.md#Service-exports-(step-eval)
#[derive(Clone)]
pub struct StepEvaluationCtx {
    pub evaluator: Arc<StepEvaluator>,
}

impl StepEvaluationCtx {
    // see: cans/plugins/step-eval.md#Service-exports-(step-eval)
    pub fn evaluator(&self) -> &Arc<StepEvaluator> {
        &self.evaluator
    }

    // see: cans/plugins/step-eval.md#Service-exports-(step-eval)
    pub async fn evaluate(
        &self,
        event: &TrapEvent,
        ctx: &HandlerCtx,
    ) -> Result<StepVerdict, PluginError> {
        StepEvaluationService::evaluate(self.evaluator.as_ref(), event, ctx).await
    }
}

// see: cans/plugins/step-eval.md#Error-and-return-semantics-(step-eval)
pub fn dag_error_to_halt(error: StepDagError) -> PluginError {
    PluginError::Halt(error.to_string())
}

impl StepEvalPlugin {
    // see: cans/plugins/step-eval.md#Plugin-step-eval
    pub fn new(
        ops: Arc<dyn kelir_interface::checkpoint::CheckpointOps>,
        config: StepEvalConfig,
    ) -> Self {
        Self {
            evaluator: Arc::new(StepEvaluator::new(ops, config, Vec::new())),
        }
    }

    // see: cans/plugins/step-eval.md#Plugin-step-eval
    pub fn evaluator(&self) -> &Arc<StepEvaluator> {
        &self.evaluator
    }
}

impl KelirPlugin for StepEvalPlugin {
    type Context = StepEvaluationCtx;

    fn context(&self) -> Self::Context {
        StepEvaluationCtx {
            evaluator: Arc::clone(&self.evaluator),
        }
    }

    fn name(&self) -> &'static str {
        "step-eval"
    }

    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}
