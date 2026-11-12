// see: cans/03-sdk-architecture.md#TypeScript-SDK

import test from "node:test";
import assert from "node:assert/strict";

import {
  EConfig,
  FilterAction,
  LifecycleEvent,
  RingEvent,
  Verdict,
  kelir,
  createPlugin,
  t,
  KelirBuilder,
  type KelirPlugin,
  type HookDescriptor,
} from "../dist/index.js";

// see: cans/03-sdk-architecture.md#TypeScript-SDK
const p: KelirPlugin = createPlugin<"x", Record<string, never>>("x");
void p;

// see: cans/03-sdk-architecture.md#TypeScript-SDK
const chained = createPlugin<"chained", Record<string, never>>("chained")
  .context(() => ({ ready: true }))
  .requires<"ledger">()
  .scope({ binaries: ["git"] })
  .onTrap(() => Verdict.Pass);
void chained;

// TODO (L2): re-express the unmet-requires compile-error assertion (kelir().use(createPlugin("x").requires<"missing">())) without @ts-expect-error, which the comment law bans.

type DescriptorShape = {
  readonly ring: string;
  readonly lifecycleTag: string | undefined;
  readonly ordinal: number;
};

type MergedDescriptorShape = DescriptorShape & {
  readonly pluginName: string | undefined;
};

function descriptors(
  builder: KelirBuilder<any, any, any, any>,
): DescriptorShape[] {
  return mergedDescriptors(builder).map((hook) => ({
    ring: hook.ring,
    lifecycleTag: hook.lifecycleTag,
    ordinal: hook.ordinal,
  }));
}

function mergedDescriptors(
  builder: KelirBuilder<any, any, any, any>,
): MergedDescriptorShape[] {
  return builder
    .fuse()
    .describe()
    .rings.flatMap((ring) =>
      ring.hooks.map((hook) => ({
        ring: ring.ring as string,
        lifecycleTag: hook.lifecycleTag,
        ordinal: hook.ordinal,
        pluginName: hook.pluginName,
      })),
    )
    .sort((a, b) => a.ordinal - b.ordinal);
}

function allVerbsFlat(): ReturnType<typeof createPlugin> {
  return createPlugin<"all-flat", Record<string, never>>("all-flat")
    .onTrap(() => Verdict.Pass)
    .onProcessExit(() => {})
    .onFsLookup(() => Verdict.Pass)
    .onFsRead(() => new Uint8Array(0))
    .onFsWrite(() => FilterAction.Forward)
    .onFsLock(() => Verdict.Pass)
    .onNetOutbound(() => FilterAction.Forward)
    .onNetInbound(() => FilterAction.Forward)
    .onConsoleInput(() => new Uint8Array(0))
    .onConsoleOutput(() => new Uint8Array(0))
    .onExecStdout(() => new Uint8Array(0))
    .onExecStderr(() => new Uint8Array(0))
    .onHostLsmExec(() => Verdict.Pass)
    .onHostPressure(() => Verdict.Pass)
    .onHostKeyringAccess(() => Verdict.Pass)
    .onHostPreempt(() => {})
    .onVmInit(() => {})
    .onVmReady(() => {})
    .onVmShutdown(() => {})
    .onPreCommit(() => {})
    .onPostCommit(() => {})
    .onMemoryPressure(() => {})
    .onHeartbeat(() => {})
    .onWatchdog(() => "execute-default");
}

function allVerbsNested(): ReturnType<typeof createPlugin> {
  return createPlugin<"all-nested", Record<string, never>>("all-nested")
    .trap((scope) =>
      scope.onTrap(() => Verdict.Pass).onProcessExit(() => {}),
    )
    .fs((scope) =>
      scope
        .onFsLookup(() => Verdict.Pass)
        .onFsRead(() => new Uint8Array(0))
        .onFsWrite(() => FilterAction.Forward)
        .onFsLock(() => Verdict.Pass),
    )
    .net((scope) =>
      scope
        .onNetOutbound(() => FilterAction.Forward)
        .onNetInbound(() => FilterAction.Forward),
    )
    .console((scope) =>
      scope
        .onConsoleInput(() => new Uint8Array(0))
        .onConsoleOutput(() => new Uint8Array(0)),
    )
    .exec((scope) =>
      scope
        .onExecStdout(() => new Uint8Array(0))
        .onExecStderr(() => new Uint8Array(0)),
    )
    .host((scope) =>
      scope
        .onHostLsmExec(() => Verdict.Pass)
        .onHostPressure(() => Verdict.Pass)
        .onHostKeyringAccess(() => Verdict.Pass)
        .onHostPreempt(() => {}),
    )
    .lifecycle((scope) =>
      scope
        .onVmInit(() => {})
        .onVmReady(() => {})
        .onVmShutdown(() => {})
        .onPreCommit(() => {})
        .onPostCommit(() => {}),
    )
    .telemetry((scope) =>
      scope
        .onMemoryPressure(() => {})
        .onHeartbeat(() => {})
        .onWatchdog(() => "execute-default"),
    );
}

