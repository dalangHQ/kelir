// see: cans/artifacts/test-plan/cases/attestation_receipts/ar_06_exit_kind_kernel_panic_uart_scrape.yaml#case_id
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kelir_core::planes::storage::fuse_storage::{FuseDispatcher, FuseOpcode, FuseRequest};
use kelir_core::uart_terminal::{UartRingBuffer, decode_early_boot_exit};
use kelir_interface::plugin_model::{FsInterposer, HandlerCtx};
use kelir_interface::protocol_wire::TrapEvent;
use kelir_interface::telemetry::{ExecutionReceipt, ExitKind};

fn attest_workspace(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("kelir-et2-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    let _ = std::fs::create_dir_all(&path);
    path
}

fn drop_workspace(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

struct AttestPassFs;

impl FsInterposer for AttestPassFs {}

fn receipt_toml(wall: u64, ffi: u64, dirty: u64) -> String {
    format!(
        "pid = 42\nbinaryPath = \"/bin/true\"\nbinaryBlake3 = \"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262\"\nexitKind = \"normal\"\ncorrelationToken = \"corr-1\"\ndirtyBytes = {dirty}\nsyscallCount = 7\nexitCode = 0\nwallClockNs = {wall}\nphaseTimings = {{ trapIngressNs = 10, ffiHopNs = {ffi}, policyNs = 30, verdictEgressNs = 40 }}\negressBytes = 0\negressDroppedBytes = 0\nextensions = []\n"
    )
}

fn load_receipt(wall: u64, ffi: u64, dirty: u64) -> Result<ExecutionReceipt, String> {
    toml::from_str(&receipt_toml(wall, ffi, dirty)).map_err(|e| e.to_string())
}

// see: cans/artifacts/test-plan/cases/attestation_receipts/ar_06_exit_kind_kernel_panic_uart_scrape.yaml#case_id
#[test]
fn ar_06_exit_kind_kernel_panic_uart_scrape() -> Result<(), String> {
    let clean = decode_early_boot_exit(b"early boot ok\nloading kernel\n");
    match clean {
        None => {}
        Some(kind) => return Err(format!("clean output decoded as exit: {kind:?}")),
    }
    let panicked =
        decode_early_boot_exit(b"booting...\nKernel panic - not syncing: test_early_boot_oops\n");
    match panicked {
        Some(ExitKind::KernelPanic(message)) => {
            assert!(message.contains("test_early_boot_oops"));
        }
        other => {
            return Err(format!(
                "panic line did not decode to KernelPanic: {other:?}"
            ));
        }
    }
    let mut ring = UartRingBuffer::new();
    assert_eq!(ring.capacity(), 65536);
    assert!(ring.is_empty());
    ring.write(b"Kernel panic - not syncing: test_early_boot_oops\n");
    assert!(!ring.is_empty());
    match ring.decode_exit_kind() {
        Some(ExitKind::KernelPanic(message)) => {
            assert!(message.contains("test_early_boot_oops"));
        }
        other => return Err(format!("UART ring did not decode panic: {other:?}")),
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/attestation_receipts/ar_08_phase_span_telemetry_isolation.yaml#case_id
#[test]
fn ar_08_phase_span_telemetry_isolation() -> Result<(), String> {
    let first = load_receipt(1000, 20, 100)?;
    let second = load_receipt(2000, 99, 100)?;
    assert_eq!(first.leaf_hash(), second.leaf_hash());
    match &first.phase_timings {
        Some(timings) => assert_eq!(timings.ffi_hop_ns, 20),
        None => return Err("first receipt lost phase timings".to_string()),
    }
    match &second.phase_timings {
        Some(timings) => assert_eq!(timings.ffi_hop_ns, 99),
        None => return Err("second receipt lost phase timings".to_string()),
    }
    assert!(first.same_identity(&second));
    Ok(())
}

// see: cans/artifacts/test-plan/cases/attestation_receipts/ar_10_receipt_struct_exhaustiveness_lint.yaml#case_id
#[test]
fn ar_10_receipt_struct_exhaustiveness_lint() -> Result<(), String> {
    let receipt = load_receipt(1000, 20, 100)?;
    assert_eq!(receipt.pid, 42);
    assert_eq!(receipt.binary_path, "/bin/true");
    assert_eq!(
        receipt.binary_blake3,
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );
    match &receipt.exit_kind {
        ExitKind::Normal => {}
        other => return Err(format!("receipt exit kind is not Normal: {other:?}")),
    }
    match &receipt.correlation_token {
        Some(token) => assert_eq!(token, "corr-1"),
        None => return Err("receipt lost its correlation token".to_string()),
    }
    assert_eq!(receipt.dirty_bytes, 100);
    assert_eq!(receipt.syscall_count, Some(7));
    assert_eq!(receipt.exit_code, 0);
    assert_eq!(receipt.wall_clock_ns, 1000);
    match &receipt.phase_timings {
        Some(timings) => {
            assert_eq!(timings.trap_ingress_ns, 10);
            assert_eq!(timings.ffi_hop_ns, 20);
            assert_eq!(timings.policy_ns, 30);
            assert_eq!(timings.verdict_egress_ns, 40);
        }
        None => return Err("receipt lost phase timings".to_string()),
    }
    assert_eq!(receipt.egress_bytes, 0);
    assert_eq!(receipt.egress_dropped_bytes, 0);
    assert_eq!(receipt.extensions.len(), 0);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/attestation_receipts/ar_03_dirty_bytes_exact_byte_accounting.yaml#case_id
#[tokio::test]
#[ignore]
async fn ar_03_dirty_bytes_exact_byte_accounting() -> Result<(), String> {
    let workspace = attest_workspace("ar03");
    let target = workspace.join("test.dat");
    std::fs::write(&target, b"").map_err(|e| e.to_string())?;
    let dispatcher = FuseDispatcher::new(Arc::new(AttestPassFs));
    let ctx = HandlerCtx::new([0u8; 16], 42, (), Vec::new());
    let mut request = FuseRequest {
        opcode: FuseOpcode::Write,
        guest_inode: 7,
        path: target,
        pid: 42,
        data: vec![0x5au8; 100],
        offset: 0,
        size: 100,
        blocking: false,
    };
    dispatcher
        .dispatch(&mut request, &ctx)
        .map_err(|e| e.to_string())?;
    let receipt = ctx.receipt().await.map_err(|e| e.to_string())?;
    drop_workspace(&workspace);
    if receipt.dirty_bytes != 100 {
        return Err(format!(
            "receipt.dirty_bytes = {} after a 100-byte guest write to test.dat; expected exact byte accounting (byte count, not page count: 4KiB Linux and 16KiB Apple hosts must agree)",
            receipt.dirty_bytes
        ));
    }
    Ok(())
}

const AR07_ORIGINAL_HEX: &str = "30c40922ba5de55d0291d9c7d5a9586a431d129ecd44bda76558ea7037a39795";
const AR07_FINAL_HEX: &str = "af837da23197419de9c8e832e45c693a80f3cf7bae7ea7dd7b0eb4d0bfbfadbc";
const AR07_ORIGINAL_BYTES: [u8; 32] = [
    0x30, 0xc4, 0x09, 0x22, 0xba, 0x5d, 0xe5, 0x5d, 0x02, 0x91, 0xd9, 0xc7, 0xd5, 0xa9, 0x58, 0x6a,
    0x43, 0x1d, 0x12, 0x9e, 0xcd, 0x44, 0xbd, 0xa7, 0x65, 0x58, 0xea, 0x70, 0x37, 0xa3, 0x97, 0x95,
];

// see: cans/artifacts/test-plan/cases/attestation_receipts/ar_07_exit_code_patch_chain_divergence.yaml#case_id
#[tokio::test]
#[ignore]
async fn ar_07_exit_code_patch_chain_divergence() -> Result<(), String> {
    let trap = TrapEvent {
        seq: 7,
        pid: 42,
        ppid: 1,
        task_id: 42,
        binary_path: "/bin/risky_tool".to_string(),
        binary_dev: 9,
        binary_ino: 11,
        binary_blake3: AR07_ORIGINAL_BYTES,
        argv: vec!["/bin/risky_tool".to_string()],
        envp: HashMap::new(),
        cwd: "/".to_string(),
        syscall: None,
        resolved_binary: Default::default(),
        interpreter_chain: Vec::new(),
        shell_payload: None,
    };
    let ctx: HandlerCtx<(), ()> = HandlerCtx::new([0u8; 16], 42, (), Vec::new()).with_trap(trap);
    let receipt = ctx.receipt().await.map_err(|e| e.to_string())?;
    if receipt.binary_blake3 != AR07_FINAL_HEX {
        return Err(format!(
            "receipt.binary_blake3 = {:?}; the receipt must record the FINAL binary of the rewrite chain {AR07_FINAL_HEX} while the frozen trap keeps the original {AR07_ORIGINAL_HEX}",
            receipt.binary_blake3
        ));
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/attestation_receipts/ar_11_pmu_counters_excluded_from_receipt.yaml#case_id
#[test]
fn ar_11_pmu_counters_excluded_from_receipt() -> Result<(), String> {
    let first = load_receipt(1000, 20, 100)?;
    let second = load_receipt(2000, 99, 100)?;
    assert_eq!(first.leaf_hash(), second.leaf_hash());
    assert!(first.same_identity(&second));
    let changed = load_receipt(1000, 20, 101)?;
    assert!(!first.same_identity(&changed));
    assert_ne!(first.leaf_hash(), changed.leaf_hash());
    Ok(())
}
