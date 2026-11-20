// see: cans/02-plugin-model.md#Unified-interposition-pipeline
// see: cans/05-storage-plane.md#Encapsulation-invariant
// see: cans/plugins/redaction.md#Plugin-redaction

import { kelir, createPlugin, FilterAction } from "@kelir/core";

const PROTECTED = ["main", "master"];

const gitGatePlugin = createPlugin("git-gate")
  // see: cans/plugins/redaction.md#Plugin-redaction
  .fs((f) =>
    f.onFsWrite((_path, buffer, _pid, _ctx) => {
      // in-place masking: the bridge copies the mutated buffer back to the guest
      const masked = buffer.toString("utf8")
        .replace(/sk_live_[0-9a-zA-Z]{24}/g, (m) => "*".repeat(m.length));
      buffer.set(Buffer.from(masked, "utf8"), 0);
      return FilterAction.Forward;
    }),
  )
  .trap((t) =>
    t.onTrap(async (event, c) => {
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
            return c.abort("EPOLICY: commit targets protected branch; commit to feature/* instead");
          return c.pass();
        }
        default:
          return c.pass();
      }
    }),
  );

async function main() {
  const session = await kelir({
    vcpus: 2, memoryMb: 1024, driver: "auto",
    kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
    rootfsErofs: "./fixtures/rootfs-git.erofs",
    tscLocked: true, enableWatchdog: true,
  })
    .storage({
      idmap: { guestUid: 0, hostUid: 0 },
      sharedCaches: [],
      encapsulation: "passthrough",
    })
    .use(gitGatePlugin)
    .seal()
    .boot();
  try {
    await session.terminal.resize(120, 40);
    const slot = await session.checkpoint.create("git-gate-base");
    console.error(`[kelir] checkpoint id=${slot.id}`);

    // Test 1: Verify forced push argument stripping and disarm
    const push = await session.spawn("git", ["push", "--force", "origin", "main"]);
    console.error(`[kelir] git push -> exit=${push.exitCode}`);
    if (push.exitCode !== 0) throw new Error("patched git push failed unexpectedly");

    // Test 2: Verify protected branch commit drop
    const commit = await session.spawn("git", ["commit", "-m", "bypassing review on main"]);
    console.error(`[kelir] git commit -> exit=${commit.exitCode}`);
    if (commit.exitCode === 0) throw new Error("commit to protected branch unexpectedly succeeded");

  } finally {
    await session.stop();
  }
}

main().catch(console.error);
