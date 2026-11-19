// see: cans/03-sdk-architecture.md#TypeScript-SDK

import test from "node:test";
import assert from "node:assert/strict";

import {
  EConfig,
  FilterAction,
  PostExecAction,
  RingEvent,
  Verdict,
  chaos,
  container,
  createPlugin,
  dbProxy,
  kelir,
  ledger,
  mcp,
  mesh,
  profile,
  replay,
  supplyChain,
  t,
  toolGuard,
  transaction,
  vault,
  wireIngress,
  KelirBuilder,
} from "../dist/index.js";

function isSealedError(verb: string): (error: unknown) => boolean {
  return (error: unknown): boolean =>
    error instanceof EConfig &&
    error.message === `builder is sealed: ${verb} after .seal()`;
}

test("every chain verb returns the same builder", () => {
  const host = kelir();
  assert.equal(host.terminal({ cols: 80 }), host);
  assert.equal(host.storage({}), host);
  assert.equal(host.wire({}), host);
  assert.equal(
    host.params((p) => p.model(t.Object({ ping: t.String() }))),
    host,
  );
  assert.equal(host.detached(false), host);
  assert.equal(
    host.vsock((v) => v.service(1024, "ping", (request) => request)),
    host,
  );
  assert.equal(
    host.use(createPlugin<"inner", Record<string, never>>("inner")),
    host,
  );
  const plugin = createPlugin<"chain", Record<string, never>>("chain");
  assert.equal(
    plugin.trap((scope) => scope.onTrap(() => Verdict.Pass).onProcessExit(() => {})),
    plugin,
  );
  assert.equal(
    plugin.fs((scope) => scope.onFsLookup(() => Verdict.Pass).onFsWrite(() => FilterAction.Forward)),
    plugin,
  );
  assert.equal(
    plugin.net((scope) => scope.onNetOutbound(() => FilterAction.Forward)),
    plugin,
  );
  assert.equal(plugin.trap("git", (c) => c.pass()), plugin);
  assert.equal(plugin.trap((scope) => scope.onTrap(() => Verdict.Pass)), plugin);
  assert.equal(plugin.trap({ pre: () => Verdict.Pass }), plugin);
  assert.equal(plugin.lifecycle((scope) => scope.onVmInit(() => {})), plugin);
  assert.equal(plugin.telemetry((scope) => scope.onHeartbeat(() => {})), plugin);
  assert.equal(plugin.requires("ledger"), plugin);
  assert.equal(plugin.derive(() => ({ ready: true })), plugin);
  assert.equal(plugin.context(() => ({ mode: "guest" })), plugin);
  assert.equal(plugin.scope({ binaries: ["git"] }), plugin);
  assert.equal(plugin.forkWith(() => createPlugin<"chain", Record<string, never>>("chain")), plugin);
  assert.equal(plugin.group((b) => b.trap((scope) => scope.onTrap(() => Verdict.Pass))), plugin);
  assert.equal(plugin.guard(true, (b) => b.trap((scope) => scope.onTrap(() => Verdict.Pass))), plugin);
  assert.equal(plugin.guard(false, (b) => b.trap((scope) => scope.onTrap(() => Verdict.Pass))), plugin);
  assert.equal(plugin.seal(), plugin);
});

