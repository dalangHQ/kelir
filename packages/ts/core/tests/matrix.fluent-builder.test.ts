// see: cans/03-sdk-architecture.md#TypeScript-SDK

import test from "node:test";
import assert from "node:assert/strict";

import {
  kelir,
  createPlugin,
  RingEvent,
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

test("build returns a frozen plugin", () => {
  const plugin = createPlugin("frozen").onTrap(() => null).build();
  assert.equal(Object.isFrozen(plugin), true);
  assert.equal(Object.isFrozen(plugin.hooksForFusion()), true);
  assert.equal(plugin.name, "frozen");
  assert.equal(plugin.enabled, true);
  assert.equal(plugin.hooksForFusion().length, 1);
});

test("build snapshots hooks", () => {
  const contract = createPlugin("snap").onTrap(() => null);
  const built = contract.build();
  contract.onTrap(() => null);
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

test("onTrap after build throws TypeError", () => {
  const built = createPlugin("x").build();
  assert.throws(
    () =>
      (built as unknown as { onTrap(handler: unknown): unknown }).onTrap(
        () => null,
      ),
    TypeError,
  );
});
