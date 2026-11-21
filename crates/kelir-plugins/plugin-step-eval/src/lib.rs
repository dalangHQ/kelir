// see: cans/plugins/step-eval.md#Plugin-step-eval
#![forbid(unsafe_code)]

use std::sync::Arc;

use kelir_interface::driver_interface::{ExternalSuspendKind, SuspendReason};

#[path = "service_step_eval.flow.rs"]
pub mod service_step_eval;
#[path = "step_attribution.rs"]
pub mod step_attribution;
#[path = "step_dag.flow.rs"]
pub mod step_dag;
#[path = "step_slot.flow.rs"]
pub mod step_slot;

pub use service_step_eval::{
    PostExecAction, PostExecGate, StepEvalAssertions, StepEvalConfig, StepEvaluationConfig,
    StepEvaluationCtx, StepEvaluationService, StepEvaluator, StepSlot, StepVerdict,
};
pub use step_dag::{
    ApprovalAuthError, ApprovalAuthPolicy, StepAttribution, StepDag, StepDagConfig, StepDagError,
    StepDagStorage, StepNode,
};

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
pub const SENTINEL_PATH: &str = "/dev/kelir-gate";

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
pub const STEP_EVALUATION_YIELD: &str = "step-evaluation-yield";

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
pub fn step_evaluation_yield() -> SuspendReason {
    SuspendReason::External(ExternalSuspendKind::External(
        STEP_EVALUATION_YIELD.to_string(),
    ))
}

// see: cans/plugins/step-eval.md#Plugin-step-eval
#[derive(Clone)]
pub struct StepEvalPlugin {
    evaluator: Arc<StepEvaluator>,
}
