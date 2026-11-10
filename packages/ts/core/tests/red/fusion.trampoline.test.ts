// see: cans/artifacts/test-plan/cases/bindings_ts/bt_03_typescript_fused_trampoline_single_crossing_chained_hooks.yaml#case_id
import test from "node:test";

import { FusedTrampoline, RingEvent, kelir } from "../../dist/index.js";

// see: cans/artifacts/test-plan/cases/bindings_ts/bt_03_typescript_fused_trampoline_single_crossing_chained_hooks.yaml#case_id
async function bt_03_typescript_fused_trampoline_single_crossing_chained_hooks(): Promise<void> {
  const builder = kelir();
  for (let index = 0; index < 5; index += 1) {
    builder.trap("fs.write:*", async () => null);
  }
  const fused: FusedTrampoline = builder.fuse();
  if (fused.crossingsPerRingEvent(RingEvent.Trap) !== 1) {
    throw new Error(
      "bt_03: five fused Trap hooks do not collapse to exactly 1 crossing per ring event",
    );
  }
  if (fused.hookCount(RingEvent.Trap) !== 5) {
    throw new Error(
      `bt_03: the fused Trap ring carries ${fused.hookCount(RingEvent.Trap)} hooks, expected 5`,
    );
  }
  try {
    await builder.boot();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(
      `bt_03: the fused trampoline table never bound at boot (one entry per ring event): ${message}`,
    );
  }
}

test(
  "bt_03_typescript_fused_trampoline_single_crossing_chained_hooks",
  bt_03_typescript_fused_trampoline_single_crossing_chained_hooks,
);