test("seal blocks every verb, and seal itself is idempotent", () => {
  const builder = kelir()
    .trap((scope) => scope.onTrap(() => Verdict.Pass))
    .seal();
  assert.equal(builder.seal(), builder);
  const verbs: readonly [string, () => unknown][] = [
    [".terminal", () => builder.terminal({ cols: 80 })],
    [".storage", () => builder.storage({})],
    [".wire", () => builder.wire({})],
    [".params", () => builder.params((p) => p.model(t.Object({ ping: t.String() })))],
    [".detached", () => builder.detached(true)],
    [".use", () => builder.use(createPlugin<"x", Record<string, never>>("x"))],
    [".plugin", () => builder.plugin(createPlugin<"x", Record<string, never>>("x"))],
    [".trap", () => builder.trap("git", (c) => c.pass())],
    [".fs", () => builder.fs((scope) => scope.onFsLookup(() => Verdict.Pass))],
    [".net", () => builder.net((scope) => scope.onNetOutbound(() => FilterAction.Forward))],
    [".console", () => builder.console((scope) => scope.onConsoleInput(() => new Uint8Array(0)))],
    [".exec", () => builder.exec((scope) => scope.onExecStdout(() => new Uint8Array(0)))],
    [".host", () => builder.host((scope) => scope.onHostLsmExec(() => Verdict.Pass))],
    [".lifecycle", () => builder.lifecycle((scope) => scope.onVmInit(() => {}))],
    [".telemetry", () => builder.telemetry((scope) => scope.onHeartbeat(() => {}))],
    [".actions", () => builder.actions((a) => a.register("act", t.Object({}), () => {}))],
    [".streams", () => builder.streams((s) => s.register(7, t.Object({}), () => new Uint8Array(0)))],
    [".vsock", () => builder.vsock((v) => v.service(1024, "ping", (request) => request))],
    [".deny", () => builder.deny((d) => d.onTrapDeny(() => {}))],
    [".requires", () => builder.requires("ledger")],
    [".derive", () => builder.derive(() => ({ ready: true }))],
    [".scope", () => builder.scope({ binaries: ["git"] })],
    [".group", () => builder.group((b) => b.trap((scope) => scope.onTrap(() => Verdict.Pass)))],
    [".guard", () => builder.guard(true, (b) => b.trap((scope) => scope.onTrap(() => Verdict.Pass)))],
  ];
  for (const [verb, run] of verbs) {
    assert.throws(run, isSealedError(verb));
  }
  const sealedPlugin = createPlugin<"sealed", Record<string, never>>("sealed")
    .trap((scope) => scope.onTrap(() => Verdict.Pass))
    .seal();
  assert.throws(
    () => sealedPlugin.trap((scope) => scope.onTrap(() => Verdict.Pass)),
    isSealedError(".trap"),
  );
});

test("seal runs validation: bad specs fail at seal", () => {
  assert.throws(
    () =>
      createPlugin<"bad-port", Record<string, never>>("bad-port")
        .vsock((v) => v.service(0, "ping", (request) => request))
        .seal(),
    EConfig,
  );
  const good = createPlugin<"good", Record<string, never>>("good")
    .trap((scope) => scope.onTrap(() => Verdict.Pass))
    .seal();
  assert.equal(good.fuse().hookCount(RingEvent.Trap), 1);
});

test("fuse seals: verbs after fuse throw", () => {
  const builder = kelir().trap((scope) => scope.onTrap(() => Verdict.Pass));
  assert.equal(builder.fuse().hookCount(RingEvent.Trap), 1);
  assert.throws(
    () => builder.trap((scope) => scope.onTrap(() => Verdict.Pass)),
    isSealedError(".trap"),
  );
});

test("hooksForFusion validates without sealing", () => {
  const builder = kelir().trap((scope) => scope.onTrap(() => Verdict.Pass));
  const first = builder.hooksForFusion();
  assert.equal(first.rings.length, 1);
  assert.equal(first.rings[0].hookCount, 1);
  builder.trap((scope) => scope.onTrap(() => Verdict.Pass));
  assert.equal(builder.hooksForFusion().rings[0].hookCount, 2);
  assert.throws(
    () =>
      createPlugin<"bad-port", Record<string, never>>("bad-port")
        .vsock((v) => v.service(0, "ping", (request) => request))
        .hooksForFusion(),
    EConfig,
  );
});

test("trap(handlers) registers pre and post hooks with no filter", () => {
  const plugin = createPlugin<"handled", Record<string, never>>("handled").trap({
    pre: () => Verdict.Pass,
    post: () => PostExecAction.Proceed,
  });
  const descriptor = plugin.hooksForFusion();
  assert.equal(descriptor.rings.length, 1);
  assert.equal(descriptor.rings[0].ring, RingEvent.Trap);
  assert.deepEqual(
    descriptor.rings[0].hooks.map((hook) => hook.stage),
    ["pre", "post"],
  );
  assert.deepEqual(
    descriptor.rings[0].hooks.map((hook) => hook.filter),
    [undefined, undefined],
  );
  const empty = createPlugin<"empty", Record<string, never>>("empty").trap({});
  assert.equal(empty.fuse().hookCount(RingEvent.Trap), 0);
});

