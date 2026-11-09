// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/artifacts/code/typescript-accumulation.ts#runtime

import type { Schema } from "./schema.config.js";
import type { TrapContext, LifecycleContext } from "./context.lifecycle.types.js";
import type { KelirPlugin, PluginContextOf } from "./plugin.model.js";
import type { PostExecAction, LifecycleEvent, Verdict } from "./verdict.protocol.js";
import type { TrapDenyEvent } from "./types.core.js";
import { Session } from "./session.lifecycle.js";
import { EConfig } from "./errors.sdk.js";
import { TrapFilter } from "./trap.protocol.js";
import {
  FusedTrampoline,
  RingEvent,
  lifecycleTagFor,
  type FusedTrampolineDescriptor,
  type HookDescriptor,
  type NativeAddonBinding,
} from "./fusion.plugin.js";

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

export type TrapHandlerResult = Verdict | PostExecAction | null | void;

export type TrapHandler<Ctx = unknown> = (
  c: TrapContext & Ctx,
) => Promise<TrapHandlerResult> | TrapHandlerResult;

export interface TrapHandlers<Ctx = unknown> {
  readonly pre?: TrapHandler<Ctx>;
  readonly post?: (c: TrapContext & Ctx) => Promise<PostExecAction> | PostExecAction;
}

interface PluginRegistration {
  readonly name: string;
  readonly enabled: boolean;
}

