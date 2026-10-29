//! examples/knob-matrix.rs
//!
//! Canonical depth example: the non-default knob set, not the swe-agent
//! defaults. Where autonomous-swe-agent.rs runs hardware-only with Locked
//! workspace encapsulation, StreamingAutomata scanning, and workspace-
//! scoped receipts, this file deliberately departs from
//! those defaults — in one place, so knob-coverage has a single home.
//!
//! SAME KNOB SET as knob-matrix.ts and knob-matrix.py. Each knob below
//! is set to a NON-DEFAULT value; the comment on each line names the
//! default it departs from:
//!
//!   STORAGE (05-storage-plane.md):
//!     encapsulation   Passthrough     (default Locked)
//!     scan_engine     FastRegex       (default StreamingAutomata)
//!     carryover_bytes 1024            (default 512)
//!   PTY:
//!     carryover_buffer_bytes 1024     (default 512)
//!   DRIVER (01, vmm-driver-trait.rs DriverPolicy):
//!     wx_enforcement        PermissiveAudit (default Strict)
//!     allow_emulated_fallback true         (default false)
//!     driver                bwrap            (the emulated tier)
//!
//! EMULATED-TIER DISCLAIMER: this example does NOT claim hardware-only
//! admission. driver bwrap with allow_emulated_fallback = true is the
//! emulated tier the swe-agent family never exercises; W^X violations
//! here are logged (PermissiveAudit), not fatal (Strict). Do not copy
//! these driver knobs into a hardware-admission example.
//!
//! GAPS (flagged, not silently invented):
//!   - VmConfig / DriverPolicy shape follows artifacts/code/
//!     vmm-driver-trait.rs (driver_policy, WxPolicy), NOT the slimmer
//!     example-facade VmConfig in policy-matrix.rs, which omits
//!     driver_policy. The facade gap is flagged here rather than
//!     papered over with a DriverChoice.
//!   - receipt_scope was removed from core: 08 deleted the
//!     receipt_scope line, and ExecutionReceipt now carries plugin
//!     TelemetryExtensions instead, so this example no longer sets
//!     it. entropy_bypass_paths was likewise removed from VmConfig
//!     (entropy analysis moved to plugin-supply-chain).
//!   - Storage/Terminal knob enum spellings (WorkspaceEncapsulation::
//!     Passthrough, ReadMaskEngine::FastRegex) are attested in
//!     05-storage-plane.md; carryover_buffer_bytes is the terminal config's
//!     snake_case form of the vfs carryover_bytes name.
//!
//! Cargo.toml:
//!   [dependencies]
//!   kelir = { version = "3.0", features = ["ledger"] }
//!   tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

use kelir::{
    KelirBuilder, Session, VmConfig,
    driver::{DriverPolicy, WxPolicy},
    plugins::{
        ledger::{LedgerConfig, LedgerPlugin, ReceiptConfig},
    },
    planes::{
        terminal::PtyConfig,
        storage::{IdmapConfig, StorageConfig, WorkspaceEncapsulation, ReadMaskEngine},
    },
};

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
    })
    // VFS — Passthrough, NOT the Locked default, so the guest
    // workspace is not locked to mode 000; FastRegex, NOT the
    // StreamingAutomata default; 1024-byte carryover, NOT 512.
    .storage(StorageConfig {
        idmap: IdmapConfig { guest_uid: 0, host_uid: 0 },
        shared_caches: vec![],
        lock_broker: false,
        synthetic_files: vec![],
        mask_read: vec![],
        snoops: vec![],
        encapsulation: WorkspaceEncapsulation::Passthrough,
        scan_engine: ReadMaskEngine::FastRegex,
        carryover_bytes: 1024,
    })
    // PTY — 1024-byte carryover buffer, NOT the 512 default.
    .terminal(PtyConfig {
        cols: 120,
        rows: 40,
        drop_osc52: true,
        strip_title: true,
        token_shield: None,
        prompts: vec![],
        stream_engine: "StreamingAutomata".into(),
        carryover_buffer_bytes: 1024,
    })
    // LEDGER — receipts include syscall counts.
    .plugin(LedgerPlugin::new(LedgerConfig {
        store: ".kelir/audit.merkle.db".into(),
        merkle_chain: true,
        otel: None,
        receipts: ReceiptConfig {
            include_syscall_count: true,
        },
    }))
    .boot()
    .await?;
    eprintln!("[kelir] driver={} boot={}ms", session.driver_name(), session.boot_ms());

    let probe = session.spawn("true").cwd("/workspace").await?;
    eprintln!("[kelir] probe -> exit={}", probe.exit_code);
    eprintln!(
        "[kelir] knobs: vfs=Passthrough/FastRegex/1024 \
         pty_carryover=1024 \
         driver=bwrap wx=PermissiveAudit emulated_fallback=true"
    );

    session.terminate().await?;
    Ok(())
}
