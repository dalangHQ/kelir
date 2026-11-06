// see: cans/03-sdk-architecture.md#TypeScript-SDK
// Verdict lattice, filter actions, suspend targets, lifecycle and veto
// events. Each name is both a type and a value (interface + const).

import type { PatchInput } from "./types.js";

export interface Verdict {
  readonly kind: "pass" | "drop" | "patch" | "suspend" | "abort";
  readonly reason?: string;
  readonly patch?: PatchInput;
  readonly until?: SuspendUntil;
}

export const Verdict: {
  readonly Pass: Verdict;
  Drop(errno: string, reason?: string): Verdict;
  Patch(attr: PatchInput): Verdict;
  Suspend(until: SuspendUntil): Verdict;
  Abort(reason?: string): Verdict;
} = {
  Pass: { kind: "pass" },
  Drop(errno: string, reason?: string): Verdict {
    return { kind: "drop", reason: reason ?? errno };
  },
  Patch(attr: PatchInput): Verdict {
    return { kind: "patch", patch: attr };
  },
  Suspend(until: SuspendUntil): Verdict {
    return { kind: "suspend", until };
  },
  Abort(reason?: string): Verdict {
    return { kind: "abort", reason };
  },
};

export interface FilterAction {
  readonly kind: "forward" | "suspend";
  readonly until?: SuspendUntil;
}

export const FilterAction: {
  readonly Forward: FilterAction;
  Suspend(until: SuspendUntil): FilterAction;
} = {
  Forward: { kind: "forward" },
  Suspend(until: SuspendUntil): FilterAction {
    return { kind: "suspend", until };
  },
};

export interface SuspendUntil {
  readonly kind: "deadline" | "approval";
  readonly deadlineMs?: number;
}

export const SuspendUntil: {
  readonly approval: SuspendUntil;
  deadline(ms: number): SuspendUntil;
} = {
  approval: { kind: "approval" },
  deadline(ms: number): SuspendUntil {
    return { kind: "deadline", deadlineMs: ms };
  },
};

export enum PostExecAction {
  Proceed = "proceed",
  RollbackAndHalt = "rollback-and-halt",
}

export enum VetoEvent {
  Suspend = "suspend",
  ClockSkew = "clock-skew",
}

export enum LifecycleEvent {
  VmInit = "vm:init",
  VmReady = "vm:ready",
  VmShutdown = "vm:shutdown",
  VmSuspend = "vm:suspend",
  VmResume = "vm:resume",
}
