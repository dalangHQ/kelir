// see: cans/03-sdk-architecture.md#TypeScript-SDK

import test from "node:test";
import assert from "node:assert/strict";

import {
  EConfig,
  FilterAction,
  LifecycleEvent,
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
  assert.equal(host.params(t.Object({ ping: t.String() })), host);
  assert.equal(host.detached(false), host);
  assert.equal(host.lockBroker({ globs: ["/workspace/**"] }), host);
  const plugin = createPlugin<"chain", Record<string, never>>("chain");
  assert.equal(plugin.onTrap(() => Verdict.Pass), plugin);
  assert.equal(plugin.onProcessExit(() => {}), plugin);
  assert.equal(plugin.onFsLookup(() => Verdict.Pass), plugin);
  assert.equal(plugin.onNetOutbound(() => FilterAction.Forward), plugin);
  assert.equal(plugin.onVmInit(() => {}), plugin);
  assert.equal(plugin.onTrapDeny(() => {}), plugin);
  assert.equal(plugin.registerCustomAction("act", t.Object({}), () => {}), plugin);
  assert.equal(plugin.registerStreamKind(7, t.Object({}), () => ({})), plugin);
  assert.equal(plugin.trap("git", (c) => c.pass()), plugin);
  assert.equal(plugin.trap((scope) => scope.onTrap(() => Verdict.Pass)), plugin);
  assert.equal(plugin.trap({ pre: () => Verdict.Pass }), plugin);
  assert.equal(plugin.on(LifecycleEvent.VmInit, () => {}), plugin);
  assert.equal(plugin.fs((scope) => scope.onFsLookup(() => Verdict.Pass)), plugin);
  assert.equal(plugin.net((scope) => scope.onNetOutbound(() => FilterAction.Forward)), plugin);
  assert.equal(plugin.console((scope) => scope.onConsoleInput(() => new Uint8Array(0))), plugin);
  assert.equal(plugin.exec((scope) => scope.onExecStdout(() => new Uint8Array(0))), plugin);
  assert.equal(plugin.host((scope) => scope.onHostLsmExec(() => Verdict.Pass)), plugin);
  assert.equal(plugin.lifecycle((scope) => scope.onVmInit(() => {})), plugin);
  assert.equal(plugin.telemetry((scope) => scope.onHeartbeat(() => {})), plugin);
  assert.equal(plugin.requires("ledger"), plugin);
  assert.equal(plugin.derive(() => ({ ready: true })), plugin);
  assert.equal(plugin.context(() => ({ mode: "guest" })), plugin);
  assert.equal(plugin.scope({ binaries: ["git"] }), plugin);
  assert.equal(plugin.forkWith(() => createPlugin<"chain", Record<string, never>>("chain")), plugin);
  assert.equal(
    plugin.vsockService(1024, "ping", t.Object({ ping: t.String() }), (request) => request),
    plugin,
  );
  assert.equal(plugin.use(createPlugin<"inner", Record<string, never>>("inner")), plugin);
  assert.equal(plugin.plugin(createPlugin<"inner2", Record<string, never>>("inner2")), plugin);
  assert.equal(plugin.group((b) => b.onTrap(() => Verdict.Pass)), plugin);
  assert.equal(plugin.seal(), plugin);
});

test("seal blocks every verb, and seal itself is idempotent", () => {
  const builder = kelir().onTrap(() => Verdict.Pass).seal();
  assert.equal(builder.seal(), builder);
  const verbs: readonly [string, () => unknown][] = [
    [".terminal", () => builder.terminal({ cols: 80 })],
    [".storage", () => builder.storage({})],
    [".wire", () => builder.wire({})],
    [".params", () => builder.params(t.Object({ ping: t.String() }))],
    [".detached", () => builder.detached(true)],
    [".lockBroker", () => builder.lockBroker({ globs: ["/workspace/**"] })],
    [".use", () => builder.use(createPlugin<"x", Record<string, never>>("x"))],
    [".plugin", () => builder.plugin(createPlugin<"x", Record<string, never>>("x"))],
    [".trap", () => builder.trap("git", (c) => c.pass())],
    [".on", () => builder.on(LifecycleEvent.VmInit, () => {})],
    [".onTrapDeny", () => builder.onTrapDeny(() => {})],
    [".registerCustomAction", () => builder.registerCustomAction("act", t.Object({}), () => {})],
    [".registerStreamKind", () => builder.registerStreamKind(7, t.Object({}), () => ({}))],
    [".fs", () => builder.fs((scope) => scope.onFsLookup(() => Verdict.Pass))],
    [".net", () => builder.net((scope) => scope.onNetOutbound(() => FilterAction.Forward))],
    [".console", () => builder.console((scope) => scope.onConsoleInput(() => new Uint8Array(0)))],
    [".exec", () => builder.exec((scope) => scope.onExecStdout(() => new Uint8Array(0)))],
    [".host", () => builder.host((scope) => scope.onHostLsmExec(() => Verdict.Pass))],
    [".lifecycle", () => builder.lifecycle((scope) => scope.onVmInit(() => {}))],
    [".telemetry", () => builder.telemetry((scope) => scope.onHeartbeat(() => {}))],
    [".onTrap", () => builder.onTrap(() => Verdict.Pass)],
    [".onFsLookup", () => builder.onFsLookup(() => Verdict.Pass)],
    [".onNetOutbound", () => builder.onNetOutbound(() => FilterAction.Forward)],
    [".onVmInit", () => builder.onVmInit(() => {})],
    [".onHeartbeat", () => builder.onHeartbeat(() => {})],
    [".requires", () => builder.requires("ledger")],
    [".derive", () => builder.derive(() => ({ ready: true }))],
    [".context", () => builder.context(() => ({ mode: "guest" }))],
    [".scope", () => builder.scope({ binaries: ["git"] })],
    [".forkWith", () => builder.forkWith(() => createPlugin<"x", Record<string, never>>("x"))],
    [".vsockService", () => builder.vsockService(1024, "ping", t.Object({ ping: t.String() }), (request) => request)],
    [".group", () => builder.group((b) => b.onTrap(() => Verdict.Pass))],
  ];
  for (const [verb, run] of verbs) {
    assert.throws(run, isSealedError(verb));
  }
  const sealedPlugin = createPlugin<"sealed", Record<string, never>>("sealed").seal();
  assert.throws(() => sealedPlugin.onTrap(() => Verdict.Pass), isSealedError(".onTrap"));
});

