// see: cans/02-plugin-model.md#Unified-interposition-pipeline
// see: cans/05-storage-plane.md#Encapsulation-invariant
// see: cans/05-storage-plane.md#In-flight-read-masking

import { kelir, createPlugin, FilterAction } from "@kelir/core";

const PROTECTED = ["main", "master"];

const gitGatePlugin = createPlugin("git-gate")
  // see: cans/05-storage-plane.md#In-flight-read-masking
  .onFsWrite((_path, buffer, _pid, _ctx) => {
    // in-place masking: the bridge copies the mutated buffer back to the guest
    const masked = buffer.toString("utf8")
      .replace(/sk_live_[0-9a-zA-Z]{24}/g, (m) => "*".repeat(m.length));
    buffer.set(Buffer.from(masked, "utf8"), 0);
    return FilterAction.Forward;
  })
  .onTrap(async (event, c) => {
    const binary = event.binaryPath.split("/").pop() ?? "";
    if (binary !== "git") return c.pass();
    switch (event.argv[1]) {
      case "push": {
        if (event.argv.includes("--force")) {
          return c.patch({
            argv: event.argv.filter((a) => a !== "--force"),
            env: { KELIR_DISARMED: "1" },
          });
        }
        return c.pass();
      }
      case "commit": {
        const mIndex = event.argv.indexOf("-m");
        const msg = mIndex >= 0 ? (event.argv[mIndex + 1] ?? "") : "";
        if (PROTECTED.some((p) => msg.includes(p)))
          return c.abort("EPOLICY: commit references protected branch");
        return c.pass();
      }
      default:
        return c.pass();
    }
  });

const runtime = kelir({
  vcpus: 2, memoryMb: 1024, driver: "auto",
  kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
  rootfsErofs: "./fixtures/rootfs-git.erofs",
  tscLocked: true, enableWatchdog: true,
})
  .storage({
    idmap: { guestUid: 0, hostUid: 0 },
    sharedCaches: [],
    lockBroker: false,
    syntheticFiles: [],
    maskRead: [],
    encapsulation: "passthrough",
    scanEngine: "FastRegex",
    carryoverBytes: 1024,
  })
  .use(gitGatePlugin);

async function main() {
  runtime.seal();
  const session = await runtime.boot();
  try {
    await session.terminal.resize(120, 40);
    const slot = await session.checkpoint.create("git-gate-base");
    console.error(`[kelir] checkpoint id=${slot.id}`);
    // TODO (L3): spawn git push --force origin main in the guest and gate it via the git-gate plugin.
  } finally {
    await session.stop();
  }
}

main().catch(console.error);
