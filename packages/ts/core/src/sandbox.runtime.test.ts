// see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
import { Sandbox } from "./sandbox.runtime.js";
import { EConfig } from "./errors.sdk.js";

function check(condition: boolean, label: string): void {
  if (!condition) {
    throw new Error(`sandbox.runtime.test: ${label}`);
  }
}

async function checkRejects(
  promise: Promise<unknown>,
  messagePart: string,
  label: string,
): Promise<void> {
  try {
    await promise;
  } catch (error) {
    check(
      error instanceof EConfig && error.message.includes(messagePart),
      `${label} (EConfig message)`,
    );
    return;
  }
  throw new Error(
    `sandbox.runtime.test: ${label} (expected EConfig rejection)`,
  );
}

// see: cans/08-lifecycle.md#Suspend-and-resume
async function sandboxFailsClosedBeforeBoot(): Promise<void> {
  const box = await Sandbox.create();
  await checkRejects(
    box.resume("/tmp/snap"),
    "Sandbox.resume before boot",
    "resume rejects before boot",
  );
  await checkRejects(
    box.terminate(),
    "Sandbox.terminate before boot",
    "terminate rejects before boot",
  );
}

// see: cans/08-lifecycle.md#Pre-teardown-flush
async function sandboxTerminateRoutesToTheAbruptSessionVerb(): Promise<void> {
  const box = await Sandbox.create();
  const calls: string[] = [];
  const booted = {
    stop: async (): Promise<void> => {
      calls.push("stop");
    },
    terminate: async (): Promise<void> => {
      calls.push("terminate");
    },
  };
  (box as unknown as { booted: unknown }).booted = booted;
  await box.terminate();
  check(
    calls.length === 1 && calls[0] === "terminate",
    "terminate delegates to the abrupt session verb, never to stop",
  );
}

async function main(): Promise<void> {
  await sandboxFailsClosedBeforeBoot();
  await sandboxTerminateRoutesToTheAbruptSessionVerb();
}

main().catch((error: unknown): never => {
  throw error;
});
