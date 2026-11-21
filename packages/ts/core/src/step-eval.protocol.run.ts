// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/plugins/step-eval.md#Plugin-step-eval

import type { Session } from "./session.lifecycle.flow.js";
import type {
  StepEvaluationResult,
  VerificationEvaluator,
} from "./step-eval.protocol.js";
import { EConfig } from "./errors.sdk.flow.js";

// TODO (L3) [lc_03_step_evaluation_taint_rollback]: pre-step checkpoint, intent analysis, post-condition receipt verification, rollback on failure.; slop: step passes without RAM/upper revert check
export async function runStepEvaluationLoop(
  session: Session,
  evaluator: VerificationEvaluator,
): Promise<StepEvaluationResult> {
  void session;
  void evaluator;
  throw new EConfig("TODO: runStepEvaluationLoop");
}
