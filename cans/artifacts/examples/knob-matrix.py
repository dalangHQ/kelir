# examples/knob-matrix.py
"""
Canonical depth example: the non-default knob set, not the swe-agent
defaults. Where autonomous-swe-agent.py runs hardware-only with Locked
workspace encapsulation, StreamingAutomata scanning, and workspace-
scoped receipts, this file deliberately departs from every one of
those defaults — in one place, so knob-coverage has a single home.

SAME KNOB SET as knob-matrix.rs and knob-matrix.ts. Each knob below
is set to a NON-DEFAULT value; the comment on each line names the
default it departs from:

  VFS (plugins/vfs.md):
    encapsulation    Passthrough     (default Locked)
    scan_engine      FastRegex       (default StreamingAutomata)
    carryover_bytes  1024            (default 512)
  PTY:
    carryover_buffer_bytes 1024      (default 512)
  RECEIPTS (08):
    receipt_scope    WholeVmRam      (default WorkspaceInodesOnly)
  DRIVER (01, vmm-driver-trait.rs DriverPolicy):
    wx_enforcement         PermissiveAudit (default Strict)
    allow_emulated_fallback True           (default False)
    driver                 bwrap            (the emulated tier)
  VM (09, VmConfig.entropy_bypass_paths):
    entropy_bypass_paths ["/opt/kelir/trusted-runner"]
      (a single trusted-runner path, not a wildcard)

EMULATED-TIER DISCLAIMER: this example does NOT claim hardware-only
admission. driver="bwrap" with allow_emulated_fallback=True is the
emulated tier the swe-agent family never exercises; W^X violations
here are logged (PermissiveAudit), not fatal (Strict). Do not copy
these driver knobs into a hardware-admission example.

GAPS (flagged, not silently invented):
  - driver_policy / entropy_bypass_paths as kelir.builder kwargs: no
    Python example shows them. The field names follow the Rust
    VmConfig / DriverPolicy structs (vmm-driver-trait.rs) in the
    Python snake_case idiom, cf. the tap_filter / stream_engine
    string kwargs in autonomous-swe-agent.py.
  - receipt_scope as a ReceiptConfig field: the field is defined in
    08 (parent decision); no Python example sets it, so the string
    form follows the include_syscall_count precedent in the same
    ReceiptConfig literal.
  - VFS/PTY knob spellings in Python: plugins/vfs.md attests the
    field names (encapsulation, scan_engine, carryover_bytes); the
    Python string forms follow the tap_filter precedent, and
    carryover_buffer_bytes follows the vfs carryover_bytes name in
    the pty config's idiom.

Requirements:
    kelir>=3.0
"""

from __future__ import annotations

import asyncio
import platform

import kelir
from kelir.plugins.ledger import LedgerConfig, LedgerPlugin, ReceiptConfig
from kelir.plugins.pty import PtyPlugin
from kelir.plugins.vfs import VfsPlugin


# Trusted runner whose entropy check is bypassed: a single explicit
# path, never a wildcard, never a directory.
TRUSTED_RUNNER_PATH = "/opt/kelir/trusted-runner"


async def main() -> None:
    builder = (
        kelir.builder(
            vcpus=2,
            memory_mb=2048,
            kernel=f"./fixtures/vmlinux-6.6-{platform.machine()}-kelir",
            rootfs_erofs="./fixtures/rootfs-knobs.erofs",
            tsc_locked=True,
            enable_watchdog=True,
            # DRIVER — emulated tier (see header disclaimer).
            driver="bwrap",
            driver_policy={
                # PermissiveAudit, NOT the Strict default (01):
                # W^X violations are logged, not fatal.
                "wx_enforcement": "PermissiveAudit",
                # True, NOT the False default (01): bwrap/proot
                # fallback is banned by default; this example opts in.
                "allow_emulated_fallback": True,
            },
            # One trusted-runner path, not a wildcard (09).
            entropy_bypass_paths=[TRUSTED_RUNNER_PATH],
        )
        # VFS — Passthrough, NOT the Locked default, so the guest
        # workspace is not locked to mode 000; FastRegex, NOT the
        # StreamingAutomata default; 1024-byte carryover, NOT 512.
        .plugin(VfsPlugin(
            encapsulation="Passthrough",
            scan_engine="FastRegex",
            carryover_bytes=1024,
            lock_broker=False,
        ))
        # PTY — 1024-byte carryover buffer, NOT the 512 default.
        .plugin(PtyPlugin(
            cols=120,
            rows=40,
            drop_osc52=True,
            strip_title=True,
            carryover_buffer_bytes=1024,
        ))
        # LEDGER — WholeVmRam receipts, NOT the WorkspaceInodesOnly
        # default: the receipt covers whole-VM RAM mutations.
        .plugin(LedgerPlugin(LedgerConfig(
            store="sqlite://.kelir/audit.merkle.db",
            merkle_chain=True,
            receipts=ReceiptConfig(
                include_syscall_count=True,
                receipt_scope="WholeVmRam",
            ),
        )))
    )

    async with builder as session:
        print(f"[kelir] booted driver={session.driver_name} "
              f"boot_ms={session.boot_ms}")
        probe = await session.spawn("true", [], cwd="/workspace")
        print(f"[kelir] probe -> exit={probe.exit_code}")
        print("[kelir] knobs: vfs=Passthrough/FastRegex/1024 "
              "pty_carryover=1024 receipt_scope=WholeVmRam "
              "driver=bwrap wx=PermissiveAudit emulated_fallback=True "
              f"entropy_bypass=[{TRUSTED_RUNNER_PATH}]")


if __name__ == "__main__":
    asyncio.run(main())
