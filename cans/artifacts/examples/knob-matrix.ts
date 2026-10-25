// ============================================================================
// knob-matrix.ts
// ============================================================================
// Canonical depth example: the non-default knob set, not the swe-agent
// defaults. Where autonomous-swe-agent.ts runs hardware-only with Locked
// workspace encapsulation, StreamingAutomata scanning, and workspace-
// scoped receipts, this file deliberately departs from every one of
// those defaults — in one place, so knob-coverage has a single home.
//
// SAME KNOB SET as knob-matrix.rs and knob-matrix.py. Each knob below
// is set to a NON-DEFAULT value; the comment on each line names the
// default it departs from:
//
//   VFS (plugins/vfs.md):
//     encapsulation   Passthrough     (default Locked)
//     scanEngine      FastRegex       (default StreamingAutomata)
//     carryoverBytes  1024            (default 512)
//   PTY:
//     carryoverBufferBytes 1024      (default 512)
//   RECEIPTS (08):
//     receiptScope    WholeVmRam      (default WorkspaceInodesOnly)
//   DRIVER (01, vmm-driver-trait.rs DriverPolicy):
//     wxEnforcement         PermissiveAudit (default Strict)
//     allowEmulatedFallback true           (default false)
//     driver                bwrap            (the emulated tier)
//   VM (09, VmConfig.entropy_bypass_paths):
//     entropyBypassPaths ["/opt/kelir/trusted-runner"]
//       (a single trusted-runner path, not a wildcard)
//
// EMULATED-TIER DISCLAIMER: this example does NOT claim hardware-only
// admission. driver="bwrap" with allowEmulatedFallback=true is the
// emulated tier the swe-agent family never exercises; W^X violations
// here are logged (PermissiveAudit), not fatal (Strict). Do not copy
// these driver knobs into a hardware-admission example.
//
// GAPS (flagged, not silently invented):
//   - driverPolicy / entropyBypassPaths as kelir() options: no TS
//     example shows them. The field names follow the Rust VmConfig /
//     DriverPolicy structs (vmm-driver-trait.rs) in TS camelCase idiom,
//     cf. tapFilter / streamEngine in autonomous-swe-agent.ts.
//   - receiptScope as a receipts field: the field is defined in 08
//     (parent decision); no TS example sets it, so the string form
//     follows the includeDirtyBytes precedent in the same literal.
//   - VFS/PTY knob spellings in TS: plugins/vfs.md attests the field
//     names (encapsulation, scan_engine, carryover_bytes); the TS
//     camelCase forms follow the tapFilter precedent, and
//     carryoverBufferBytes follows the vfs carryover_bytes name in
//     the pty config's idiom.

import { kelir, type Session } from "@kelir/core";
import { pty }    from "@kelir/plugin-pty";
import { vfs }    from "@kelir/plugin-vfs";
import { ledger } from "@kelir/plugin-ledger";

// Trusted runner whose entropy check is bypassed: a single explicit
// path, never a wildcard, never a directory.
const TRUSTED_RUNNER_PATH = "/opt/kelir/trusted-runner";

const runtime = kelir({
  vcpus: 2, memoryMb: 2048,
  kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
  rootfsErofs: "./fixtures/rootfs-knobs.erofs",
  tscLocked: true, enableWatchdog: true,
  // DRIVER — emulated tier (see header disclaimer).
  driver: "bwrap",
  driverPolicy: {
    // PermissiveAudit, NOT the Strict default (01):
    // W^X violations are logged, not fatal.
    wxEnforcement: "PermissiveAudit",
    // true, NOT the false default (01): bwrap/proot fallback is
    // banned by default; this example opts in.
    allowEmulatedFallback: true,
  },
  // One trusted-runner path, not a wildcard (09).
  entropyBypassPaths: [TRUSTED_RUNNER_PATH],

  // → c.fs — Passthrough, NOT the Locked default, so the guest
  // workspace is not locked to mode 000; FastRegex, NOT the
  // StreamingAutomata default; 1024-byte carryover, NOT 512.
}).use(vfs({
  encapsulation: "Passthrough",
  scanEngine: "FastRegex",
  carryoverBytes: 1024,
  lockBroker: false,

  // → c.term — 1024-byte carryover buffer, NOT the 512 default.
})).use(pty({
  cols: 120, rows: 40,
  dropOsc52: true, stripTitle: true,
  carryoverBufferBytes: 1024,

  // → c.ledger — WholeVmRam receipts, NOT the WorkspaceInodesOnly
  // default: the receipt covers whole-VM RAM mutations.
})).use(ledger({
  store: "sqlite://.kelir/audit.merkle.db",
  merkleChain: true,
  receipts: { includeSyscallCount: true, receiptScope: "WholeVmRam" },
}));

async function main() {
  const session: Session = await runtime.boot();
  console.log(`[kelir] driver=${session.driverName} boot=${session.bootMs}ms`);
  const probe = await session.spawn("true", [], { cwd: "/workspace" });
  console.log(`[kelir] probe -> exit=${probe.exitCode}`);
  console.log("[kelir] knobs: vfs=Passthrough/FastRegex/1024 " +
              "pty_carryover=1024 receiptScope=WholeVmRam " +
              "driver=bwrap wx=PermissiveAudit emulatedFallback=true " +
              `entropyBypass=[${TRUSTED_RUNNER_PATH}]`);
  await session.terminate();
}

main().catch(console.error);
