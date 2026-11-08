// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/02-plugin-model.md#Foreign-plugin-proxy

import type { TrapContext } from "./context.js";
import {
  RingEvent,
  type HookDescriptor,
} from "./fusion.js";
import type {
  FilterAction,
  Verdict,
} from "./verdict.js";
import type {
  HeartbeatEvent,
  ProcessExitEvent,
  TrapInfo,
  WatchdogEvent,
  WatchdogVerdict,
} from "./types.js";

export interface KelirPlugin {
  readonly name: string;
  readonly enabled?: boolean;
  readonly config?: unknown;
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export interface PluginContext<Ctx> {
  readonly pluginCtx?: Ctx;
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export type PluginContextOf<P> =
  P extends PluginContract<any, infer C, any> ? C
    : P extends PluginContext<infer C> ? C
      : unknown;

export interface CreatePluginOptions {
  readonly opts?: unknown;
}

function unpack(event: unknown): readonly unknown[] {
  return Array.isArray(event) ? (event as readonly unknown[]) : [event];
}

export class PluginContract<
  Name extends string = string,
  Ctx = unknown,
  Deps extends string = string,
> implements KelirPlugin {
  readonly name: Name;
  private readonly hooks: HookDescriptor[] = [];
  private nextOrdinal = 0;

  constructor(name: Name, _options?: CreatePluginOptions) {
    void _options;
    this.name = name;
  }

  hooksForFusion(): readonly HookDescriptor[] {
    return this.hooks;
  }

  // see: cans/02-plugin-model.md#Foreign-plugin-proxy
  private addHook(
    ring: RingEvent,
    run: (event: unknown) => unknown,
  ): void {
    this.hooks.push({
      ring,
      stage: "single",
      pluginName: this.name,
      ordinal: this.nextOrdinal,
      run,
    });
    this.nextOrdinal += 1;
  }

  onNetOutbound(
    handler: (frame: Uint8Array) => FilterAction,
  ): PluginContract<Name, Ctx, Deps> {
    this.addHook(RingEvent.NetOutbound, (event) =>
      handler(unpack(event)[0] as Uint8Array),
    );
    return this;
  }

  onFsLookup(
    handler: (path: string) => Verdict,
  ): PluginContract<Name, Ctx, Deps> {
    this.addHook(RingEvent.FsLookup, (event) =>
      handler(unpack(event)[0] as string),
    );
    return this;
  }

  onFsRead(
    handler: (path: string, buffer: Uint8Array) => Uint8Array,
  ): PluginContract<Name, Ctx, Deps> {
    this.addHook(RingEvent.FsRead, (event) => {
      const args = unpack(event);
      return handler(args[0] as string, args[1] as Uint8Array);
    });
    return this;
  }

  onFsWrite(
    handler: (
      path: string,
      buffer: Uint8Array,
      pid: number,
      ctx: TrapContext,
    ) => FilterAction,
  ): PluginContract<Name, Ctx, Deps> {
    this.addHook(RingEvent.FsWrite, (event) => {
      const args = unpack(event);
      return handler(
        args[0] as string,
        args[1] as Uint8Array,
        args[2] as number,
        args[3] as TrapContext,
      );
    });
    return this;
  }

  onConsoleInput(
    handler: (
      port: number,
      bytes: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): PluginContract<Name, Ctx, Deps> {
    this.addHook(RingEvent.ConsoleInput, (event) => {
      const args = unpack(event);
      return handler(args[0] as number, args[1] as Uint8Array);
    });
    return this;
  }

  onConsoleOutput(
    handler: (
      port: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): PluginContract<Name, Ctx, Deps> {
    this.addHook(RingEvent.ConsoleOutput, (event) => {
      const args = unpack(event);
      return handler(args[0] as number, args[1] as Uint8Array);
    });
    return this;
  }

  // see: cans/02-plugin-model.md#KelirPlugin-trait
  onExecStdout(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): PluginContract<Name, Ctx, Deps> {
    this.addHook(RingEvent.ExecStdout, (event) => {
      const args = unpack(event);
      return handler(
        args[0] as number,
        args[1] as number,
        args[2] as Uint8Array,
      );
    });
    return this;
  }

  // see: cans/02-plugin-model.md#KelirPlugin-trait
  onExecStderr(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): PluginContract<Name, Ctx, Deps> {
    this.addHook(RingEvent.ExecStderr, (event) => {
      const args = unpack(event);
      return handler(
        args[0] as number,
        args[1] as number,
        args[2] as Uint8Array,
      );
    });
    return this;
  }

  onHeartbeat(
    handler: (event: HeartbeatEvent) => Promise<void> | void,
  ): PluginContract<Name, Ctx, Deps> {
    this.addHook(RingEvent.Heartbeat, (event) =>
      handler(unpack(event)[0] as HeartbeatEvent),
    );
    return this;
  }

  onProcessExit(
    handler: (event: ProcessExitEvent) => Promise<void> | void,
  ): PluginContract<Name, Ctx, Deps> {
    this.addHook(RingEvent.ProcessExit, (event) =>
      handler(unpack(event)[0] as ProcessExitEvent),
    );
    return this;
  }

  onTrap(
    handler: (
      event: TrapInfo,
      c: TrapContext,
    ) => Promise<Verdict | null> | Verdict | null,
  ): PluginContract<Name, Ctx, Deps> {
    this.addHook(RingEvent.Trap, (event) => {
      const args = unpack(event);
      return handler(args[0] as TrapInfo, args[1] as TrapContext);
    });
    return this;
  }

  onWatchdog(
    handler: (
      event: WatchdogEvent,
    ) => Promise<WatchdogVerdict> | WatchdogVerdict,
  ): PluginContract<Name, Ctx, Deps> {
    this.addHook(RingEvent.Watchdog, (event) =>
      handler(unpack(event)[0] as WatchdogEvent),
    );
    return this;
  }
}

export function createPlugin<
  Name extends string,
  Ctx = unknown,
  Deps extends string = string,
>(name: Name, options?: CreatePluginOptions): PluginContract<Name, Ctx, Deps> {
  return new PluginContract<Name, Ctx, Deps>(name, options);
}
