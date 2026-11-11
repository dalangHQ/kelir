// see: cans/03-sdk-architecture.md#TypeScript-SDK

import test from "node:test";
import assert from "node:assert/strict";

import {
  EConfig,
  FilterAction,
  RingEvent,
  Verdict,
  kelir,
  createPlugin,
  t,
  type KelirPlugin,
} from "../dist/index.js";

// see: cans/03-sdk-architecture.md#TypeScript-SDK
const p: KelirPlugin = createPlugin("x").build();
void p;

// see: cans/03-sdk-architecture.md#TypeScript-SDK
const chained = createPlugin("chained")
  .context(() => ({ ready: true }))
  .requires<"ledger">()
  .scope({ binaries: ["git"] })
  .onTrap(() => null)
  .build();
void chained;

// see: cans/03-sdk-architecture.md#TypeScript-SDK
// @ts-expect-error
kelir().use(createPlugin("x").requires<"missing">().build());

type DescriptorShape = {
  readonly ring: string;
  readonly lifecycleTag: string | undefined;
  readonly ordinal: number;
};

function descriptors(
  plugin: ReturnType<typeof createPlugin>,
): DescriptorShape[] {
  return plugin
    .build()
    .hooksForFusion()
    .map((hook) => ({
      ring: hook.ring as string,
      lifecycleTag: hook.lifecycleTag,
      ordinal: hook.ordinal,
    }));
}