// see: cans/11-protocol.md#Control-channel
export interface VsockServiceEntry {
  readonly port: number;
  readonly service: string;
  readonly schema: Schema<unknown>;
  readonly handler: (request: unknown) => Promise<unknown> | unknown;
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export class KelirBuilder<Ctx = unknown> {
  private readonly config: KelirConfig;
  private readonly pluginRegistrations: PluginRegistration[] = [];
  private readonly hooks: HookDescriptor[] = [];
  // TODO (L3): the engine dispatch reads these entries once register_interceptors wiring lands per cans/02-plugin-model.md#Static-dispatch.
  readonly vsockServices: VsockServiceEntry[] = [];
  private nextOrdinal = 0;
  private detachedFlag: boolean;

  constructor(config: KelirConfig = {}) {
    this.config = config;
    this.detachedFlag = config.detached ?? false;
  }

  // see: cans/08-lifecycle.md#Detached-lifecycle
  detached(detached: boolean): this {
    this.detachedFlag = detached;
    return this;
  }

  isDetached(): boolean {
    return this.detachedFlag;
  }

  // see: cans/artifacts/code/typescript-accumulation.ts#runtime
  use<P extends KelirPlugin>(
    plugin: P,
  ): KelirBuilder<Ctx & PluginContextOf<P>> {
    const enabled = (plugin as { readonly enabled?: boolean }).enabled !== false;
    this.pluginRegistrations.push({ name: plugin.name, enabled });
    const contract = plugin as unknown as {
      hooksForFusion?: () => readonly HookDescriptor[];
    };
    if (typeof contract.hooksForFusion === "function" && enabled) {
      for (const hook of contract.hooksForFusion()) {
        this.hooks.push(hook);
      }
    }
    return this as KelirBuilder<Ctx & PluginContextOf<P>>;
  }

  // see: cans/02-plugin-model.md#Foreign-plugin-proxy
  trap(
    filter: string | TrapFilter,
    handler: TrapHandler<Ctx> | TrapHandlers<Ctx>,
  ): this {
    const filterSnapshot =
      typeof filter === "string" ? TrapFilter.pattern(filter).snapshot() : filter.snapshot();
    if (typeof handler === "function") {
      this.hooks.push({
        ring: RingEvent.Trap,
        stage: "single",
        ordinal: this.nextOrdinal,
        filter: filterSnapshot,
        run: (event: unknown): unknown =>
          (handler as TrapHandler<Ctx>)(event as TrapContext & Ctx),
      });
      this.nextOrdinal += 1;
      return this;
    }
    if (handler.pre !== undefined) {
      const pre = handler.pre;
      this.hooks.push({
        ring: RingEvent.Trap,
        stage: "pre",
        ordinal: this.nextOrdinal,
        filter: filterSnapshot,
        run: (event: unknown): unknown => pre(event as TrapContext & Ctx),
      });
      this.nextOrdinal += 1;
    }
    if (handler.post !== undefined) {
      const post = handler.post;
      this.hooks.push({
        ring: RingEvent.Trap,
        stage: "post",
        ordinal: this.nextOrdinal,
        filter: filterSnapshot,
        run: (event: unknown): unknown => post(event as TrapContext & Ctx),
      });
      this.nextOrdinal += 1;
    }
    return this;
  }

  on(
    event: LifecycleEvent.VmInit,
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): this;
  on(
    event: LifecycleEvent.VmReady | LifecycleEvent.VmShutdown,
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): this;
  on(
    event: LifecycleEvent,
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): this {
    this.hooks.push({
      ring: RingEvent.Lifecycle,
      stage: "single",
      ordinal: this.nextOrdinal,
      lifecycleTag: lifecycleTagFor(event),
      run: (payload: unknown): unknown =>
        (handler as (arg: unknown) => Promise<void> | void)(payload),
    });
    this.nextOrdinal += 1;
    return this;
  }

  onTrapDeny(
    handler: (
      event: TrapDenyEvent,
      session: Session & Ctx,
    ) => Promise<void> | void,
  ): this {
    this.hooks.push({
      ring: RingEvent.Lifecycle,
      stage: "single",
      ordinal: this.nextOrdinal,
      lifecycleTag: "trap-deny",
      run: (payload: unknown): unknown => {
        const args = Array.isArray(payload)
          ? (payload as readonly unknown[])
          : [payload, undefined];
        return (handler as (a: unknown, b: unknown) => Promise<void> | void)(
          args[0],
          args[1],
        );
      },
    });
    this.nextOrdinal += 1;
    return this;
  }

  registerCustomAction<P>(
    name: string,
    _schema: Schema<P>,
    handler: (payload: P) => Promise<void> | void,
  ): this {
    this.hooks.push({
      ring: RingEvent.CustomAction,
      stage: "single",
      ordinal: this.nextOrdinal,
      customActionName: name,
      run: (payload: unknown): unknown =>
        (handler as (arg: unknown) => Promise<void> | void)(payload),
    });
    this.nextOrdinal += 1;
    return this;
  }

  // see: cans/11-protocol.md#Control-channel
  registerVsockService<Req>(
    port: number,
    service: string,
    _schema: Schema<Req>,
    handler: (request: Req) => Promise<Req> | Req,
  ): this {
    if (!Number.isInteger(port) || port < 0 || port > 0xffffffff) {
      throw new EConfig(`vsock service port ${String(port)} outside u32`);
    }
    this.vsockServices.push({
      port,
      service,
      schema: _schema as Schema<unknown>,
      handler: handler as (request: unknown) => Promise<unknown> | unknown,
    });
    return this;
  }

  // see: cans/11-protocol.md#Control-channel
  registerStreamKind<Req>(
    kind: number,
    _schema: Schema<Req>,
    handler: (request: Req) => Promise<Req> | Req,
  ): this {
    this.hooks.push({
      ring: RingEvent.VsockService,
      stage: "single",
      ordinal: this.nextOrdinal,
      vsockKind: kind,
      run: (payload: unknown): unknown =>
        (handler as (arg: unknown) => unknown)(payload),
    });
    this.nextOrdinal += 1;
    return this;
  }

  // see: cans/02-plugin-model.md#Foreign-plugin-proxy
  fuse(): FusedTrampoline {
    return FusedTrampoline.fuse(this.hooks);
  }

  private fusedDescriptor(): FusedTrampolineDescriptor {
    return this.fuse().describe();
  }

  private async loadNativeAddon(): Promise<NativeAddonBinding> {
    const specifier = "./native/kelir.node";
    try {
      // TODO (L3): ship the precompiled NAPI addon next to the dist bundle and verify its ABI version.
      return (await import(specifier)) as NativeAddonBinding;
    } catch {
      throw new EConfig(
        "kelir native NAPI addon is missing: install the precompiled @kelir/core native build (L3)",
      );
    }
  }

  async boot(): Promise<Session & Ctx> {
    const descriptor = this.fusedDescriptor();
    const addon = await this.loadNativeAddon();
    const nativeBuilder = new addon.NativeKelirBuilder(
      this.config.vcpus,
      this.config.memoryMb,
      this.config.kernel,
      this.config.rootfsErofs,
      this.config.tscLocked,
      this.config.enableWatchdog,
      this.config.driver,
    );
    // TODO (L3): forward pageSizeKb/deterministicClock/pinnedClocksource/preemptionGraceMs once the native constructor accepts them.
    for (const registration of this.pluginRegistrations) {
      nativeBuilder.addPlugin(registration.name, registration.enabled);
    }
    nativeBuilder.fuseHooks(descriptor.rings);
    if (nativeBuilder.trampolineEntryCount() !== descriptor.rings.length) {
      throw new EConfig(
        "kelir native NAPI trampoline table is not one entry per ring event",
      );
    }
    // TODO (L3): bind each fused ring chain to its single trampoline entry as one threadsafe closure.
    // TODO (L3): bind the mounted plugins' capability members onto the booted session.
    return new Session(await nativeBuilder.boot()) as Session & Ctx;
  }
}

export function kelir(config: KelirConfig = {}): KelirBuilder {
  return new KelirBuilder(config);
}
