// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/01-architecture.md#Foreign-runtime-embedding
// see: cans/03-sdk-architecture.md#TypeScript-SDK

import { EConfig } from "./errors.sdk.js";
import { LifecycleEvent, VetoEvent } from "./verdict.protocol.js";
import type { NativeSessionHandle } from "./session.lifecycle.js";
import type { LifecycleContext } from "./context.lifecycle.types.js";
import {
  LIFECYCLE_TAGS,
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

// see: cans/08-lifecycle.md#Session-and-context-handles
// TODO (L3) [plugin_engine]: forward the recorded vetoes and watchdog arming to the engine lifecycle dispatch.; slop: recorded veto/watchdog never enforced
export function createLifecycleContext(
  cap?: LifecycleContext["cap"],
): LifecycleContext {
  const vetoes: Array<[VetoEvent, () => boolean]> = [];
  let watchdog: { timeoutMs: number; action: string } | undefined;
  return {
    setWatchdog: (options) => {
      watchdog = options;
    },
    registerVeto: (event, predicate) => {
      vetoes.push([event, predicate]);
    },
    cap,
  };
}

export interface FusedRingChain {
  readonly ring: RingEvent;
  readonly hooks: readonly HookDescriptor[];
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
export class FusedTrampoline {
  private constructor(readonly chains: readonly FusedRingChain[]) {}

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  static fuse(hooks: readonly HookDescriptor[]): FusedTrampoline {
    const byRing = new Map<RingEvent, HookDescriptor[]>();
    for (const hook of hooks) {
      validateHook(hook);
      const chain = byRing.get(hook.ring);
      if (chain === undefined) {
        byRing.set(hook.ring, [hook]);
      } else {
        chain.push(hook);
      }
    }
    const chains: FusedRingChain[] = [];
    for (const [ring, chain] of byRing) {
      const ordered = [...chain].sort((a, b) => a.ordinal - b.ordinal);
      for (let i = 1; i < ordered.length; i += 1) {
        if (ordered[i - 1].ordinal === ordered[i].ordinal) {
          throw new EConfig(
            `fused trampoline ring ${ring} has duplicate ordinal ${ordered[i].ordinal}`,
          );
        }
      }
      chains.push({ ring, hooks: ordered });
    }
    return new FusedTrampoline(chains);
  }

  crossingsPerRingEvent(ring: RingEvent): number {
    return this.chains.some((chain) => chain.ring === ring) ? 1 : 0;
  }

  hookCount(ring: RingEvent): number {
    return this.chains.find((chain) => chain.ring === ring)?.hooks.length ?? 0;
  }

  // TODO (L3) [bt_03_typescript_fused_trampoline_single_crossing_chained_hooks]: compile each fused chain into the single flat closure that runs it per ring event.; slop: descriptor claims fused while hooks cross per hook
  describe(): FusedTrampolineDescriptor {
    return {
      rings: this.chains.map((chain) => ({
        ring: chain.ring,
        hookCount: chain.hooks.length,
        hooks: chain.hooks.map((hook) => ({
          ordinal: hook.ordinal,
          stage: hook.stage,
          pluginName: hook.pluginName,
          filter: hook.filter,
          lifecycleTag: hook.lifecycleTag,
          customActionName: hook.customActionName,
          vsockKind: hook.vsockKind,
        })),
      })),
    };
  }

  // see: cans/02-plugin-model.md#Foreign-plugin-proxy
  // see: cans/08-lifecycle.md#Session-and-context-handles
  runs(cap?: LifecycleContext["cap"]): readonly FusedHookRun[] {
    return this.chains.flatMap((chain) =>
      chain.hooks.map((hook) => ({
        ring: chain.ring,
        ordinal: hook.ordinal,
        run: (ring: RingEvent, ordinal: number) =>
          hook.ring === RingEvent.Lifecycle
            ? hook.run(createLifecycleContext(cap))
            : hook.run({ ring, ordinal }),
      })),
    );
  }
}

function validateHook(hook: HookDescriptor): void {
  if (hook.ring === RingEvent.CustomAction && !hook.customActionName) {
    throw new EConfig(
      `custom action hook on ordinal ${hook.ordinal} declares no action name`,
    );
  }
  if (hook.ring === RingEvent.VsockService) {
    if (hook.vsockKind === undefined) {
      throw new EConfig(
        `vsock service hook on ordinal ${hook.ordinal} declares no stream kind`,
      );
    }
    if (!Number.isInteger(hook.vsockKind) || hook.vsockKind < 0 || hook.vsockKind > 0xffff) {
      throw new EConfig(
        `vsock service hook on ordinal ${hook.ordinal} declares kind ${String(hook.vsockKind)} outside u16`,
      );
    }
  }
  if (hook.ring === RingEvent.Lifecycle &&
    (hook.lifecycleTag === undefined || !LIFECYCLE_TAGS.includes(hook.lifecycleTag))) {
    throw new EConfig(
      `lifecycle hook on ordinal ${hook.ordinal} declares unknown lifecycle tag ${String(hook.lifecycleTag)}`,
    );
  }
}

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
  readonly dropOsc52?: boolean;
  readonly stripTitle?: boolean;
  readonly tokenShieldMaxBytesTotal?: number;
  readonly tokenShieldMaxLinesTotal?: number;
  readonly tokenShieldClampStrategy?: string;
  readonly streamEngine?: string;
}

// see: cans/05-storage-plane.md#Workspace-commit
export interface NativeStorageConfigBinding {
  readonly idmapGuestUid?: number;
  readonly idmapHostUid?: number;
  readonly sharedCaches?: string[];
  readonly lockBroker?: boolean;
  readonly syntheticFiles?: readonly {
    readonly path: string;
    readonly content: string;
    readonly mode?: number;
  }[];
  readonly maskRead?: readonly {
    readonly glob: string;
    readonly pattern: string;
    readonly replace: string;
  }[];
  readonly encapsulation?: string;
  readonly scanEngine?: string;
  readonly carryoverBytes?: number;
}

// see: cans/07-wire-plane.md#DNS-policy-filter
export interface NativeWireConfigBinding {
  readonly sinkholeDnsExcept?: string[];
  readonly sinkholeDropAll?: boolean;
  readonly sshAgentBridge?: boolean;
  readonly vaults?: readonly {
    readonly name: string;
    readonly kind: string;
    readonly audience: string;
    readonly placeholder: string;
  }[];
  readonly l4Proxies?: readonly {
    readonly listenPort: number;
    readonly destination: string;
    readonly injectMtlsCertPath?: string;
    readonly injectMtlsKeyPath?: string;
  }[];
  readonly tapFilter?: string;
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
export async function loadNativeAddon(): Promise<NativeAddonBinding> {
  for (const specifier of NATIVE_ADDON_SPECIFIERS) {
    let addon: unknown;
    try {
      addon = await import(specifier);
    } catch {
      continue;
    }
    const binding =
      (addon as { readonly default?: NativeAddonBinding }).default ??
      (addon as NativeAddonBinding);
    verifyNativeAddonAbi(binding);
    return binding;
  }
  // TODO (L3) [bt_04_napi_boot_before_fuse_hooks_fails_closed]: ship the precompiled NAPI addon next to the dist bundle.; slop: dist ships without native addon, boot fails closed with NativeAddonMissingError
  throw new NativeAddonMissingError(
    "kelir native NAPI addon is missing: install the precompiled @kelir/core native build (L3)",
  );
}
