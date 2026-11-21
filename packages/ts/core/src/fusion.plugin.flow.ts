// see: cans/03-sdk-architecture.md#TypeScript-SDK

import { EConfig } from "./errors.sdk.flow.js";
import {
  LIFECYCLE_TAGS,
  RingEvent,
} from "./generated/fusion.tables.generated.js";
import type {
  FusedHookRun,
  FusedRingChain,
  FusedTrampolineDescriptor,
  HookDescriptor,
} from "./fusion.plugin.js";

import { LifecycleEvent } from "./verdict.protocol.flow.js";
import { KELIR_ADDON_ABI_VERSION, type NativeAddonBinding } from "./fusion.plugin.js";
import type { LifecycleTag } from "./generated/fusion.tables.generated.js";

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

  // TODO: [bt_03_typescript_fused_trampoline_single_crossing_chained_hooks]: compile each fused chain into the single flat closure that runs it per ring event.; slop: descriptor claims fused while hooks cross per hook
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
  runs(): readonly FusedHookRun[] {
    return this.chains.flatMap((chain) =>
      chain.hooks.map((hook) => ({
        ring: chain.ring,
        ordinal: hook.ordinal,
        run: (ring: RingEvent, ordinal: number) => hook.run({ ring, ordinal }),
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
