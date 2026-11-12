// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type { PatchInput } from "./types.core.js";

export type Verdict =
  | { readonly kind: "pass" }
  | { readonly kind: "drop"; readonly errno?: string; readonly reason: string }
  | { readonly kind: "patch"; readonly patch: PatchInput }
  | { readonly kind: "suspend"; readonly until: SuspendUntil }
  | { readonly kind: "abort"; readonly reason?: string };

export const Verdict: {
  readonly Pass: Verdict;
  Drop(errno: string, reason?: string): Verdict;
  Patch(attr: PatchInput): Verdict;
  Suspend(until: SuspendUntil): Verdict;
  Abort(reason?: string): Verdict;
} = {
  Pass: { kind: "pass" },
  Drop(errno: string, reason?: string): Verdict {
    return { kind: "drop", errno, reason: reason ?? errno };
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

// see: cans/02-plugin-model.md#Unified-verdict
export type FilterAction =
  | { readonly kind: "forward" }
  | { readonly kind: "drop"; readonly errno?: string; readonly reason: string }
  | { readonly kind: "suspend"; readonly until: SuspendUntil }
  | { readonly kind: "abort"; readonly reason?: string };

export const FilterAction: {
  readonly Forward: FilterAction;
  Drop(errno?: string, reason?: string): FilterAction;
  Suspend(until: SuspendUntil): FilterAction;
  Abort(reason?: string): FilterAction;
} = {
  Forward: { kind: "forward" },
  Drop(errno?: string, reason?: string): FilterAction {
    return { kind: "drop", errno, reason: reason ?? errno ?? "drop" };
  },
  Suspend(until: SuspendUntil): FilterAction {
    return { kind: "suspend", until };
  },
  Abort(reason?: string): FilterAction {
    return { kind: "abort", reason };
  },
};

// see: cans/11-protocol.md#Wire-messages
export type SuspendUntil =
  | { readonly kind: "deadline"; readonly deadlineMs: number }
  | { readonly kind: "approval" }
  | { readonly kind: "handshake" };

export const SuspendUntil: {
  readonly approval: SuspendUntil;
  readonly handshake: SuspendUntil;
  deadline(ms: number): SuspendUntil;
} = {
  approval: { kind: "approval" },
  handshake: { kind: "handshake" },
  deadline(ms: number): SuspendUntil {
    return { kind: "deadline", deadlineMs: ms };
  },
};

export enum PostExecAction {
  Proceed = "proceed",
  RollbackAndHalt = "rollback-and-halt",
}

export enum VetoEvent {
  Fork = "fork",
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
