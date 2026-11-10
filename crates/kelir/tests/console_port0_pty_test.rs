// see: cans/artifacts/test-plan/cases/virtio_rings/vr_09_virtio_console_port0_pty_dedication.yaml#case_id
use kelir::builder_sdk::KelirBuilder;
use kelir::interface::driver_interface::VmConfig;
use kelir::interface::driver_interface::default_vm_config;
use kelir::interface::protocol_wire::StreamKind;

fn builder_config() -> VmConfig {
    default_vm_config(
        None,
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    )
}

// see: cans/artifacts/test-plan/cases/virtio_rings/vr_09_virtio_console_port0_pty_dedication.yaml#case_id
#[tokio::test]
#[ignore]
async fn vr_09_virtio_console_port0_pty_dedication() -> Result<(), String> {
    let session = KelirBuilder::new(builder_config())
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    let builder = session.spawn_builder("ls");
    if builder.stream_kind != StreamKind::Exec {
        return Err(format!(
            "programmatic exec bound to stream kind {:?} instead of EXEC 0x0001; Port 0 must stay reserved for the break-glass PTY",
            builder.stream_kind
        ));
    }
    match builder.await {
        Ok(result) => assert_eq!(result.exit_code, 0),
        Err(e) => {
            return Err(format!(
                "programmatic exec never dispatched over stream kind EXEC (Port 0 must carry zero exec bytes): {e}"
            ));
        }
    }
    Ok(())
}
