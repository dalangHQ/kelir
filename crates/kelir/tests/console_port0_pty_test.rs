// see: cans/artifacts/test-plan/cases/virtio_rings/vr_09_virtio_console_port0_pty_dedication.yaml#case_id
use kelir::builder_sdk::KelirBuilder;
use kelir::interface::driver_interface::VmConfig;
use kelir::interface::protocol_wire::StreamKind;

fn builder_config() -> VmConfig {
    VmConfig {
        kernel_path: "kernel/vmlinux".into(),
        rootfs_erofs_path: "rootfs.erofs".into(),
        ..VmConfig::default()
    }
}

// see: cans/artifacts/test-plan/cases/virtio_rings/vr_09_virtio_console_port0_pty_dedication.yaml#case_id
#[tokio::test]
#[ignore]
async fn vr_09_virtio_console_port0_pty_dedication() -> Result<(), String> {
    let session = KelirBuilder::new(builder_config())
        .seal()
        .map_err(|e| e.to_string())?
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