test("end() returns the builder from all 8 scopes", () => {
  const plugin = createPlugin<"ended", Record<string, never>>("ended")
    .trap((scope) => {
      scope.onTrap(() => Verdict.Pass).end().trap((s) => s.onTrap(() => Verdict.Pass));
    })
    .fs((scope) => {
      scope.onFsLookup(() => Verdict.Pass).end().fs((s) => s.onFsLookup(() => Verdict.Pass));
    })
    .net((scope) => {
      scope.onNetOutbound(() => FilterAction.Forward).end().net((s) => s.onNetOutbound(() => FilterAction.Forward));
    })
    .console((scope) => {
      scope.onConsoleInput(() => new Uint8Array(0)).end().console((s) => s.onConsoleInput(() => new Uint8Array(0)));
    })
    .exec((scope) => {
      scope.onExecStdout(() => new Uint8Array(0)).end().exec((s) => s.onExecStdout(() => new Uint8Array(0)));
    })
    .host((scope) => {
      scope.onHostLsmExec(() => Verdict.Pass).end().host((s) => s.onHostLsmExec(() => Verdict.Pass));
    })
    .lifecycle((scope) => {
      scope.onVmInit(() => {}).end().lifecycle((s) => s.onVmInit(() => {}));
    })
    .telemetry((scope) => {
      scope.onHeartbeat(() => {}).end().telemetry((s) => s.onHeartbeat(() => {}));
    });
  const descriptor = plugin.hooksForFusion();
  const total = descriptor.rings.reduce((sum, ring) => sum + ring.hookCount, 0);
  assert.equal(total, 16);
  assert.equal(plugin.fuse().hookCount(RingEvent.Trap), 2);
  assert.equal(plugin.seal(), plugin);
  assert.throws(
    () =>
      plugin
        .seal()
        .fs((scope) => scope.onFsLookup(() => Verdict.Pass)),
    isSealedError(".fs"),
  );
});

test("scope block bodies need no return", () => {
  const plugin = createPlugin<"no-return", Record<string, never>>("no-return")
    .trap((scope) => {
      scope.onTrap(() => Verdict.Pass);
    })
    .fs((scope) => {
      scope.onFsLookup(() => Verdict.Pass);
    });
  assert.equal(plugin.fuse().hookCount(RingEvent.Trap), 1);
  assert.equal(plugin.fuse().hookCount(RingEvent.FsLookup), 1);
});

test("group runs the block on the same builder and rejects a foreign builder", () => {
  const builder = kelir().group((inner) => {
    inner.trap((scope) => scope.onTrap(() => Verdict.Pass));
    inner.fs((scope) => scope.onFsWrite(() => FilterAction.Forward));
  });
  assert.equal(builder.fuse().hookCount(RingEvent.Trap), 1);
  assert.equal(builder.fuse().hookCount(RingEvent.FsWrite), 1);
  const returned = kelir().group((inner) => inner.trap((scope) => scope.onTrap(() => Verdict.Pass)));
  assert.equal(returned.fuse().hookCount(RingEvent.Trap), 1);
  assert.throws(
    () => kelir().group(() => kelir()),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "group() must return the builder it receives",
  );
});

test("guard applies the block only when enabled", () => {
  const skipped = kelir().guard(false, (inner) => inner.trap((scope) => scope.onTrap(() => Verdict.Pass)));
  assert.equal(skipped.fuse().hookCount(RingEvent.Trap), 0);
  const applied = kelir().guard(true, (inner) => inner.trap((scope) => scope.onTrap(() => Verdict.Pass)));
  assert.equal(applied.fuse().hookCount(RingEvent.Trap), 1);
  const returned = kelir().guard(true, (inner) => inner.trap((scope) => scope.onTrap(() => Verdict.Pass)));
  assert.equal(returned.fuse().hookCount(RingEvent.Trap), 1);
  assert.throws(
    () => kelir().guard(true, () => kelir()),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "guard() must return the builder it receives",
  );
});