function chaosPlugin(): ReturnType<typeof createPlugin> {
  return createPlugin<"chaos", Record<string, never>>("chaos")
    .requires("ledger")
    .scope({ paths: ["/workspace"] })
    .fs((scope) =>
      scope.onFsWrite(() => FilterAction.Forward),
    )
    .net((scope) =>
      scope.onNetOutbound(() => FilterAction.Forward),
    );
}

test("createPlugin returns a KelirBuilder with no build step", () => {
  const plugin = createPlugin<"x", Record<string, never>>("x");
  assert.equal(plugin instanceof KelirBuilder, true);
  assert.equal(
    (plugin as unknown as Record<string, unknown>).build,
    undefined,
  );
  assert.equal(plugin.name, "x");
});

test("plugin mounts directly and merges hooks into the host", () => {
  const plugin = createPlugin<"wired", Record<string, never>>("wired").onTrap(() => Verdict.Pass);
  const builder = kelir().use(plugin);
  assert.equal(builder.fuse().hookCount(RingEvent.Trap), 1);
});

test("mounting the same plugin twice throws EConfig", () => {
  const plugin = createPlugin<"wired", Record<string, never>>("wired").onTrap(() => Verdict.Pass);
  const builder = kelir().use(plugin);
  assert.throws(
    () => builder.use(plugin),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "plugin already mounted: wired",
  );
});

test("verbs on a mounted plugin throw EConfig", () => {
  const plugin = createPlugin<"wired", Record<string, never>>("wired").onTrap(() => Verdict.Pass);
  kelir().use(plugin);
  assert.throws(() => plugin.onTrap(() => Verdict.Pass), EConfig);
  assert.throws(
    () => plugin.trap((scope) => scope.onTrap(() => Verdict.Pass)),
    EConfig,
  );
  assert.throws(() => plugin.requires("ledger"), EConfig);
  assert.throws(() => plugin.scope({ binaries: ["git"] }), EConfig);
  assert.throws(() => plugin.on(LifecycleEvent.VmInit, () => {}), EConfig);
  assert.throws(() => plugin.onTrapDeny(() => {}), EConfig);
  assert.throws(
    () => plugin.registerCustomAction("act", t.Object({}), () => {}),
    EConfig,
  );
  assert.throws(
    () => plugin.registerStreamKind(7, t.Object({}), () => ({})),
    EConfig,
  );
});

test("scope survives mount", () => {
  const plugin = createPlugin<"scoped", Record<string, never>>("scoped").scope({ binaries: ["git"] });
  kelir().use(plugin);
  assert.deepEqual(plugin.getScope(), { binaries: ["git"] });
});

test("forkWith re-runs the fork closure", () => {
  const plugin = createPlugin<"forked", Record<string, never>>("forked").forkWith(() =>
    createPlugin<"forked", Record<string, never>>("forked").onTrap(() => Verdict.Pass),
  );
  assert.equal(plugin.fuse().hookCount(RingEvent.Trap), 0);
  const forked = plugin.forkState();
  assert.equal(forked.name, "forked");
  assert.equal(forked.fuse().hookCount(RingEvent.Trap), 1);
  assert.notEqual(forked, plugin);
});

test("forkState without forkWith clones the builder", () => {
  const plugin = createPlugin<"cloned", Record<string, never>>("cloned").onTrap(() => Verdict.Pass);
  const forked = plugin.forkState();
  assert.equal(forked.name, "cloned");
  assert.equal(forked.fuse().hookCount(RingEvent.Trap), 1);
  assert.notEqual(forked, plugin);
});

