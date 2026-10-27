// ============================================================================
// knob-matrix.ts
// ============================================================================
// Canonical depth example: the non-default knob set, not the swe-agent
// defaults. Where autonomous-swe-agent.ts runs hardware-only with Locked
// workspace encapsulation, StreamingAutomata scanning, and workspace-
// scoped receipts, this file deliberately departs from
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
//   DRIVER (01, vmm-driver-trait.rs DriverPolicy):
//     wxEnforcement         PermissiveAudit (default Strict)
//     allowEmulatedFallback true           (default false)
//     driver                bwrap            (the emulated tier)
//
// EMULATED-TIER DISCLAIMER: this example does NOT claim hardware-only
// admission. driver="bwrap" with allowEmulatedFallback=true is the
// emulated tier the swe-agent family never exercises; W^X violations
// here are logged (PermissiveAudit), not fatal (Strict). Do not copy
// these driver knobs into a hardware-admission example.
//
// GAPS (flagged, not silently invented):
//   - driverPolicy as a kelir() option: no TS example shows it.
//     The field name follows the Rust VmConfig / DriverPolicy
//     structs (vmm-driver-trait.rs) in TS camelCase idiom, cf.
//     tapFilter / streamEngine in autonomous-swe-agent.ts.
//   - receiptScope was removed from core: 08 deleted the
//     receipt_scope line, and ExecutionReceipt now carries plugin
//     TelemetryExtensions instead, so this example no longer sets
//     it. entropyBypassPaths was likewise removed from VmConfig
//     (entropy analysis moved to plugin-supply-chain).
//   - VFS/PTY knob spellings in TS: plugins/vfs.md attests the field
//     names (encapsulation, scan_engine, carryover_bytes); the TS
//     camelCase forms follow the tapFilter precedent, and
//     carryoverBufferBytes follows the vfs carryover_bytes name in
//     the pty config's idiom.

import { kelir, type Session } from "@kelir/core";
import { pty }    from "@kelir/plugin-pty";
import { vfs }    from "@kelir/plugin-vfs";
import { ledger } from "@kelir/plugin-ledger";

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

  // → c.fs — Passthrough, NOT the Locked default, so the guest
  // workspace is not locked to mode 000; FastRegex, NOT the
  // StreamingAutomata default; 1024-byte carryover, NOT 512.
}).use(vfs({
  encapsulation: "Passthrough",
  scanEngine: "FastRegex",
  carryoverBytes: 1024,
  lockBroker: false,

  // → terminal extension — 1024-byte carryover buffer, NOT the 512 default.
})).use(pty({
  cols: 120, rows: 40,
  dropOsc52: true, stripTitle: true,
  carryoverBufferBytes: 1024,

  // → c.ledger — receipts include syscall counts.
})).use(ledger({
  store: "sqlite://.kelir/audit.merkle.db",
  merkleChain: true,
  receipts: { includeSyscallCount: true },
}));

async function main() {
  const session: Session = await runtime.boot();
  console.log(`[kelir] driver=${session.driverName} boot=${session.bootMs}ms`);
  const probe = await session.spawn("true", [], { cwd: "/workspace" });
  console.log(`[kelir] probe -> exit=${probe.exitCode}`);
  console.log("[kelir] knobs: vfs=Passthrough/FastRegex/1024 " +
              "pty_carryover=1024 " +
              "driver=bwrap wx=PermissiveAudit emulatedFallback=true");
  await session.terminate();
}

main().catch(console.error);
