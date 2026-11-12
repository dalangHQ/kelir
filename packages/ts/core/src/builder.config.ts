// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type { TrapContext } from "./context.lifecycle.types.js";

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export interface KelirConfig {
  readonly vcpus?: number;
  readonly memoryMb?: number;
  readonly pageSizeKb?: number;
  readonly deterministicClock?: boolean;
  readonly pinnedClocksource?: string;
  readonly preemptionGraceMs?: number;
  readonly driver?: string;
  readonly kernel?: string;
  readonly rootfsErofs?: string;
  readonly tscLocked?: boolean;
  readonly enableWatchdog?: boolean;
  readonly detached?: boolean;
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
export interface TerminalStream {
  write(data: string): void;
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
export interface TerminalConfig {
  readonly cols?: number;
  readonly rows?: number;
  readonly dropOsc52?: boolean;
  readonly stripTitle?: boolean;
  readonly tokenShield?: {
    readonly maxBytesTotal: number;
    readonly maxLinesTotal: number;
    readonly clampStrategy: string;
    readonly onClamp?: (stats: {
      droppedBytes: number;
      droppedLines: number;
    }) => string;
  };
  readonly prompts?: readonly {
    readonly match: RegExp;
    readonly timeoutMs: number;
    readonly verifyForeground: boolean;
    readonly action: (
      prompt: unknown,
      stream: TerminalStream,
    ) => Promise<void> | void;
  }[];
  readonly streamEngine?: string;
}

// see: cans/05-storage-plane.md#Workspace-commit
export interface StorageConfig<Ctx = unknown> {
  readonly idmap?: { readonly guestUid: number; readonly hostUid: number };
  readonly sharedCaches?: readonly string[];
  readonly lockBroker?: boolean;
  readonly syntheticFiles?: readonly {
    readonly path: string;
    readonly content: string | (() => string);
    readonly mode?: number;
  }[];
  readonly maskRead?: readonly {
    readonly glob: string;
    readonly pattern: RegExp;
    readonly replace: string;
  }[];
  readonly snoops?: readonly {
    readonly path: string;
    readonly sync: boolean;
    readonly onWrite: (
      path: string,
      raw: Uint8Array,
      c: TrapContext & Ctx,
    ) => Promise<void> | void;
  }[];
  readonly encapsulation?: string;
  readonly scanEngine?: string;
  readonly carryoverBytes?: number;
}

// see: cans/07-wire-plane.md#DNS-policy-filter
export interface WireConfig {
  readonly sinkhole?: { readonly dnsExcept: readonly string[] };
  readonly sshAgentBridge?: boolean;
  readonly vaults?: readonly {
    readonly name: string;
    readonly kind: "bearer_token" | "sigv4" | "mtls_bundle";
    readonly audience: string;
    readonly placeholder: string;
  }[];
  readonly l4Proxies?: readonly {
    readonly listenPort: number;
    readonly destination: string;
    readonly injectMtls?: {
      readonly certPath: string;
      readonly keyPath: string;
    };
  }[];
  readonly tapFilter?: string;
}

// see: cans/05-storage-plane.md#Host-inode-lock-broker
export interface LockBrokerConfig {
  readonly globs: readonly string[];
  readonly nonBlocking?: boolean;
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export interface CreatePluginOptions<Opts = unknown> {
  readonly opts?: Opts;
  readonly enabled?: boolean;
  readonly config?: Opts;
}