test("all 24 ring verbs present", () => {
  const hooks = descriptors(allVerbsFlat());
  assert.equal(hooks.length, 24);
  assert.deepEqual(
    hooks.map((hook) => hook.ring),
    [
      RingEvent.Trap,
      RingEvent.ProcessExit,
      RingEvent.FsLookup,
      RingEvent.FsRead,
      RingEvent.FsWrite,
      "fs-lock",
      RingEvent.NetOutbound,
      RingEvent.NetInbound,
      RingEvent.ConsoleInput,
      RingEvent.ConsoleOutput,
      RingEvent.ExecStdout,
      RingEvent.ExecStderr,
      "host-lsm-exec",
      "host-pressure",
      "host-keyring-access",
      "host-preempt",
      RingEvent.Lifecycle,
      RingEvent.Lifecycle,
      RingEvent.Lifecycle,
      RingEvent.Lifecycle,
      RingEvent.Lifecycle,
      "memory-pressure",
      RingEvent.Heartbeat,
      RingEvent.Watchdog,
    ],
  );
  assert.deepEqual(
    hooks
      .filter((hook) => hook.ring === RingEvent.Lifecycle)
      .map((hook) => hook.lifecycleTag),
    ["vm-init", "vm-ready", "vm-shutdown", "pre-commit", "post-commit"],
  );
});

test("nested scopes equal flat descriptors", () => {
  assert.deepEqual(descriptors(allVerbsNested()), descriptors(allVerbsFlat()));
});

test("fuse rejects an invalid vsock spec", () => {
  assert.throws(
    () =>
      createPlugin<"bad-port", Record<string, never>>("bad-port")
        .vsockService(
          0,
          "ping",
          t.Object({ ping: t.String() }),
          (request) => request,
        )
        .fuse(),
    EConfig,
  );
  assert.throws(
    () =>
      createPlugin<"bad-port", Record<string, never>>("bad-port")
        .vsockService(
          65536,
          "ping",
          t.Object({ ping: t.String() }),
          (request) => request,
        )
        .fuse(),
    EConfig,
  );
  assert.throws(
    () =>
      createPlugin<"bad-service", Record<string, never>>("bad-service")
        .vsockService(
          1024,
          "",
          t.Object({ ping: t.String() }),
          (request) => request,
        )
        .fuse(),
    EConfig,
  );
});

test("fuse rejects a malformed scope", () => {
  assert.throws(
    () =>
      createPlugin<"bad-scope", Record<string, never>>("bad-scope")
        .scope({ binaries: "git" } as unknown as { binaries: string[] })
        .fuse(),
    EConfig,
  );
  assert.throws(
    () =>
      createPlugin<"bad-scope", Record<string, never>>("bad-scope")
        .scope({ unknown: [] } as unknown as { binaries: string[] })
        .fuse(),
    EConfig,
  );
});

test("use rejects a host-mode builder as a plugin", () => {
  assert.throws(
    () => kelir().use(kelir() as unknown as KelirPlugin),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "cannot mount a host-mode builder as a plugin",
  );
});

test("use reports a missing prerequisite", () => {
  const chaos = chaosPlugin();
  assert.throws(
    () => kelir().use(chaos),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "missing prerequisite: ledger required by chaos",
  );
  const ledger = createPlugin<"ledger", Record<string, never>>("ledger");
  const builder = kelir().use(ledger).use(chaos);
  assert.equal(builder.fuse().hookCount(RingEvent.FsWrite), 1);
  assert.equal(builder.fuse().hookCount(RingEvent.NetOutbound), 1);
});

test("canonical chaos plugin", () => {
  const plugin = chaosPlugin();
  assert.equal(plugin.name, "chaos");
  assert.deepEqual(plugin.getScope(), { paths: ["/workspace"] });
  assert.deepEqual(plugin.requiredDeps, ["ledger"]);
  assert.deepEqual(
    descriptors(plugin).map((hook) => hook.ring),
    [RingEvent.FsWrite, RingEvent.NetOutbound],
  );
});

test("derive and context fns are stored on the builder", () => {
  const plugin = createPlugin<"stored", Record<string, never>>("stored")
    .derive(() => ({ ready: true }))
    .context(() => ({ mode: "guest" }));
  assert.equal(plugin.deriveFns.length, 1);
  assert.equal(plugin.contextFns.length, 1);
  assert.deepEqual(plugin.deriveFns[0]({}), { ready: true });
  assert.deepEqual(plugin.contextFns[0]({}), { mode: "guest" });
});

