# WO5 report — ts lane (Agent E, branch temp/wo5-ts)

Scope: packages/ts/** only; base 7939d50 → 4 item commits + report. No tsc here — merge gate owns it; parse/brace/comment-law checks clean.

## Item 13 — done
- verdict.protocol.ts:16-17,26-31 (Verdict), 44-45,51-56 (FilterAction):
  `Suspend({ until }: { until: SuspendUntil })` / `Abort({ reason }: { reason?: string })`
  (decl + impl). Readonly unions, Drop/Patch untouched (not pinned).
- Call sites swept (src+tests+e2b-compat): only matrix.hook.test.ts:108 →
  `FilterAction.Suspend({ until: SuspendUntil.deadline(0) })`; no Abort callers.
- Decision: TrapContext `c.suspend(until)` / `c.abort(reason?)` stay positional
  (context.lifecycle.types.ts:120-121) — contract pins only Verdict/FilterAction.

## Item 16 — done
- builder.lifecycle.ts:440-452 (onTrapDeny run): result flows through
  `Promise.resolve(...).catch((error) => { throw EConfig.fromNative(error); })`.
  Rejects rethrow (never swallowed); envelope-shaped rejects map via
  EConfig.fromNative (errors.sdk.ts:60-64, verified static); sync throws stay
  unmapped; void normalizes to Promise<void>. Existing deny tests touch only
  registration/count/verb paths — unaffected.

## Item 17 — done (TODO form; anchor absent at check time)
- builder.config.ts:6: `// TODO (WO5 item 17): ...` on KelirConfig — no
  default_vm_config node in cans/03-sdk-architecture.md yet (verified live;
  01-architecture.md#Embedded-profile covers preset values, not the helper);
  flip to `see:` once the spec lane lands the node.
- Decision: builder.config.ts only — session.lifecycle.ts carries no vm-config
  defaults (work-order's "add a see: in session.lifecycle.ts" is stale).

## Item 2 — done
- context.lifecycle.types.ts:98-99: MeshHandle grows
  `awaitWinner(timeoutMs: number): Promise<number>` + TODO (L3) one-liner.
- Decision: interface member only — no native mesh decl anywhere in packages/ts
  (NativeSessionHandle, generated decls, e2b-compat clean); TS never constructs
  a MeshHandle (native pluginContexts materialize it) so no throwing-stub site;
  mirrors Rust `await_winner(timeout) -> u32` (mesh_state.flow.rs:338) as ms.

## Items 20, 1 — no-change (verified)
- LedgerHandle.countDenials(ppid): Promise<number> exists
  (context.lifecycle.types.ts:31) — matches `ledgerHandle.countDenials(pid)`.
- Session.terminate() exists (session.lifecycle.ts:305-307, native decl :110);
  Sandbox.terminate too (sandbox.runtime.ts:177).

## Risks
- No tsc here: item 13 signatures are standard TS, parse-verified only.
  tsconfig.check.json includes cans/artifacts/examples/*.ts → examples lane
  must match pinned Suspend/Abort shapes (they will) and awaitWinner
  positionally (my decision; if examples went object-shaped, reconcile).
- Item 17 TODO raises the TS TODO count by 1 until the see: swap.
- Item 16: fused JS hooks stay fire-and-forget (L3 TODO builder.lifecycle.ts:913).