function allVerbsFlat(): ReturnType<typeof createPlugin> {
  return createPlugin("all-flat")
    .onTrap(() => null)
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
  return createPlugin("all-nested")
    .trap((scope) =>
      scope.onTrap(() => null).onProcessExit(() => {}),
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
  return createPlugin("chaos")
    .requires("ledger")
    .scope({ paths: ["/workspace"] })
    .fs((scope) =>
      scope.onFsWrite(() => FilterAction.Forward),
    )
    .net((scope) =>
      scope.onNetOutbound(() => FilterAction.Forward),
    );
}

test("build returns a frozen plugin", () => {
  const plugin = createPlugin("frozen").onTrap(() => null).build();
  assert.equal(Object.isFrozen(plugin), true);
  assert.equal(Object.isFrozen(plugin.hooksForFusion()), true);
  assert.equal(plugin.name, "frozen");
  assert.equal(plugin.enabled, true);
  assert.equal(plugin.hooksForFusion().length, 1);
});

test("build snapshots hooks and post-build verbs throw EConfig", () => {
  const contract = createPlugin("snap").onTrap(() => null);
  const built = contract.build();
  assert.throws(() => contract.onTrap(() => null), EConfig);
  assert.equal(built.hooksForFusion().length, 1);
});

test("built plugin registers interceptors into a builder", () => {
  const plugin = createPlugin("wired").onTrap(() => null).build();
  const builder = kelir().use(plugin);
  assert.equal(builder.fuse().hookCount(RingEvent.Trap), 1);
});

test("scope survives build", () => {
  const plugin = createPlugin("scoped").scope({ binaries: ["git"] }).build();
  assert.deepEqual(plugin.scope, { binaries: ["git"] });
});

test("forkWith re-runs the fork closure", () => {
  const plugin = createPlugin("forked")
    .forkWith(() => createPlugin("forked").onTrap(() => null))
    .build();
  assert.equal(plugin.hooksForFusion().length, 0);
  const forked = plugin.forkState();
  assert.equal(forked.name, "forked");
  assert.equal(forked.hooksForFusion().length, 1);
  assert.equal(Object.isFrozen(forked), true);
});

test("forkState without forkWith clones the wrapper", () => {
  const plugin = createPlugin("cloned").onTrap(() => null).build();
  const forked = plugin.forkState();
  assert.equal(forked.name, "cloned");
  assert.equal(forked.hooksForFusion().length, 1);
  assert.notEqual(forked, plugin);
});

test("any verb after build throws EConfig", () => {
  const contract = createPlugin("x").onTrap(() => null);
  contract.build();
  assert.throws(() => contract.onTrap(() => null), EConfig);
  assert.throws(
    () => contract.trap((scope) => scope.onTrap(() => null)),
    EConfig,
  );
  assert.throws(() => contract.requires("ledger"), EConfig);
  assert.throws(() => contract.scope({ binaries: ["git"] }), EConfig);
  assert.throws(() => contract.build(), EConfig);
});

test("all 24 ring verbs present", () => {
  const built = allVerbsFlat().build();
  const hooks = built.hooksForFusion();
  assert.equal(hooks.length, 24);
  assert.deepEqual(
    hooks.map((hook) => hook.ring as string),
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

test("build rejects an invalid vsock spec", () => {
  assert.throws(
    () =>
      createPlugin("bad-port")
        .vsockService(
          0,
          "ping",
          t.Object({ ping: t.String() }),
          (request) => request,
        )
        .build(),
    EConfig,
  );
  assert.throws(
    () =>
      createPlugin("bad-port")
        .vsockService(
          65536,
          "ping",
          t.Object({ ping: t.String() }),
          (request) => request,
        )
        .build(),
    EConfig,
  );
  assert.throws(
    () =>
      createPlugin("bad-service")
        .vsockService(
          1024,
          "",
          t.Object({ ping: t.String() }),
          (request) => request,
        )
        .build(),
    EConfig,
  );
});

test("build rejects a malformed scope", () => {
  assert.throws(
    () =>
      createPlugin("bad-scope")
        .scope({ binaries: "git" } as unknown as { binaries: string[] })
        .build(),
    EConfig,
  );
  assert.throws(
    () =>
      createPlugin("bad-scope")
        .scope({ unknown: [] } as unknown as { binaries: string[] })
        .build(),
    EConfig,
  );
});

test("use rejects an unbuilt contract", () => {
  const contract = createPlugin("unbuilt").onTrap(() => null);
  assert.throws(
    () => kelir().use(contract as unknown as KelirPlugin),
    EConfig,
  );
});

test("use reports a missing prerequisite", () => {
  const chaos = chaosPlugin().build();
  assert.throws(
    () => kelir().use(chaos),
    (error: unknown) =>
      error instanceof EConfig &&
      error.message === "missing prerequisite: ledger required by chaos",
  );
  const ledger = createPlugin("ledger").build();
  const builder = kelir().use(ledger).use(chaos);
  assert.equal(builder.fuse().hookCount(RingEvent.FsWrite), 1);
  assert.equal(builder.fuse().hookCount(RingEvent.NetOutbound), 1);
});

test("canonical chaos plugin", () => {
  const plugin = chaosPlugin().build();
  assert.equal(plugin.name, "chaos");
  assert.deepEqual(plugin.scope, { paths: ["/workspace"] });
  assert.deepEqual(plugin.requiredDeps, ["ledger"]);
  assert.deepEqual(
    plugin.hooksForFusion().map((hook) => hook.ring),
    [RingEvent.FsWrite, RingEvent.NetOutbound],
  );
});

test("derive and context fns are stored on the built plugin", () => {
  const plugin = createPlugin("stored")
    .derive(() => ({ ready: true }))
    .context(() => ({ mode: "guest" }))
    .build();
  assert.equal(plugin.deriveFns.length, 1);
  assert.equal(plugin.contextFns.length, 1);
  assert.deepEqual(plugin.deriveFns[0]({}), { ready: true });
  assert.deepEqual(plugin.contextFns[0]({}), { mode: "guest" });
});

test("plugin-level vsockService forwards through use", () => {
  const plugin = createPlugin("pinged")
    .vsockService(1024, "ping", t.Object({ ping: t.String() }), (request) => ({
      pong: request.ping,
    }))
    .build();
  assert.equal(plugin.vsockServices.length, 1);
  assert.equal(plugin.vsockServices[0].port, 1024);
  assert.equal(plugin.vsockServices[0].service, "ping");
  const builder = kelir().use(plugin);
  assert.equal(builder.vsockServices.length, 1);
  assert.equal(builder.vsockServices[0].port, 1024);
});
