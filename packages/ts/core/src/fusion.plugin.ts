// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/01-architecture.md#Foreign-runtime-embedding
// see: cans/03-sdk-architecture.md#TypeScript-SDK

import { EConfig } from "./errors.sdk.flow.js";
import { LifecycleEvent } from "./verdict.protocol.flow.js";
import type { NativeSessionHandle } from "./session.lifecycle.types.flow.js";
import {
  RingEvent,
  type HookStage,
  type LifecycleTag,
} from "./generated/fusion.tables.generated.js";

export {
  LIFECYCLE_TAGS,
  RingEvent,
  type HookStage,
  type LifecycleTag,
} from "./generated/fusion.tables.generated.js";

export interface TrapFilterSnapshot {
  readonly binary?: string;
  readonly args: readonly string[];
  readonly pattern?: string;
  readonly cwd?: string;
  readonly env: readonly (readonly [string, string])[];
  readonly uid?: number;
  readonly argv?: string;
}

export interface HookDescriptor {
  readonly ring: RingEvent;
  readonly stage: HookStage;
  readonly pluginName?: string;
  readonly ordinal: number;
  readonly filter?: TrapFilterSnapshot;
  readonly lifecycleTag?: LifecycleTag;
  readonly customActionName?: string;
  readonly vsockKind?: number;
  run(event: unknown): unknown;
}

export function lifecycleTagFor(event: LifecycleEvent): LifecycleTag {
  switch (event) {
    case LifecycleEvent.VmInit:
      return "vm-init";
    case LifecycleEvent.VmReady:
      return "vm-ready";
    case LifecycleEvent.VmShutdown:
      return "vm-shutdown";
    default:
      throw new EConfig(
        `lifecycle event ${String(event)} has no fused trampoline ring`,
      );
  }
}

export interface FusedHookRun {
  readonly ring: RingEvent;
  readonly ordinal: number;
  readonly run: (ring: RingEvent, ordinal: number) => unknown;
}

export interface FusedRingChain {
  readonly ring: RingEvent;
  readonly hooks: readonly HookDescriptor[];
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
export { FusedTrampoline } from "./fusion.plugin.flow.js";

export interface FusedHookEntry {
  readonly ordinal: number;
  readonly stage: HookStage;
  readonly pluginName?: string;
  readonly filter?: TrapFilterSnapshot;
  readonly lifecycleTag?: LifecycleTag;
  readonly customActionName?: string;
  readonly vsockKind?: number;
}

export interface FusedRingChainDescriptor {
  readonly ring: RingEvent;
  readonly hookCount: number;
  readonly hooks: readonly FusedHookEntry[];
}

export interface FusedTrampolineDescriptor {
  readonly rings: readonly FusedRingChainDescriptor[];
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export interface NativeTerminalConfigBinding {
  readonly cols?: number;
  readonly rows?: number;
  readonly wireframeGrid?: {
    readonly trackScreenMatrix?: boolean;
    readonly exportOnTrap?: boolean;
  };
  readonly sanitizer?: {
    readonly dropOsc52?: boolean;
    readonly stripTitle?: boolean;
  };
  readonly streamEngine?: string;
}

// see: cans/05-storage-plane.md#Workspace-commit
export interface NativeStorageConfigBinding {
  readonly idmapGuestUid?: number;
  readonly idmapHostUid?: number;
  readonly sharedCaches?: readonly {
    readonly hostPath: string;
    readonly guestMount: string;
    readonly readOnly?: boolean;
  }[];
  readonly lockBroker?: boolean;
  readonly lockBrokerGlobs?: string[];
  readonly lockBrokerNonBlocking?: boolean;
  readonly encapsulation?: string;
  readonly daxWindow?: string;
}

// see: cans/07-wire-plane.md#Sinkhole-rules
export interface NativeSinkholeRuleBinding {
  readonly host: string;
  readonly allow?: boolean;
}

// see: cans/07-wire-plane.md#DNS-policy-filter
export interface NativeWireConfigBinding {
  readonly sinkhole?: { readonly rules?: readonly NativeSinkholeRuleBinding[] };
  readonly sshAgentBridge?: boolean;
  readonly dialTimeoutMs?: number;
  readonly l4Proxies?: readonly {
    readonly listenPort: number;
    readonly destination: string;
    readonly forwardUnixSocket?: string;
    readonly injectMtlsCertPath?: string;
    readonly injectMtlsKeyPath?: string;
  }[];
  readonly tapFilter?: string;
  readonly blockAbstractUnixSockets?: boolean;
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export interface NativeKelirBuilderBinding {
  addPlugin(name: string, enabled?: boolean): void;
  setTerminalConfig(config: NativeTerminalConfigBinding): void;
  setStorageConfig(config: NativeStorageConfigBinding): void;
  setWireConfig(config: NativeWireConfigBinding): void;
  fuseHooks(
    rings: readonly FusedRingChainDescriptor[],
    runs?: readonly FusedHookRun[],
  ): void;
  trampolineEntryCount(): number;
  boot(): Promise<NativeSessionHandle>;
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export interface NativeKelirOptionsBinding {
  readonly vcpus?: number;
  readonly memoryMb?: number;
  readonly kernel?: string;
  readonly rootfsErofs?: string;
  readonly tscLocked?: boolean;
  readonly enableWatchdog?: boolean;
  readonly driver?: string;
  readonly networkOff?: boolean;
  readonly nsPerInstruction?: number;
  readonly entropySeed?: number;
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export interface NativeAddonBinding {
  readonly abiVersion: () => number;
  readonly relaySocketPath: (sessionId: string) => string;
  readonly NativeKelirBuilder: new (
    options?: NativeKelirOptionsBinding,
  ) => NativeKelirBuilderBinding;
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export const KELIR_ADDON_ABI_VERSION = 1;

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export const NATIVE_ADDON_SPECIFIERS: readonly string[] = [
  "./native/kelir.node",
];

// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
export class NativeAddonMissingError extends EConfig {
  constructor(message: string) {
    super(message);
    this.name = "NativeAddonMissingError";
  }
}

// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
export class NativeAddonAbiMismatchError extends EConfig {
  readonly expectedAbiVersion: number;
  readonly foundAbiVersion: number;

  constructor(expected: number, found: number) {
    super(
      `kelir native NAPI addon ABI version mismatch: expected ${expected}, found ${found}`,
    );
    this.name = "NativeAddonAbiMismatchError";
    this.expectedAbiVersion = expected;
    this.foundAbiVersion = found;
  }
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export function verifyNativeAddonAbi(addon: NativeAddonBinding): void {
  const found: unknown =
    typeof addon.abiVersion === "function" ? addon.abiVersion() : undefined;
  if (found !== KELIR_ADDON_ABI_VERSION) {
    throw new NativeAddonAbiMismatchError(
      KELIR_ADDON_ABI_VERSION,
      typeof found === "number" ? found : -1,
    );
  }
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export { loadNativeAddon } from "./fusion.plugin.run.js";
