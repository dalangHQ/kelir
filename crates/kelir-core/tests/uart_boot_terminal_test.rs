// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_12_arm64_stage2_4k_on_16k_host.yaml#case_id
use kelir_core::Session;
use kelir_interface::driver_interface::default_vm_config;
use kelir_interface::telemetry::ExitKind;

fn boot_config() -> kelir_interface::driver_interface::VmConfig {
    default_vm_config(
        None,
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    )
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_12_arm64_stage2_4k_on_16k_host.yaml#case_id
#[test]
fn hd_12_arm64_stage2_4k_on_16k_host() -> Result<(), String> {
    let config = boot_config();
    assert_eq!(config.page_size_kb, 4);
    assert_eq!(config.vcpus, 1);
    assert_eq!(config.memory_mb, 256);
    assert!(config.tsc_locked);
    assert_eq!(config.kernel_path.as_os_str(), "kernel/vmlinux");
    assert_eq!(config.rootfs_erofs_path.as_os_str(), "rootfs.erofs");
    Ok(())
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_13_uart_early_boot_panic_scrape.yaml#case_id
#[test]
fn hd_13_uart_early_boot_panic_scrape() -> Result<(), String> {
    let mut session = Session::boot(boot_config()).map_err(|e| e.to_string())?;
    let clean = session.record_early_boot_output(b"early boot ok\n");
    match clean {
        None => {}
        Some(kind) => return Err(format!("clean boot output decoded as exit: {kind:?}")),
    }
    let panicked =
        session.record_early_boot_output(b"Kernel panic - not syncing: test_early_boot_oops\n");
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
    match session.early_boot_exit() {
        Some(ExitKind::KernelPanic(message)) => {
            assert!(message.contains("test_early_boot_oops"));
        }
        other => return Err(format!("early boot exit lost the panic: {other:?}")),
    }
    assert!(!session.uart_ring().is_empty());
    Ok(())
}
