// see: cans/03-sdk-architecture.md#TypeScript-SDK
import type { HandlerContext } from "./context.lifecycle.types.js";
import type { PluginContext } from "./hook.registry.js";
import type { PostExecAction } from "./step-eval.protocol.js";
import type { Verdict } from "./verdict.protocol.js";
import type { KelirBuilder } from "./builder.lifecycle.js";

export type TrapHandlerResult = Verdict | PostExecAction;

export type TrapHandler<Ctx, Params> = (
  c: HandlerContext<Ctx, Params>,
) => Promise<TrapHandlerResult> | TrapHandlerResult;

export interface TrapHandlers<Ctx, Params> {
  readonly pre?: TrapHandler<Ctx, Params>;
  readonly post?: (c: HandlerContext<Ctx, Params>) => Promise<PostExecAction> | PostExecAction;
}

export interface PluginRegistration {
  readonly name: string;
  readonly enabled: boolean;
}

export interface MountRecord {
  readonly name: string;
  readonly enabled: boolean;
  readonly requiredDeps: readonly string[];
  readonly duplicate: boolean;
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export type ValidateDeps<Ctx, P> = P extends KelirBuilder<any, any, any, any, infer Deps, any>
  ? [Deps] extends [never]
    ? unknown
    : string extends Deps
      ? unknown
      : [Deps] extends [keyof Ctx]
        ? unknown
        : { readonly __missing_dependency: Deps }
  : unknown;

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export type PluginContextOf<P> =
  P extends KelirBuilder<any, any, infer Name, infer C, any, any>
    ? (C extends Record<string, unknown>
        ? (string extends keyof C
            ? { readonly [K in Name]: C }
            : (Name extends keyof C ? C : { readonly [K in Name]: C }))
        : { readonly [K in Name]: C })
    : P extends PluginContext<infer C>
      ? C
      : unknown;
