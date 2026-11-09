// see: cans/artifacts/test-plan/cases/attestation_receipts/ar_06_exit_kind_kernel_panic_uart_scrape.yaml#case_id
use kelir_core::uart_terminal::{UartRingBuffer, decode_early_boot_exit};
use kelir_interface::telemetry::{ExecutionReceipt, ExitKind};

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
