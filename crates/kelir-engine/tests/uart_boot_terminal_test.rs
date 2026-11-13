// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_12_arm64_stage2_4k_on_16k_host.yaml#case_id
use std::sync::Arc;

use kelir_engine::Session;
use kelir_interface::driver_interface::default_vm_config;
use kelir_interface::telemetry::ExitKind;

fn plane_wiring() -> kelir_engine::session_lifecycle::PlaneWiring {
    let metrics = || Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new());
    let dns_ops: Arc<dyn kelir_interface::wire::dns::DnsOps> = kelir_wire::dns_ops();
    kelir_engine::session_lifecycle::PlaneWiring {
        terminal_ops: Some(kelir_terminal::terminal_ops(metrics())),
        checkpoint_ops: Some(kelir_checkpoint::checkpoint_ops(metrics())),
        host_ops: Some(kelir_host::host_ops(metrics())),
        net_ops: Some(kelir_wire::net_ops(Arc::clone(&dns_ops))),
        dns_ops: Some(dns_ops),
        udp_nat_ops: Some(kelir_wire::udp_nat_ops()),
        l4_ops: Some(kelir_wire::l4_ops()),
        storage_io: Some(kelir_storage_io::storage_io()),
        ..Default::default()
    }
}

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
    let mut session =
        Session::boot_with_plugins(boot_config(), &[], None, Vec::new(), plane_wiring())
            .map_err(|e| e.to_string())?;
    session.record_early_boot_output(b"early boot ok\n");
    session.record_early_boot_output(b"Kernel panic - not syncing: test_early_boot_oops\n");
    match session.early_boot_exit() {
        Some(ExitKind::KernelPanic(message)) => {
            assert!(message.contains("test_early_boot_oops"));
        }
        other => return Err(format!("early boot exit lost the panic: {other:?}")),
    }
    assert!(!session.uart_ring().is_empty());
    Ok(())
}
