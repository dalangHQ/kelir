// see: cans/02-plugin-model.md#Unified-builder
// see: cans/03-sdk-architecture.md#Unified-builder-surface
// see: cans/artifacts/examples/fluent-builder-chaos.ts#buildChaosPlugin
// see: cans/artifacts/examples/unified-builder-surface.rs#build_surface_plugin
// see: cans/artifacts/examples/unified-builder-surface.py#build_surface_plugin

// The unified builder surface, walked end to end in one file: structural
// verbs, all 24 ring verbs flat, all 8 scope closures, the four chain cases
// (seal, trap filter, trap_scope, lock_broker) and the four mount cases
// (host-only verbs on a host builder, host-only verbs on a plugin builder,
// host.use(builder) merge ordering, nested-scope = flat-chain equivalence).
// Handlers are trivial stubs: Forward for filter verbs, Pass for verdict
// verbs, passthrough bytes for console/exec, no-ops for lifecycle and
// telemetry. L1/L2: descriptors only, no execution, no boot() on a host.

import {
  createPlugin,
  EConfig,
  FilterAction,
  kelir,
  t,
  TrapFilter,
  Verdict,
  type KelirBuilder,
} from "@kelir/core";

// see: cans/02-plugin-model.md#Unified-builder
// Structural verbs: context, requires, scope, forkWith, vsockService.
// Then the 24 ring verbs flat, then the 8 scope closures.
export function buildSurfacePlugin(): KelirBuilder<
  "surface",
  { surface: boolean },
  "ledger",
  unknown
> {
  return createPlugin("surface")
    .context(() => ({ surface: true }))
    .requires<"ledger">()
    .scope({ paths: ["/workspace"] })
    .forkWith(
      (): KelirBuilder<"surface", { surface: boolean }, "ledger", unknown> =>
        buildSurfacePlugin(),
    )
    .vsockService(0x0013, "echo", t.Object({}), async (request) => request)
    .onTrap((_event, _c) => Verdict.Pass)
    .onProcessExit((_event) => {})
    .onFsLookup((_path) => Verdict.Pass)
    .onFsRead((_path, buffer) => buffer)
    .onFsWrite((_path, _buffer, _pid, _ctx) => FilterAction.Forward)
    .onFsLock((_path, _flockCmd, _flockType, _pid) => Verdict.Pass)
    .onNetOutbound((_frame) => FilterAction.Forward)
    .onNetInbound((_frame) => FilterAction.Forward)
    .onConsoleInput((_port, bytes) => bytes)
    .onConsoleOutput((_port, chunk) => chunk)
    .onExecStdout((_pid, _execId, chunk) => chunk)
    .onExecStderr((_pid, _execId, chunk) => chunk)
    .onHostLsmExec((_path, _pid) => Verdict.Pass)
    .onHostPressure((_level) => Verdict.Pass)
    .onHostKeyringAccess((_key) => Verdict.Pass)
    .onVmInit((_ctx) => {})
    .onVmReady((_ctx) => {})
    .onVmShutdown((_ctx) => {})
    .onPreCommit((_ctx) => {})
    .onPostCommit((_ctx, _committed) => {})
    .onHostPreempt((_ctx) => {})
    .onMemoryPressure((_availableMb, _totalMb) => {})
    .onHeartbeat((_event) => {})
    .onWatchdog((_event) => "execute-default")
    .trap((scope) =>
      scope
        .onTrap((_event, _c) => Verdict.Pass)
        .onProcessExit((_event) => {}),
    )
    .fs((f) =>
      f
        .onFsLookup((_path) => Verdict.Pass)
        .onFsRead((_path, buffer) => buffer)
        .onFsWrite((_path, _buffer, _pid, _ctx) => FilterAction.Forward)
        .onFsLock((_path, _flockCmd, _flockType, _pid) => Verdict.Pass),
    )
    .net((n) =>
      n
        .onNetOutbound((_frame) => FilterAction.Forward)
        .onNetInbound((_frame) => FilterAction.Forward),
    )
    .console((c) =>
      c
        .onConsoleInput((_port, bytes) => bytes)
        .onConsoleOutput((_port, chunk) => chunk),
    )
    .exec((e) =>
      e
        .onExecStdout((_pid, _execId, chunk) => chunk)
        .onExecStderr((_pid, _execId, chunk) => chunk),
    )
    .host((h) =>
      h
        .onHostLsmExec((_path, _pid) => Verdict.Pass)
        .onHostPressure((_level) => Verdict.Pass)
        .onHostKeyringAccess((_key) => Verdict.Pass)
        .onHostPreempt((_ctx) => {}),
    )
    .lifecycle((l) =>
      l
        .onVmInit((_ctx) => {})
        .onVmReady((_ctx) => {})
        .onVmShutdown((_ctx) => {})
        .onPreCommit((_ctx) => {})
        .onPostCommit((_ctx, _committed) => {}),
    )
    .telemetry((t) =>
      t
        .onMemoryPressure((_availableMb, _totalMb) => {})
        .onHeartbeat((_event) => {})
        .onWatchdog((_event) => "execute-default"),
    );
}

