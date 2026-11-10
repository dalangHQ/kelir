// see: cans/artifacts/test-plan/cases/bindings_ts/bt_01_typescript_boot_econfig_missing_typebox_dependency.yaml#case_id
import test from "node:test";

import { EConfig, kelir } from "../../dist/index.js";

// see: cans/artifacts/test-plan/cases/bindings_ts/bt_01_typescript_boot_econfig_missing_typebox_dependency.yaml#case_id
async function bt_01_typescript_boot_econfig_missing_typebox_dependency(): Promise<void> {
  try {
    await kelir().boot();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (error instanceof EConfig && message.toLowerCase().includes("typebox")) {
      return;
    }
    throw new Error(
      `bt_01: builder.boot() must throw EConfig naming the missing TypeBox schema validator; got: ${message}`,
    );
  }
  throw new Error(
    "bt_01: builder.boot() completed without the EConfig prerequisite gate",
  );
}

test(
  "bt_01_typescript_boot_econfig_missing_typebox_dependency",
  bt_01_typescript_boot_econfig_missing_typebox_dependency,
);
