// see: cans/02-plugin-model.md#Nested-scopes
// see: cans/02-plugin-model.md#Host-only-verbs
// see: cans/03-sdk-architecture.md#TypeScript-SDK
import { kelir, createPlugin, t, Verdict, FilterAction } from "../../src/index.js";

// see: cans/02-plugin-model.md#Nested-scopes
export function negativeFsOnTrap(): unknown {
  return kelir().fs((f) =>
    // @ts-expect-error Property 'onTrap' does not exist on type 'FsScope<...>'
    f.onTrap((_event: unknown) => Verdict.Pass),
  );
}

// see: cans/02-plugin-model.md#Host-only-verbs
export function negativeHostVerbOnPlugin(): void {
  const plugin = createPlugin("neg");
  // @ts-expect-error Argument of type 'this' is not assignable to parameter of 'this'
  plugin.terminal({ cols: 80, rows: 24 });
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export function negativePluginVerbOnHost(): void {
  const host = kelir();
  // @ts-expect-error Argument of type 'this' is not assignable to parameter of 'this'
  host.context(() => ({ mode: "guest" }));
}

// see: cans/02-plugin-model.md#One-validation,-at-seal-and-boot
export function negativePostSealVerb(): unknown {
  const sealed = kelir().seal();
  // @ts-expect-error Argument of type 'this' is not assignable to parameter of 'this'
  return sealed.storage({});
}

// see: cans/02-plugin-model.md#Chain-decisions
export function negativeDeletedFlatVerb(): unknown {
  const host = kelir();
  // @ts-expect-error Property 'onFsWrite' does not exist on type 'KelirBuilder<...>'
  return host.onFsWrite((_path: string, _buffer: Uint8Array, _pid: number, _ctx: unknown) =>
    FilterAction.Forward,
  );
}

// see: cans/02-plugin-model.md#One-validation,-at-seal-and-boot
export function negativeBootOnPlugin(): unknown {
  const plugin = createPlugin("neg");
  // @ts-expect-error Argument of type 'this' is not assignable to parameter of 'this'
  return plugin.boot();
}

// see: cans/03-sdk-architecture.md#Params-typed-views
export function negativeParamsDirectModel(): unknown {
  const host = kelir();
  // @ts-expect-error Argument of type 'Schema<...>' is not assignable to parameter of 'fn'
  return host.params(t.Object({ ping: t.String() }));
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export function negativeGuardOnSealed(): unknown {
  const sealed = kelir()
    .trap((scope) => scope.onTrap(() => Verdict.Pass))
    .seal();
  // @ts-expect-error Argument of type 'this' is not assignable to parameter of 'this'
  return sealed.guard(true, (b) => b);
}