test("plugin-level vsockService forwards through use", () => {
  const plugin = createPlugin<"pinged", Record<string, never>>("pinged").vsockService(
    1024,
    "ping",
    t.Object({ ping: t.String() }),
    (request) => ({ pong: request.ping }),
  );
  assert.equal(plugin.vsockServices.length, 1);
  assert.equal(plugin.vsockServices[0].port, 1024);
  assert.equal(plugin.vsockServices[0].service, "ping");
  const builder = kelir().use(plugin);
  assert.equal(builder.vsockServices.length, 1);
  assert.equal(builder.vsockServices[0].port, 1024);
});

test("host-only verbs on a plugin-mode builder throw EConfig at the call site", () => {
  assert.throws(
    () => createPlugin<"term", Record<string, never>>("term").terminal({ cols: 80 }),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "plugin term called host-only verb .terminal",
  );
  assert.throws(
    () => createPlugin<"det", Record<string, never>>("det").detached(true),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "plugin det called host-only verb .detached",
  );
  assert.throws(
    () => createPlugin<"stor", Record<string, never>>("stor").storage({}),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "plugin stor called host-only verb .storage",
  );
  assert.throws(
    () => createPlugin<"wir", Record<string, never>>("wir").wire({}),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "plugin wir called host-only verb .wire",
  );
  assert.throws(
    () =>
      createPlugin<"par", Record<string, never>>("par").params(
        t.Object({ ping: t.String() }),
      ),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "plugin par called host-only verb .params",
  );
  assert.throws(
    () =>
      createPlugin<"lock", Record<string, never>>("lock").lockBroker({
        globs: ["/workspace/**"],
      }),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "plugin lock called host-only verb .lockBroker",
  );
});

test("host gains the 24 verbs", () => {
  const builder = kelir()
    .onTrap(() => Verdict.Pass)
    .onFsWrite(() => FilterAction.Forward)
    .onNetOutbound(() => FilterAction.Forward)
    .onVmInit(() => {});
  assert.equal(builder.fuse().hookCount(RingEvent.Trap), 1);
  assert.equal(builder.fuse().hookCount(RingEvent.FsWrite), 1);
  assert.equal(builder.fuse().hookCount(RingEvent.NetOutbound), 1);
  assert.equal(builder.fuse().hookCount(RingEvent.Lifecycle), 1);
});

test("mount merges descriptors: host hooks first, then plugin in mount order", () => {
  const builder = kelir().onTrap(() => Verdict.Pass);
  const chaos = chaosPlugin();
  builder.use(createPlugin<"ledger", Record<string, never>>("ledger"));
  builder.use(chaos);
  assert.deepEqual(mergedDescriptors(builder), [
    {
      ring: RingEvent.Trap,
      lifecycleTag: undefined,
      ordinal: 0,
      pluginName: undefined,
    },
    {
      ring: RingEvent.FsWrite,
      lifecycleTag: undefined,
      ordinal: 1,
      pluginName: "chaos",
    },
    {
      ring: RingEvent.NetOutbound,
      lifecycleTag: undefined,
      ordinal: 2,
      pluginName: "chaos",
    },
  ]);
});

test("fuse rejects a mounted plugin with a bad vsock spec", () => {
  const plugin = createPlugin<"bad-port", Record<string, never>>("bad-port").vsockService(
    0,
    "ping",
    t.Object({ ping: t.String() }),
    (request) => request,
  );
  const builder = kelir().use(plugin);
  assert.throws(
    () => builder.fuse(),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message.includes("bad-port"),
  );
});

test("plain KelirPlugin objects still mount via use", () => {
  const plain: KelirPlugin = { name: "plain", enabled: true, config: {} };
  const builder = kelir().use(plain);
  assert.equal(builder.fuse().hookCount(RingEvent.Trap), 0);
  const trait = {
    name: "plain-hooks",
    registerInterceptors(bind: (hook: HookDescriptor) => void): void {
      bind({
        ring: RingEvent.Trap,
        stage: "single",
        pluginName: "plain-hooks",
        ordinal: 0,
        run: () => null,
      });
    },
  };
  const wired = kelir().use(trait as unknown as KelirPlugin);
  assert.equal(wired.fuse().hookCount(RingEvent.Trap), 1);
});
