//! examples/knob-matrix.rs
//!
//! Canonical depth example: the non-default knob set, not the swe-agent
//! defaults. Where autonomous-swe-agent.rs runs hardware-only with Locked
//! workspace encapsulation, StreamingAutomata scanning, and workspace-
//! scoped receipts, this file deliberately departs from every one of
//! those defaults — in one place, so knob-coverage has a single home.
//!
//! SAME KNOB SET as knob-matrix.ts and knob-matrix.py. Each knob below
//! is set to a NON-DEFAULT value; the comment on each line names the
//! default it departs from:
//!
//!   VFS (plugins/vfs.md):
//!     encapsulation   Passthrough     (default Locked)
//!     scan_engine     FastRegex       (default StreamingAutomata)
//!     carryover_bytes 1024            (default 512)
//!   PTY:
//!     carryover_buffer_bytes 1024     (default 512)
//!   RECEIPTS (08):
//!     receipt_scope   WholeVmRam      (default WorkspaceInodesOnly)
//!   DRIVER (01, vmm-driver-trait.rs DriverPolicy):
//!     wx_enforcement        PermissiveAudit (default Strict)
//!     allow_emulated_fallback true         (default false)
//!     driver                bwrap            (the emulated tier)
//!   VM (09, VmConfig.entropy_bypass_paths):
//!     entropy_bypass_paths ["/opt/kelir/trusted-runner"]
//!       (a single trusted-runner path, not a wildcard)
//!
//! EMULATED-TIER DISCLAIMER: this example does NOT claim hardware-only
//! admission. driver bwrap with allow_emulated_fallback = true is the
//! emulated tier the swe-agent family never exercises; W^X violations
//! here are logged (PermissiveAudit), not fatal (Strict). Do not copy
//! these driver knobs into a hardware-admission example.
//!
//! GAPS (flagged, not silently invented):
//!   - VmConfig / DriverPolicy shape follows artifacts/code/
//!     vmm-driver-trait.rs (driver_policy, entropy_bypass_paths,
//!     WxPolicy), NOT the slimmer example-facade VmConfig in
//!     policy-matrix.rs, which omits driver_policy. The facade gap is
//!     flagged here rather than papered over with a DriverChoice.
//!   - receipt_scope as a ReceiptConfig field is defined in 08 (parent
//!     decision); the ReceiptScope enum spelling below is the closest
//!     form, not an attested enum name.
//!   - VFS/PTY knob enum spellings (WorkspaceEncapsulation::
//!     Passthrough, ReadMaskEngine::FastRegex) are attested in
//!     plugins/vfs.md; carryover_buffer_bytes is the pty config's
//!     snake_case form of the vfs carryover_bytes name.
//!
//! Cargo.toml:
//!   [dependencies]
//!   kelir = { version = "3.0", features = ["pty", "vfs", "ledger"] }
//!   tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

use kelir::{
    KelirBuilder, Session, VmConfig,
    driver::{DriverPolicy, WxPolicy},
    plugins::{
        ledger::{LedgerConfig, LedgerPlugin, ReceiptConfig, ReceiptScope},
        pty::{PtyConfig, PtyPlugin},
        vfs::{IdmapConfig, VfsConfig, VfsPlugin, WorkspaceEncapsulation, ReadMaskEngine},
    },
};

/// Trusted runner whose entropy check is bypassed: a single explicit
/// path, never a wildcard, never a directory.
const TRUSTED_RUNNER_PATH: &str = "/opt/kelir/trusted-runner";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session: Session = KelirBuilder::new(VmConfig {
        vcpus: 2,
        memory_mb: 2048,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-knobs.erofs".into(),
        page_size_kb: 4,
        tsc_locked: true,
        entropy_seed: None,
        // DRIVER — emulated tier (see header disclaimer). Hardware
        // admission is NOT required here: bwrap cannot satisfy it, so
        // require_hardware_vmm and verify_hw_lockfile are false as a
        // consequence of the emulated tier, not as separate knobs.
        driver_policy: DriverPolicy {
            require_hardware_vmm: false,
            // true, NOT the false default (01): bwrap/proot fallback
            // is banned by default; this example opts in.
            allow_emulated_fallback: true,
            verify_hw_lockfile: false,
            // PermissiveAudit, NOT the Strict default (01): W^X
            // violations are logged, not fatal.
            wx_enforcement: WxPolicy::PermissiveAudit,
            ffi_worker_threads: 4,
            ffi_max_ops_per_sec: 1000,
        },
        enable_gpu_passthrough: false,
        enable_watchdog: true,
        deterministic_clock: false,
        pinned_clocksource: None,
        ns_per_instruction: None,
        preemption_grace_ms: 0,
        // One trusted-runner path, not a wildcard (09).
        entropy_bypass_paths: vec![TRUSTED_RUNNER_PATH.into()],
    })
    // VFS — Passthrough, NOT the Locked default, so the guest
    // workspace is not locked to mode 000; FastRegex, NOT the
    // StreamingAutomata default; 1024-byte carryover, NOT 512.
    .plugin(VfsPlugin::new(VfsConfig {
        idmap: IdmapConfig { guest_uid: 0, host_uid: 0 },
        shared_caches: vec![],
        lock_broker: false,
        synthetic_files: vec![],
        mask_read: vec![],
        snoops: vec![],
        encapsulation: WorkspaceEncapsulation::Passthrough,
        scan_engine: ReadMaskEngine::FastRegex,
        carryover_bytes: 1024,
    }))
    // PTY — 1024-byte carryover buffer, NOT the 512 default.
    .plugin(PtyPlugin::new(PtyConfig {
        cols: 120,
        rows: 40,
        drop_osc52: true,
        strip_title: true,
        token_shield: None,
        prompts: vec![],
        stream_engine: "StreamingAutomata".into(),
        carryover_buffer_bytes: 1024,
    }))
    // LEDGER — WholeVmRam receipts, NOT the WorkspaceInodesOnly
    // default: the receipt covers whole-VM RAM mutations.
    .plugin(LedgerPlugin::new(LedgerConfig {
        store: ".kelir/audit.merkle.db".into(),
        merkle_chain: true,
        otel: None,
        receipts: ReceiptConfig {
            include_syscall_count: true,
            receipt_scope: ReceiptScope::WholeVmRam,
        },
    }))
    .boot()
    .await?;
    eprintln!("[kelir] driver={} boot={}ms", session.driver_name(), session.boot_ms());

    let probe = session.spawn("true").cwd("/workspace").await?;
    eprintln!("[kelir] probe -> exit={}", probe.exit_code);
    eprintln!(
        "[kelir] knobs: vfs=Passthrough/FastRegex/1024 \
         pty_carryover=1024 receipt_scope=WholeVmRam \
         driver=bwrap wx=PermissiveAudit emulated_fallback=true \
         entropy_bypass=[{TRUSTED_RUNNER_PATH}]"
    );

    session.terminate().await?;
    Ok(())
}