// see: cans/02-plugin-model.md#Unified-builder
// The ledger provider the surface plugin requires: the name "ledger" is
// what satisfies requires<"ledger"> at seal time. One trivial verb.
export function buildLedgerPlugin(): KelirBuilder<"ledger", unknown, never, unknown> {
  return createPlugin("ledger").onHeartbeat((_event) => {});
}

// see: cans/02-plugin-model.md#Unified-builder
// The merge-ordering second mount: no requires, one trivial verb.
export function buildPlainPlugin(): KelirBuilder<"plain", unknown, never, unknown> {
  return createPlugin("plain").onHeartbeat((_event) => {});
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount case 1: host-only verbs on a host builder (terminal, storage,
// wire, detached, lockBroker). Chain cases: trap filter, trap scope,
// lockBroker, then seal() in main. Mount case 3: merge ordering — the
// ledger provider first (it satisfies the surface plugin's requires at
// seal time), then the surface plugin via plugin(), then the plain plugin
// via use(), the same verb under its second name.
export function buildSurfaceHost() {
  return kelir({
    vcpus: 2,
    memoryMb: 2048,
    kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
    rootfsErofs: "./fixtures/rootfs-surface.erofs",
    tscLocked: true,
    enableWatchdog: true,
    driver: "auto",
  })
    .terminal({ cols: 120, rows: 40 })
    .storage({ idmap: { guestUid: 0, hostUid: 0 }, lockBroker: true })
    .wire({})
    .detached(false)
    .use(buildLedgerPlugin())
    .plugin(buildSurfacePlugin())
    .use(buildPlainPlugin())
    .trap(TrapFilter.binary("surface-allow"), (c) => c.pass())
    .trap((scope) =>
      scope
        .onTrap((_event, _c) => Verdict.Pass)
        .onProcessExit((_event) => {}),
    )
    .lockBroker({ globs: ["**/Cargo.lock", "**/package-lock.json"], nonBlocking: true });
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount case 2: host-only verbs on a plugin builder, expecting EConfig at
// the call site. Live TypeScript rejects exactly these six verbs plus
// boot() below; seal(), use(), on(), onTrapDeny(), registerCustomAction()
// and registerStreamKind() do not throw on a plugin-mode builder live,
// although the spec lists them host-only, so they are not probed here.
export function hostOnlyVerbsRejectedOnPlugin(): string[] {
  const rejected: string[] = [];
  const plugin = createPlugin("surface");
  const probe = (verb: string, call: () => void): void => {
    try {
      call();
    } catch (error) {
      if (error instanceof EConfig) {
        rejected.push(verb);
      }
    }
  };
  probe("terminal", () => {
    plugin.terminal({ cols: 120, rows: 40 });
  });
  probe("storage", () => {
    plugin.storage({ lockBroker: true });
  });
  probe("wire", () => {
    plugin.wire({});
  });
  probe("lockBroker", () => {
    plugin.lockBroker({ globs: ["**/Cargo.lock"] });
  });
  probe("params", () => {
    plugin.params(t.Object({}));
  });
  probe("detached", () => {
    plugin.detached(true);
  });
  return rejected;
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount case 2, async leg: boot() on a plugin-mode builder throws EConfig.
export async function hostOnlyBootRejectedOnPlugin(): Promise<boolean> {
  const plugin = createPlugin("surface");
  try {
    await plugin.boot();
    return false;
  } catch (error) {
    return error instanceof EConfig;
  }
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount case 4, scoped form: the same verbs as buildFlatPlugin, grouped
// into the 8 scope closures. Scope groups organise the chain only; ring
// membership and descriptors are unchanged by scope use.
export function buildScopedPlugin() {
  return createPlugin("surface-scoped")
    .trap((scope) =>
      scope
        .onTrap((_event, _c) => Verdict.Pass)
        .onProcessExit((_event) => {}),
    )
    .fs((f) =>
      f
        .onFsLookup((_path) => Verdict.Pass)
        .onFsRead((_path, buffer) => buffer)
        .onFsWrite((_path, _buffer, _pid, _ctx) => FilterAction.Forward)
        .onFsLock((_path, _flockCmd, _flockType, _pid) => Verdict.Pass),
    )
    .net((n) =>
      n
        .onNetOutbound((_frame) => FilterAction.Forward)
        .onNetInbound((_frame) => FilterAction.Forward),
    )
    .console((c) =>
      c
        .onConsoleInput((_port, bytes) => bytes)
        .onConsoleOutput((_port, chunk) => chunk),
    )
    .exec((e) =>
      e
        .onExecStdout((_pid, _execId, chunk) => chunk)
        .onExecStderr((_pid, _execId, chunk) => chunk),
    )
    .host((h) =>
      h
        .onHostLsmExec((_path, _pid) => Verdict.Pass)
        .onHostPressure((_level) => Verdict.Pass)
        .onHostKeyringAccess((_key) => Verdict.Pass)
        .onHostPreempt((_ctx) => {}),
    )
    .lifecycle((l) =>
      l
        .onVmInit((_ctx) => {})
        .onVmReady((_ctx) => {})
        .onVmShutdown((_ctx) => {})
        .onPreCommit((_ctx) => {})
        .onPostCommit((_ctx, _committed) => {}),
    )
    .telemetry((t) =>
      t
        .onMemoryPressure((_availableMb, _totalMb) => {})
        .onHeartbeat((_event) => {})
        .onWatchdog((_event) => "execute-default"),
    );
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount case 4, flat form: the same 24 verbs as buildScopedPlugin, with
// no scope grouping. A nested chain and the equivalent flat chain produce
// identical descriptors: same rings, same ordinals.
export function buildFlatPlugin() {
  return createPlugin("surface-flat")
    .onTrap((_event, _c) => Verdict.Pass)
    .onProcessExit((_event) => {})
    .onFsLookup((_path) => Verdict.Pass)
    .onFsRead((_path, buffer) => buffer)
    .onFsWrite((_path, _buffer, _pid, _ctx) => FilterAction.Forward)
    .onFsLock((_path, _flockCmd, _flockType, _pid) => Verdict.Pass)
    .onNetOutbound((_frame) => FilterAction.Forward)
    .onNetInbound((_frame) => FilterAction.Forward)
    .onConsoleInput((_port, bytes) => bytes)
    .onConsoleOutput((_port, chunk) => chunk)
    .onExecStdout((_pid, _execId, chunk) => chunk)
    .onExecStderr((_pid, _execId, chunk) => chunk)
    .onHostLsmExec((_path, _pid) => Verdict.Pass)
    .onHostPressure((_level) => Verdict.Pass)
    .onHostKeyringAccess((_key) => Verdict.Pass)
    .onVmInit((_ctx) => {})
    .onVmReady((_ctx) => {})
    .onVmShutdown((_ctx) => {})
    .onPreCommit((_ctx) => {})
    .onPostCommit((_ctx, _committed) => {})
    .onHostPreempt((_ctx) => {})
    .onMemoryPressure((_availableMb, _totalMb) => {})
    .onHeartbeat((_event) => {})
    .onWatchdog((_event) => "execute-default");
}

// see: cans/02-plugin-model.md#Unified-builder
async function main(): Promise<void> {
  buildSurfaceHost().seal();
  const rejected = hostOnlyVerbsRejectedOnPlugin();
  if (rejected.length !== 6) {
    throw new Error(
      `host-only rejection: expected 6 verbs, got ${rejected.length}: ${rejected.join(", ")}`,
    );
  }
  if (!(await hostOnlyBootRejectedOnPlugin())) {
    throw new Error("host-only rejection: boot() did not throw EConfig on a plugin builder");
  }
  buildScopedPlugin();
  buildFlatPlugin();
}

main().catch(console.error);
