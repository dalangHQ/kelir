// see: cans/02-plugin-model.md#Unified-builder
// see: cans/02-plugin-model.md#Nested-scopes
// see: cans/03-sdk-architecture.md#Unified-builder-surface
// see: cans/artifacts/examples/fluent-builder-chaos.ts#buildChaosPlugin
// see: cans/artifacts/examples/unified-builder-surface.rs#build_surface_plugin
// see: cans/artifacts/examples/unified-builder-surface.py#build_surface_plugin

// The unified builder surface, walked end to end in one file: structural
// verbs, all 24 ring verbs reached through their 8 scope closures, the
// five orphan-verb scopes (actions, streams, vsock, params, deny), the
// chain cases (seal, trap filter, trap scope, fs config) and the mount
// cases (host-only verbs on a host builder, host-only verbs on a plugin
// builder, host.use(builder) merge ordering, flat-fold = nested-scope
// equivalence via group()). Handlers are trivial stubs: Forward for
// filter verbs, Pass for verdict verbs, passthrough bytes for
// console/exec, no-ops for lifecycle and telemetry. L1/L2: descriptors
// only, no execution, no boot() on a host. There is no .end() in
// TypeScript: scope closures return the scope or void.

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

// see: cans/02-plugin-model.md#Nested-scopes
// Structural verbs: context, requires, scope, forkWith. The vsock service
// and the 24 ring verbs are scope-reached: .vsock() carries the service,
// the 8 ring scopes carry the verbs.
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
    .vsock((v) => v.service(0x0013, "echo", async (request) => request))
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
    .telemetry((tm) =>
      tm
        .onMemoryPressure((_availableMb, _totalMb) => {})
        .onHeartbeat((_event) => {})
        .onWatchdog((_event) => "execute-default"),
    );
}

// see: cans/02-plugin-model.md#Unified-builder
// The ledger provider the surface plugin requires: the name "ledger" is
// what satisfies requires<"ledger"> at seal time. One trivial verb.
export function buildLedgerPlugin(): KelirBuilder<"ledger", unknown, never, unknown> {
  return createPlugin("ledger").telemetry((t) => t.onHeartbeat((_event) => {}));
}

// see: cans/02-plugin-model.md#Unified-builder
// The merge-ordering second mount: no requires, one trivial verb.
export function buildPlainPlugin(): KelirBuilder<"plain", unknown, never, unknown> {
  return createPlugin("plain").telemetry((t) => t.onHeartbeat((_event) => {}));
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount case 1: host-only verbs on a host builder (terminal, storage,
// wire, detached; the lock-broker config rides .fs((f) => f.config(…))).
// Chain cases: trap filter, trap scope, fs config, then seal() in main.
// Mount case 3: merge ordering — the ledger provider first (it satisfies
// the surface plugin's requires at seal time), then the surface plugin
// via plugin(), then the plain plugin via use(), the same verb under its
// second name.
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
    // see: cans/02-plugin-model.md#TrapFilter-predicates
    // The filter call site carries the full predicate chain (withArg,
    // withCwd, withEnv, withUid, argvMatches — AND-ed); the handler
    // body needs no cwd/argv guard.
    .trap(
      TrapFilter.binary("surface-allow")
        .withArg("run")
        .withCwd("/workspace/**")
        .withEnv("CI", "1")
        .withUid(0)
        .argvMatches(/^--fast$/),
      (c) => c.pass(),
    )
    .trap((scope) =>
      scope
        .onTrap((_event, _c) => Verdict.Pass)
        .onProcessExit((_event) => {}),
    )
    // see: cans/02-plugin-model.md#Nested-scopes
    // The host-only fs scope sub-nodes: config carries lockBroker,
    // carryoverBytes, scanEngine, snoops; requires / forkWith / timeout /
    // priority / rules pin the shared scope shape. The standalone
    // lockBroker verb folds into this config.
    .fs((f) =>
      f
        .config({
          lockBroker: { globs: ["**/Cargo.lock", "**/package-lock.json"], nonBlocking: true },
          carryoverBytes: 1024,
          scanEngine: "FastRegex",
          snoops: [],
        })
        .requires<"ledger">()
        .forkWith(() => buildPlainPlugin())
        .timeout(0.005)
        .priority(0)
        .rules({
          read: { allow: ["/workspace/**"], deny: ["/workspace/.policy/identity"] },
          write: { mask: ["sk_live_[A-Za-z0-9]{24}"] },
        })
        .onFsLookup((_path) => Verdict.Pass)
        .onFsWrite((_path, _buffer, _pid, _ctx) => FilterAction.Forward),
    )
    // see: cans/02-plugin-model.md#Nested-scopes
    // The five orphan-verb scopes on the host: actions, streams, vsock,
    // params, deny. The standalone registerCustomAction,
    // registerStreamKind, vsockService, params, onTrapDeny verbs delete;
    // these scopes are the one surface.
    .actions((a) =>
      a.register("quarantine", t.Object({}), async (payload) => {
        void payload;
      }),
    )
    .streams((s) =>
      s.register(0x0013, t.Object({}), async (request) => new Uint8Array(request)),
    )
    .vsock((v) => v.service(0x0013, "echo", async (request) => request))
    .params((p) => p.model(t.Object({})))
    .deny((d) =>
      d.onTrapDeny((_event, _verdict, _session) => {}),
    );
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount case 2: host-only verbs on a plugin builder, expecting EConfig at
// the call site. Live TypeScript rejects exactly these six verbs plus
// boot() below; seal() and use() do not throw on a plugin-mode builder
// live, although the spec lists them host-only, so they are not probed
// here. The scope entries (.fs, .actions, .streams, .vsock, .params,
// .deny) are both-modes surfaces; their host-only sub-config is what the
// plugin-mode probe rejects.
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
  probe("fs.config.lockBroker", () => {
    plugin.fs((f) => f.config({ lockBroker: { globs: ["**/Cargo.lock"] } }));
  });
  probe("params", () => {
    plugin.params((p) => p.model(t.Object({})));
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

// see: cans/02-plugin-model.md#Nested-scopes
// Negative vector: f.onTrap(...) inside .fs(...) is a type error in
// TypeScript — the FsScope interface has no onTrap member. Uncommenting
// the body must fail tsc with "Property 'onTrap' does not exist on type
// 'FsScope<...>'". vectors.json cannot express an expected-error case
// (its schema carries event/expected_verdict only), so the negative case
// is pinned here as a comment-pinned block and in the rs/py siblings.
//
// export function negativeFsOnTrap(
//   builder: KelirBuilder<"surface", unknown, never, unknown>,
// ): KelirBuilder<"surface", unknown, never, unknown> {
//   return builder.fs((f) => f.onTrap((_event) => Verdict.Pass));
// }

// see: cans/02-plugin-model.md#Nested-scopes
// Flat-fold pin: a flat chain exists only as a group() fold over the
// scopes — a readability fold that produces the same descriptors (same
// rings, same ordinals). There is no separate flat surface: no
// buildFlatPlugin exists, the fold delegates each verb to its scope.
export function buildFlatFoldPlugin() {
  return createPlugin("surface-flat-fold").group((g) =>
    g
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
      .telemetry((tm) =>
        tm
          .onMemoryPressure((_availableMb, _totalMb) => {})
          .onHeartbeat((_event) => {})
          .onWatchdog((_event) => "execute-default"),
      ),
  );
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
  buildFlatFoldPlugin();
}

main().catch(console.error);
