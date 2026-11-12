# CHAIN-A4-DECISIONS — TypeScript builder-chain fix (L1/L2)

Track A4, worktree `~/workspace/kelir-chain-a4`, branch `chain-a4`, base `8092fd5`.
Scope: `packages/ts/**` only. No `cans/` edits; no token; no push.

## Decisions

1. Factories: all 13 plain-object factories in `plugins.registry.ts`
   (`chaos`, `container`, `dbProxy`, `ledger`, `mcp`, `mesh`, `profile`,
   `replay`, `supplyChain`, `toolGuard`, `transaction`, `vault`,
   `wireIngress`) now return `createPlugin(...)` builders. The duplicate
   `*Builder` set is deleted — verified zero usages repo-wide by grep
   before deleting (only its own definitions existed).
   `enabled` is preserved via `CreatePluginOptions { enabled, config }`;
   the old `*Builder` set silently dropped `enabled: false` (it passed
   `{ config }`, so `use()` read `enabled` as `undefined`), fixed as a
   side effect. `chaos`/`ledger`/`mesh` keep their typed contexts
   (`{ chaos: ChaosHandle }` etc.), so `.use()` typing is unchanged.
2. `HookRegistry`: all 24 `on*` plus `addHook`/`adoptHook`/`setScope`/
   `validate` return `this`. `recordHostOnly`/`hostOnlyCalls` are kept
   as a backstop (`validate()` and `use()` still check them), though
   builder verbs now throw before anything is recorded.
3. `seal(): this` is idempotent: re-sealing is a no-op success. This is
   required, not lenient — `boot()`/`fuse()` call `seal()` on a builder
   the user may already have sealed. Every verb goes through
   `checkVerb` (sealed, then mounted). `fuse()` is `seal()` + fuse.
   `boot()` stays the single terminal (it fuses internally). `fuse()`
   stays public — tests and `boot()` use it — but its role is now
   validation + fusion, not a terminal.
4. `.plugin()` got its own `checkVerb(".plugin")` guard: it delegates
   to `use()`, whose guard would otherwise report the wrong verb name
   (`.use`) in a sealed error.
5. Host-only verbs (`terminal`, `storage`, `wire`, `params`, `detached`,
   `lockBroker`) throw `EConfig` immediately at the call site on a
   plugin-mode builder. The old mount-time failure test is updated to
   assert the call-site throw.
6. `trap(handlers)`: a single object argument with no `snapshot()`
   method is treated as `TrapHandlers`; `pre`/`post` register
   unfiltered. The old code crashed here (`filterOrFn.snapshot()` on a
   plain object). Overload order is filter, scope-fn, handlers: the
   scope-fn overload must precede the handlers overload, or a scope
   function would match `TrapHandlers` (all-optional props) and break
   contextual typing of `scope`.
7. `group(fn)`: runs `fn` on the same builder — no sub-builder, no
   isolation. Guard (cheap identity check): a non-`undefined` return
   that is not the receiving builder throws `EConfig`.
8. Scopes: `.end(): KelirBuilder` on all 8 scopes via an `Owner`
   generic (`TrapScope<Ctx, Owner>` etc.). The registry scope methods
   take `endTarget: () => Owner` per call, so `clone()` needs no
   rebinding. Closure type is `(scope) => Scope | void`, so a block
   body forgetting `return scope` typechecks. `.end()` is for block
   bodies: an expression body returning `.end()` yields the builder,
   which is not `Scope | void` — by design, per the specified type.
9. `forkState()`: name kept (renaming breaks the public surface; the
   task said keep). It is a factory, not a chain verb: it returns a
   fresh unsealed builder (`clone()` does not copy `sealedFlag`), and
   it is allowed after `seal()`. Factory nature is recorded here and
   in `matrix.builder-chain.test.ts`.
10. `createPlugin` gains a 5th `Params` generic (default `unknown`);
    all 13 factories thread a `P = unknown` through it.
11. `lockBroker(config)`: host-only verb mirroring Rust
    `KelirBuilder<Host, Unsealed>::lockBroker` (`LockBrokerConfig`
    `{ scope: LockScope::Globs, non_blocking }`); TS shape is
    `{ globs, nonBlocking? }`. `boot()` forwards `lockBroker: true`
    via `setStorageConfig` when a broker config is set, even with no
    `storageCfg`. Globs/`nonBlocking` forwarding is L3 (see open gaps).
12. `hooksForFusion()`: read-only query — validates and describes the
    fused descriptor without sealing. A query that sealed would
    surprise; `fuse()` is the sealing validation + fusion step.

## Open gaps (deliberately left, not faked)

- `lockBroker` globs/`nonBlocking` have no native setter yet, so only
  the `lockBroker: true` boolean reaches the NAPI addon at `boot()`.
  Marked `// TODO (L3)` in `builder.lifecycle.ts`; full forwarding is
  L3 native work.
- `packages/ts/e2b-compat` was not rebuilt: it imports only `Sandbox`
  from `@kelir/core` (untouched file) and its workspace link is not set
  up in this worktree.

## Spec pins

None needed. All changes fit existing spec leaves (`seal`:
`cans/02-plugin-model.md#One-validation,-at-seal-and-boot`, present in
`anchors.json`; `lockBroker`:
`cans/05-storage-plane.md#Host-inode-lock-broker`). No `cans/` edit.

## Audit verification (live tree, before editing)

- `*Builder` set: 13 definitions, zero usages repo-wide.
- `HookRegistry.on*`: exactly 24 `void` methods, all converted.
- `trap(handlers)` crash: confirmed by reading the `snapshot()` call
  on the filter path; no `TrapHandlers`-only overload existed.
- Host-only verbs: record-and-defer confirmed (`hostOnlyRejected` +
  `hostOnlyCalls`, mount-time throw in `use()`/`validate()`).
- `cans/artifacts/examples/*.ts` use the plain factories via
  `.plugin()`; they still typecheck (builders are accepted by
  `.plugin()`). No example edits needed.

## Gates run in this worktree (before commit)

- Baseline (base `8092fd5`): `pnpm build` ok, `pnpm test` 30/30,
  `pnpm typecheck` (`tsc -p tsconfig.check.json`) exit 0.
- After change: `pnpm build` ok, `pnpm test` 45/45 (30 existing +
  15 new in `matrix.builder-chain.test.ts`), `pnpm typecheck` exit 0.
- Comment law: TS `//` grep clean. TODO markers: +1 (lockBroker L3
  marker), −0 (none deleted).
- LOC: `builder.lifecycle.ts` 997, `hook.registry.ts` 927,
  `plugins.registry.ts` 284 (all ≤ 1000).
