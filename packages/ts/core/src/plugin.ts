// see: cans/03-sdk-architecture.md#TypeScript-SDK
// Plugin contract + factory. The Name/Ctx/Deps parameters document the
// plugin's identity, context shape, and dependencies; the stub does not
// enforce them structurally (no runtime plugin system yet), so a
// contract built without explicit type arguments still satisfies a
// fully-annotated PluginContract type.

import type { TrapContext } from "./context.js";
import type {
  FilterAction,
  Verdict,
} from "./verdict.js";
import type {
  HeartbeatEvent,
  ProcessExitEvent,
  TrapInfo,
  WatchdogAction,
  WatchdogEvent,
} from "./types.js";

export interface KelirPlugin {
  readonly name: string;
}

export interface CreatePluginOptions {
  readonly opts?: unknown;
}

export class PluginContract<
  Name extends string = string,
  Ctx = any,
  Deps extends string = string,
> implements KelirPlugin {
  readonly name: Name;

  constructor(name: Name, _options?: CreatePluginOptions) {
    void _options;
    this.name = name;
  }

  onNetOutbound(
    _handler: (
      frame: Uint8Array,
    ) => Promise<FilterAction> | FilterAction,
  ): PluginContract<Name, Ctx, Deps> {
    return this;
  }

  onFsLookup(
    _handler: (path: string) => Promise<Verdict> | Verdict,
  ): PluginContract<Name, Ctx, Deps> {
    return this;
  }

  onFsRead(
    _handler: (
      path: string,
      buffer: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): PluginContract<Name, Ctx, Deps> {
    return this;
  }

  onFsWrite(
    _handler: (
      path: string,
      buffer: Uint8Array,
      pid: number,
      ctx: TrapContext,
    ) => Promise<FilterAction> | FilterAction,
  ): PluginContract<Name, Ctx, Deps> {
    return this;
  }

  onConsoleInput(
    _handler: (
      port: number,
      bytes: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): PluginContract<Name, Ctx, Deps> {
    return this;
  }

  onConsoleOutput(
    _handler: (
      port: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): PluginContract<Name, Ctx, Deps> {
    return this;
  }

  onHeartbeat(
    _handler: (event: HeartbeatEvent) => Promise<void> | void,
  ): PluginContract<Name, Ctx, Deps> {
    return this;
  }

  onProcessExit(
    _handler: (event: ProcessExitEvent) => Promise<void> | void,
  ): PluginContract<Name, Ctx, Deps> {
    return this;
  }

  onTrap(
    _handler: (
      event: TrapInfo,
      c: TrapContext,
    ) => Promise<Verdict | null> | Verdict | null,
  ): PluginContract<Name, Ctx, Deps> {
    return this;
  }

  onWatchdog(
    _handler: (
      event: WatchdogEvent,
    ) => Promise<WatchdogAction> | WatchdogAction,
  ): PluginContract<Name, Ctx, Deps> {
    return this;
  }
}

export function createPlugin<
  Name extends string,
  Ctx = any,
  Deps extends string = string,
>(name: Name, options?: CreatePluginOptions): PluginContract<Name, Ctx, Deps> {
  return new PluginContract<Name, Ctx, Deps>(name, options);
}
