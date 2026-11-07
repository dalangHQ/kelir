// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type { Schema } from "./schema.js";
import type { TrapContext, LifecycleContext } from "./context.js";
import type { KelirPlugin } from "./plugin.js";
import type { PostExecAction, LifecycleEvent } from "./verdict.js";
import type { Verdict } from "./verdict.js";
import type { TrapDenyEvent } from "./types.js";
import type { Session } from "./session.js";
import { EConfig } from "./errors.js";
import { TrapFilter } from "./trap.js";

export interface KelirConfig {
  readonly vcpus?: number;
  readonly memoryMb?: number;
  readonly driver?: string;
  readonly kernel?: string;
  readonly rootfsErofs?: string;
  readonly tscLocked?: boolean;
  readonly enableWatchdog?: boolean;
}

export interface TerminalStream {
  write(data: string): void;
}

export interface TerminalConfig {
  readonly cols?: number;
  readonly rows?: number;
  readonly dropOsc52?: boolean;
  readonly stripTitle?: boolean;
  readonly tokenShield?: {
    readonly maxBytesTotal: number;
    readonly maxLinesTotal: number;
    readonly clampStrategy: string;
    readonly onClamp: (stats: {
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

export interface StorageConfig {
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
      c: TrapContext,
    ) => Promise<void> | void;
  }[];
  readonly encapsulation?: string;
  readonly scanEngine?: string;
  readonly carryoverBytes?: number;
}

export interface WireConfig {
  readonly sinkhole?: { readonly dnsExcept: readonly string[] };
  readonly sshAgentBridge?: boolean;
  readonly vaults?: readonly {
    readonly name: string;
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

export type TrapHandlerResult = Verdict | PostExecAction | null | void;

export type TrapHandler = (
  c: TrapContext,
) => Promise<TrapHandlerResult> | TrapHandlerResult;

export interface TrapHandlers {
  readonly pre?: TrapHandler;
  readonly post?: (
    c: TrapContext,
  ) => Promise<PostExecAction> | PostExecAction;
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
export class FusedClosure {
  readonly trampolineCrossings = 1;
  private readonly handlers: readonly TrapHandler[];

  constructor(handlers: readonly TrapHandler[]) {
    this.handlers = handlers;
  }

  get handlerCount(): number {
    return this.handlers.length;
  }

  async invoke(c: TrapContext): Promise<TrapHandlerResult> {
    for (const handler of this.handlers) {
      const result = await handler(c);
      if (result !== null && result !== undefined) {
        return result;
      }
    }
    return null;
  }
}

export class KelirBuilder {
  private plugins: KelirPlugin[] = [];
  private fusedHandlers: TrapHandler[] = [];

  constructor(_config: KelirConfig = {}) {}

  terminal(_config: TerminalConfig): this {
    return this;
  }

  storage(_config: StorageConfig): this {
    return this;
  }

  wire(_config: WireConfig): this {
    return this;
  }

  use<P extends KelirPlugin>(plugin: P): this {
    this.plugins.push(plugin);
    return this;
  }

  params(_schema: Schema<any>): this {
    return this;
  }

  trap(filter: string | TrapFilter, handler: TrapHandler | TrapHandlers): this {
    void filter;
    if (typeof handler === "function") {
      this.fusedHandlers.push(handler);
    } else if (handler.pre !== undefined) {
      this.fusedHandlers.push(handler.pre);
    }
    return this;
  }

  fuseHooks(): FusedClosure {
    // TODO (L3): hand the fused closure to the NAPI addon as the single FFI trampoline; codegen of the flat native closure lives in the addon.
    return new FusedClosure(this.fusedHandlers);
  }

  on(
    event: LifecycleEvent.VmInit,
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): this;
  on(
    event: LifecycleEvent.VmReady | LifecycleEvent.VmShutdown,
    handler: (session: Session) => Promise<void> | void,
  ): this;
  on(
    event: LifecycleEvent,
    handler:
      | ((ctx: LifecycleContext) => Promise<void> | void)
      | ((session: Session) => Promise<void> | void),
  ): this {
    void event;
    void handler;
    return this;
  }

  onTrapDeny(
    _handler: (event: TrapDenyEvent, session: Session) => Promise<void> | void,
  ): this {
    return this;
  }

  registerCustomAction<P>(
    _name: string,
    _schema: Schema<P>,
    _handler: (payload: P) => Promise<void> | void,
  ): this {
    return this;
  }

  registerVsockService<Req>(
    _kind: number,
    _schema: Schema<Req>,
    _handler: (request: Req) => Promise<Req> | Req,
  ): this {
    return this;
  }

  async boot(): Promise<Session> {
    const fused = this.fuseHooks();
    void fused;
    const specifier = "./native/kelir.node";
    let addon: unknown;
    try {
      // TODO (L3): load the precompiled NAPI addon and call NativeKelirBuilder.boot().
      addon = await import(specifier);
    } catch {
      throw new EConfig(
        "kelir native NAPI addon is missing: install the precompiled @kelir/core native build (L3)",
      );
    }
    void addon;
    throw new EConfig(
      "kelir native NAPI addon binding is not implemented (L3)",
    );
  }
}

export function kelir(config: KelirConfig = {}): KelirBuilder {
  return new KelirBuilder(config);
}
