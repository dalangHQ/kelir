# WO5 report — spec lane (Agent B, branch temp/wo5-spec). Format: status / nodes / decisions.
- 1 done. 08-lifecycle Pre-teardown-flush: session.terminate() nodes under Abrupt path and
  CLI and SDK (abrupt variant, kill() alias, no flush). TS/Py live; RS Session lacks it.
- 2 done (all five APIs; details below and item 20).
  - 02-plugin-model Core context primitives: c.step added. STALE CLAIM FIXED: live is
    HandlerCtx.step: StepStack, last() -> Option<SnapshotDescriptor>, not "StepHandle
    -> SnapshotDescriptor"; TS spells StepHandle.last().
  - 04-terminal-plane Disclosure: c.terminal.notice(msg) is the canonical TerminalNotice
    publisher (TerminalHandle::notice, handles_lifecycle.flow.rs:375).
  - 03 Session data operations + wire-ingress.md Service exports: session.handleIngress
    ({cwd?, handler}) -> AsyncGenerator (TS live, session.lifecycle.ts:120/327).
  - mesh.md: MeshCtx::await_winner(timeout) -> u32 (mesh_state.flow.rs:338).
- 4 done. db-proxy.md Key pin: false "reads c.cap.db" claim dropped; pinned Py
  c.cap.db_proxy.rewrite_target / TS c.cap.dbProxy.rewriteTarget. 03 Service lookup:
  stale Non-conforming node withdrawn; the mapping table is unambiguous.
- 7 no-change-needed. #Execution-receipts and #binary_blake3-identity both exist in
  10-attestation.md; decision: repoint the broken ref at #binary_blake3-identity (best
  semantic fit). All 8 artifacts/code targets exist. Note: 10-attestation.md:169 merges
  "- Lifecycle nano-tracer" into a see: line (pre-existing).
- 8 no-change-needed. 04 step 4 names on_console_output; its child mandates the
  MAX_PLACEHOLDER_LEN - 1 carryover window — the examples are the bug.
- 9 done. 03 Session surfaces: session.vm pinned as property (TS readonly vm; Py
  attribute) over vm/terminal/storage/wire/checkpoint/host/vsock/step. 09-snapshots:
  session.vm().export_snapshot() fixed to the property form.
- 10 done. 02 Chainable verbs trap-filter string form + Verbs-in-both-modes bare-string
  note. DECISION: From<&str> verified builder_sdk.flow.rs:54-66 — "*" is match-all,
  other strings are TrapFilter::binary(s); the work order's "pattern(str)" is stale.
  impl Into<TrapFilter> on trap/trap_async/post_exec (builder_plugin.flow.rs:111/580/596).
- 11 done. 05-storage-plane Write snoops node (after In-flight read masking). DECISION:
  live SDKs diverge — Py SnoopRule(glob=, sync=, on_write=) vs TS snoops[{path, sync,
  onWrite}] — mirrored both live spellings, not the work order's uniform `path`.
- 12 done. 08 Suspend-and-resume per-language shapes: RS enum; TS factory
  SuspendReason.external("...")/.resourcePressure; Py External(ExternalSuspendKind.X) /
  ResourcePressure(), both instantiated classes (verified py __init__.py:229-248).
- 13 done. 02 Unified verdict Constructor pin: TS object-shaped FilterAction.Suspend
  ({ until })/.Abort({ reason }) (live TS positional today; code lane converges); Py
  kwargs suspend(until=...)/abort(reason=...).
- 14 done. 02 Read/console pin: on_fs_write buffer — RS &mut [u8]; TS Buffer (bridge
  copies back on return); Py bytearray (bytes is read-only).
- 15 done. 08 Watchdog per-language shapes: RS (Duration, WatchdogAction) positional; TS
  setWatchdog({timeoutMs, action}); Py kwargs (timedelta, WatchdogAction; py lane adds
  the enum). WatchdogAction RS = Reboot | Terminate{emit_diagnostic}; TS string union.
- 16 done. 03 Unified builder surface on_trap_deny return pin: RS Result<(), 
  PluginError>; TS Promise<void> (throws); Py Awaitable[None] (raises); session errors.
- 17 done. DECISION: documented in 01-architecture.md#Embedded-profile (the code's see:
  points there). Live: default_vm_config(vcpus, memory_mb, kernel, rootfs_erofs,
  tsc_locked, enable_watchdog) -> VmConfig, all Option; defaults 1/256/empty/true/false.
- 18 done. 02 KelirPlugin trait: fork_state runs on the async runtime thread; clone
  without blocking (try_lock, atomics, Arc::clone), never blocking_lock/block_on.
- 19 done. 02 Mount is a merge: Satisfies is order-sensitive; Deps order must match the
  mounted order; canonical example mounts LedgerCtx first (hlist_sdk.flow.rs:45 E0207).
- 20 done. ledger.md Ledger handler API: full five-method table with per-SDK spellings
  (RS/Py count_denials, TS countDenials; all five verified in plugin-ledger lib.rs
  :133/:156/:164/:171/:178); finalize/get_causal_delta point at existing pins.

Gates: new/edited lines <= 90 chars; ref checker zero new failures vs baseline (failure
set byte-identical to untouched main checkout); abstraction leak 04-06 -> plugins/ clean;
siblings <= 20 per edited region (02 root's 21 pre-existing); no new TBDs. CONVERGENCE:
node renames + added nodes drift generated crates/kelir-testkit/anchors.json (out of
spec-lane scope) — run `just codegen` before check-coherence.