test("seal runs validation: bad specs fail at seal", () => {
  assert.throws(
    () =>
      createPlugin<"bad-port", Record<string, never>>("bad-port")
        .vsockService(0, "ping", t.Object({ ping: t.String() }), (request) => request)
        .seal(),
    EConfig,
  );
  const good = createPlugin<"good", Record<string, never>>("good").onTrap(() => Verdict.Pass).seal();
  assert.equal(good.fuse().hookCount(RingEvent.Trap), 1);
});

test("fuse seals: verbs after fuse throw", () => {
  const builder = kelir().onTrap(() => Verdict.Pass);
  assert.equal(builder.fuse().hookCount(RingEvent.Trap), 1);
  assert.throws(() => builder.onTrap(() => Verdict.Pass), isSealedError(".onTrap"));
});

test("hooksForFusion validates without sealing", () => {
  const builder = kelir().onTrap(() => Verdict.Pass);
  const first = builder.hooksForFusion();
  assert.equal(first.rings.length, 1);
  assert.equal(first.rings[0].hookCount, 1);
  builder.onTrap(() => Verdict.Pass);
  assert.equal(builder.hooksForFusion().rings[0].hookCount, 2);
  assert.throws(
    () =>
      createPlugin<"bad-port", Record<string, never>>("bad-port")
        .vsockService(0, "ping", t.Object({ ping: t.String() }), (request) => request)
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
      scope.onTrap(() => Verdict.Pass).end().onTrap(() => Verdict.Pass);
    })
    .fs((scope) => {
      scope.onFsLookup(() => Verdict.Pass).end().onFsLookup(() => Verdict.Pass);
    })
    .net((scope) => {
      scope.onNetOutbound(() => FilterAction.Forward).end().onNetOutbound(() => FilterAction.Forward);
    })
    .console((scope) => {
      scope.onConsoleInput(() => new Uint8Array(0)).end().onConsoleInput(() => new Uint8Array(0));
    })
    .exec((scope) => {
      scope.onExecStdout(() => new Uint8Array(0)).end().onExecStdout(() => new Uint8Array(0));
    })
    .host((scope) => {
      scope.onHostLsmExec(() => Verdict.Pass).end().onHostLsmExec(() => Verdict.Pass);
    })
    .lifecycle((scope) => {
      scope.onVmInit(() => {}).end().onVmInit(() => {});
    })
    .telemetry((scope) => {
      scope.onHeartbeat(() => {}).end().onHeartbeat(() => {});
    });
  const descriptor = plugin.hooksForFusion();
  const total = descriptor.rings.reduce((sum, ring) => sum + ring.hookCount, 0);
  assert.equal(total, 16);
  assert.equal(plugin.fuse().hookCount(RingEvent.Trap), 2);
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
    inner.onTrap(() => Verdict.Pass);
    inner.onFsWrite(() => FilterAction.Forward);
  });
  assert.equal(builder.fuse().hookCount(RingEvent.Trap), 1);
  assert.equal(builder.fuse().hookCount(RingEvent.FsWrite), 1);
  const returned = kelir().group((inner) => inner.onTrap(() => Verdict.Pass));
  assert.equal(returned.fuse().hookCount(RingEvent.Trap), 1);
  assert.throws(
    () => kelir().group(() => kelir()),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "group() must return the builder it receives",
  );
});

test("lockBroker is a host verb returning the builder", () => {
  const builder = kelir();
  assert.equal(builder.lockBroker({ globs: ["/workspace/**"], nonBlocking: true }), builder);
  const sealed = kelir().lockBroker({ globs: ["/workspace/**"] }).seal();
  assert.throws(() => sealed.lockBroker({ globs: [] }), isSealedError(".lockBroker"));
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
    .onTrap(() => Verdict.Pass)
    .seal();
  assert.throws(() => plugin.onTrap(() => Verdict.Pass), isSealedError(".onTrap"));
  const fork = plugin.forkState();
  assert.notEqual(fork, plugin);
  assert.equal(fork.name, "forked");
  fork.onTrap(() => Verdict.Pass);
  assert.equal(fork.hooksForFusion().rings[0].hookCount, 2);
  assert.equal(fork.fuse().hookCount(RingEvent.Trap), 2);
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
