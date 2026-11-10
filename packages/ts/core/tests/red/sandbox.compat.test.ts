// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_12_e2b_compat_session_parity.yaml#case_id
import test from "node:test";

import { Sandbox } from "../../../e2b-compat/dist/e2b-compat/src/index.js";

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_12_e2b_compat_session_parity.yaml#case_id
async function ad_12_e2b_compat_session_parity(): Promise<void> {
  let sandbox: InstanceType<typeof Sandbox>;
  try {
    sandbox = await Sandbox.create();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(
      `ad_12: Sandbox.create() must boot the underlying Session; the e2b-compat create path halted: ${message}`,
    );
  }
  try {
    await sandbox.close();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(
      `ad_12: Sandbox.close() must stop the Session to completion; close halted: ${message}`,
    );
  }
}

test("ad_12_e2b_compat_session_parity", ad_12_e2b_compat_session_parity);
