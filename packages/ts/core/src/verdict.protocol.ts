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
  Suspend({ until }: { until: SuspendUntil }): Verdict;
  Abort({ reason }: { reason?: string }): Verdict;
} = {
  Pass: { kind: "pass" },
  Drop(errno: string, reason?: string): Verdict {
    return { kind: "drop", errno, reason: reason ?? errno };
  },
  Patch(attr: PatchInput): Verdict {
    return { kind: "patch", patch: attr };
  },
  Suspend({ until }: { until: SuspendUntil }): Verdict {
    return { kind: "suspend", until };
  },
  Abort({ reason }: { reason?: string }): Verdict {
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
  Suspend({ until }: { until: SuspendUntil }): FilterAction;
  Abort({ reason }: { reason?: string }): FilterAction;
} = {
  Forward: { kind: "forward" },
  Drop(errno?: string, reason?: string): FilterAction {
    return { kind: "drop", errno, reason: reason ?? errno ?? "drop" };
  },
  Suspend({ until }: { until: SuspendUntil }): FilterAction {
    return { kind: "suspend", until };
  },
  Abort({ reason }: { reason?: string }): FilterAction {
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

// see: cans/artifacts/code/vmm-driver-trait.rs#SuspendReason
export type ExternalSuspendKind =
  | "human-authorization-hold"
  | "wire-connection-parking"
  | "step-evaluation-yield"
  | "host-power-event-snapshot";

// see: cans/artifacts/code/vmm-driver-trait.rs#SuspendReason
export type SuspendReason =
  | { readonly kind: "external"; readonly detail: ExternalSuspendKind }
  | { readonly kind: "resource-pressure" };

// see: cans/artifacts/code/vmm-driver-trait.rs#SuspendReason
export const SuspendReason: {
  readonly resourcePressure: SuspendReason;
  external(detail: ExternalSuspendKind): SuspendReason;
} = {
  resourcePressure: { kind: "resource-pressure" },
  external(detail: ExternalSuspendKind): SuspendReason {
    return { kind: "external", detail };
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
