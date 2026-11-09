// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/01-architecture.md#Foreign-runtime-embedding
// see: cans/03-sdk-architecture.md#TypeScript-SDK

import { EConfig } from "./errors.sdk.js";
import { LifecycleEvent } from "./verdict.protocol.js";
import type { NativeSessionHandle } from "./session.lifecycle.js";
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

  // TODO (L3): compile each fused chain into the single flat closure that runs it per ring event.
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
export interface NativeKelirBuilderBinding {
  addPlugin(name: string, enabled?: boolean): void;
  fuseHooks(rings: readonly FusedRingChainDescriptor[]): void;
  trampolineEntryCount(): number;
  boot(): Promise<NativeSessionHandle>;
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export interface NativeAddonBinding {
  readonly NativeKelirBuilder: new (
    vcpus?: number,
    memoryMb?: number,
    kernel?: string,
    rootfsErofs?: string,
    tscLocked?: boolean,
    enableWatchdog?: boolean,
    driver?: string,
  ) => NativeKelirBuilderBinding;
}
