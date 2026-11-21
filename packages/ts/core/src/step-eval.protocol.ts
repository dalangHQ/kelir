// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/plugins/step-eval.md#Plugin-step-eval

import type { PidLineage } from "./context.lifecycle.types.flow.js";
import type { SnapshotDescriptor, TrapInfo } from "./types.core.flow.js";

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
export enum PostExecAction {
  Proceed = "proceed",
  RollbackAndHalt = "rollback-and-halt",
}

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
export enum StepVerdict {
  Commit = "commit",
  Rollback = "rollback",
  YieldToHost = "yield-to-host",
}

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
export const SENTINEL_PATH = "/dev/kelir-gate";

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
export const STEP_EVALUATION_YIELD = "step-evaluation-yield";

// see: cans/plugins/step-eval.md#Service-exports-(step-eval)
export interface StepEvalHandle {
  evaluate(event: TrapInfo): Promise<StepVerdict>;
  beginStep(
    lineage: PidLineage,
    descriptor: SnapshotDescriptor,
  ): Promise<number>;
  commitStep(lineage: PidLineage): Promise<number>;
  rollbackStep(lineage: PidLineage): Promise<SnapshotDescriptor>;
}

export interface IntentInput {
  readonly binary: string;
  readonly argv: readonly string[];
  readonly lineage: number;
}

export interface VerificationIntent {
  readonly isHypothesisValid: boolean;
  readonly falsificationReason?: string;
}

export interface PhysicalReceipt {
  readonly id: string;
}

export interface VerificationEvaluator {
  analyzeIntent(input: IntentInput): Promise<VerificationIntent>;
  verifyPhysicalReceipt(receipt: PhysicalReceipt): Promise<boolean>;
}

export type StepEvaluationOutcome = "passed" | "reverted" | "dropped";

export interface StepEvaluationResult {
  readonly outcome: StepEvaluationOutcome;
  readonly reason?: string;
}
