// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/artifacts/code/typescript-accumulation.ts#runtime
// see: cans/10-attestation.md#Step-evaluation-loop
import type { Session } from "./session.js";

export * from "./types.js";
export * from "./schema.js";
export * from "./verdict.js";
export * from "./trap.js";
export * from "./context.js";
export * from "./errors.js";
export * from "./session.js";
export * from "./sandbox.js";
export * from "./plugin.js";
export * from "./builder.js";
export * from "./plugins.js";
export * from "./fusion.js";

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

export async function runStepEvaluationLoop(
  session: Session,
  evaluator: VerificationEvaluator,
): Promise<StepEvaluationResult> {
  void session;
  void evaluator;
  // TODO (L3): pre-step checkpoint, intent analysis, post-condition receipt verification, rollback on failure.
  throw new Error("TODO: runStepEvaluationLoop (L3)");
}
