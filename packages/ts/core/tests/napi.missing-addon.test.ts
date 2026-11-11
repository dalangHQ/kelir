// see: cans/03-sdk-architecture.md#Cross-SDK-guarantees

import test from "node:test";
import assert from "node:assert/strict";

// see: cans/03-sdk-architecture.md#TypeScript-SDK
import { EConfig } from "../dist/errors.sdk.js";
import { kelir } from "../dist/builder.lifecycle.js";

async function napiAddonShipped(): Promise<boolean> {
  const specifier = "../dist/native/kelir.node";
  try {
    await import(specifier);
    return true;
  } catch {
    return false;
  }
}

test("boot fails closed with EConfig when the NAPI addon is missing", async (t) => {
  if (await napiAddonShipped()) {
    t.skip("the precompiled NAPI addon is shipped; the missing path cannot fire");
    return;
  }
  await assert.rejects(
    kelir().boot(),
    (error: unknown): boolean => {
      assert.ok(
        error instanceof EConfig,
        `boot() must throw EConfig; got: ${String(error)}`,
      );
      assert.match(error.message, /NAPI addon is missing/);
      return true;
    },
  );
});