test("lockBroker rides .fs((f) => f.config(...))", () => {
  const routed = kelir()
    .fs((f) =>
      f.config({ lockBroker: { globs: ["/workspace/**"], nonBlocking: true } }),
    )
    .seal();
  assert.deepEqual(
    (routed as unknown as { lockBrokerCfg?: unknown }).lockBrokerCfg,
    { globs: ["/workspace/**"], nonBlocking: true },
  );
  const merged = kelir()
    .storage({ idmap: { guestUid: 0, hostUid: 0 } })
    .fs((f) => f.config({ lockBroker: { globs: [], nonBlocking: false } }))
    .seal();
  const state = merged as unknown as {
    storageCfg?: { idmap?: { guestUid: number; hostUid: number } };
    lockBrokerCfg?: unknown;
  };
  assert.deepEqual(state.storageCfg?.idmap, { guestUid: 0, hostUid: 0 });
  assert.deepEqual(state.lockBrokerCfg, { globs: [], nonBlocking: false });
  const host = kelir();
  assert.throws(
    () =>
      host.storage({
        lockBroker: true,
      } as unknown as Parameters<typeof host.storage>[0]),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message ===
        "storage() does not accept lockBroker: set it via .fs((f) => f.config({ lockBroker }))",
  );
  assert.throws(
    () =>
      kelir().fs((f) =>
        f.config({ lockBroker: "yes" } as unknown as Parameters<typeof f.config>[0]),
      ),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "fs.config(lockBroker=...) expects a record",
  );
  const sealed = kelir()
    .fs((f) => f.config({ lockBroker: { globs: ["/workspace/**"] } }))
    .seal();
  assert.throws(
    () => sealed.fs((f) => f.config({ lockBroker: { globs: [] } })),
    isSealedError(".fs"),
  );
});

test("plugin factories return mountable builders", () => {
  const pairs: readonly [string, KelirBuilder<any, any, any, any>][] = [
    ["chaos", chaos({})],
    ["container", container({})],
    ["db-proxy", dbProxy({})],
    ["ledger", ledger({})],
    ["mcp", mcp({})],
    ["mesh", mesh({})],
    ["profile", profile({})],
    ["replay", replay({})],
    ["supply-chain", supplyChain({})],
    ["tool-guard", toolGuard({})],
    ["transaction", transaction({})],
    ["vault", vault({})],
    ["wire-ingress", wireIngress({})],
  ];
  assert.equal(pairs.length, 13);
  const host = kelir();
  for (const [name, factory] of pairs) {
    assert.equal(factory instanceof KelirBuilder, true);
    assert.equal(factory.name, name);
    host.use(factory);
  }
  assert.equal(host.fuse().hookCount(RingEvent.Trap), 0);
  const disabled = kelir().use(chaos({ enabled: false }));
  assert.equal(disabled.fuse().hookCount(RingEvent.Trap), 0);
});

test("forkState is a factory: a fork of a sealed builder is a fresh unsealed builder", () => {
  const plugin = createPlugin<"forked", Record<string, never>>("forked")
    .trap((scope) => scope.onTrap(() => Verdict.Pass))
    .seal();
  assert.throws(
    () => plugin.trap((scope) => scope.onTrap(() => Verdict.Pass)),
    isSealedError(".trap"),
  );
  const fork = plugin.forkState();
  assert.notEqual(fork, plugin);
  assert.equal(fork.name, "forked");
  fork.trap((scope) => scope.onTrap(() => Verdict.Pass));
  assert.equal(fork.hooksForFusion().rings[0].hookCount, 2);
  assert.equal(fork.fuse().hookCount(RingEvent.Trap), 2);
});

test("forkState on a host-mode builder throws EConfig", () => {
  assert.throws(
    () => kelir().forkState(),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message ===
        "host-mode KelirBuilder forks via Session.fork(), not .forkState()",
  );
});

test("createPlugin threads a Params generic into trap handlers", () => {
  const plugin = createPlugin<"pp", Record<string, never>, unknown, never, { action: string }>("pp")
    .trap("git", (c) => {
      const action: string = c.params.action;
      void action;
      return c.pass();
    });
  assert.equal(plugin.fuse().hookCount(RingEvent.Trap), 1);
});

test("boot stays the single terminal: a plugin cannot boot", async () => {
  await assert.rejects(
    createPlugin<"p", Record<string, never>>("p").boot(),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "cannot boot plugin p: mount it on a host with .use()",
  );
});

async function napiAddonShipped(): Promise<boolean> {
  const specifier = "../dist/native/kelir.node";
  try {
    await import(specifier);
    return true;
  } catch {
    return false;
  }
}

test("boot stays the single terminal: a sealed host still boots", async (t) => {
  if (await napiAddonShipped()) {
    t.skip("the precompiled NAPI addon is shipped; the missing path cannot fire");
    return;
  }
  await assert.rejects(
    kelir().seal().boot(),
    (error: unknown) =>
      error instanceof EConfig && error.message.includes("NAPI addon is missing"),
  );
});
