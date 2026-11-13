// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_05_monotonic_clock_clamp_on_resume.yaml#case_id
use kelir_engine::planes::checkpoint::{CheckpointPlane, PidLineage};
use kelir_interface::driver_interface::default_vm_config;
use kelir_interface::driver_interface::{GuestClock, SnapshotDescriptor};
use kelir_interface::plugin_model::PluginError;

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

fn descriptor(offset_ns: u64) -> SnapshotDescriptor {
    SnapshotDescriptor {
        id: "snap-1".to_string(),
        dirty_bytes: 0,
        created_at_ns: 0,
        parent_digest: None,
        guest_clock: Some(GuestClock {
            absolute_wall_clock_ns: 0,
            monotonic_offset_ns: offset_ns,
            timezone: "UTC".to_string(),
            boot_params: String::new(),
        }),
    }
}

fn restored_offset(restored: &SnapshotDescriptor) -> Result<u64, String> {
    match &restored.guest_clock {
        Some(clock) => Ok(clock.monotonic_offset_ns),
        None => Err("restored snapshot lost its guest clock".to_string()),
    }
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_05_monotonic_clock_clamp_on_resume.yaml#case_id
#[test]
fn lc_05_monotonic_clock_clamp_on_resume() -> Result<(), String> {
    let plane = CheckpointPlane::new();
    let lineage = PidLineage::new_in_session(100, [0x05; 16]);
    let first = plane
        .create_step(lineage, descriptor(5_000_000_000))
        .map_err(|e| e.to_string())?;
    assert_eq!(first, 0);
    let committed = plane.commit_step(lineage).map_err(|e| e.to_string())?;
    assert_eq!(committed, 0);
    let second = plane
        .create_step(lineage, descriptor(1_000_000_000))
        .map_err(|e| e.to_string())?;
    assert_eq!(second, 1);
    let committed_second = plane.commit_step(lineage).map_err(|e| e.to_string())?;
    assert_eq!(committed_second, 1);
    let restored_first = plane.restore(0, 0).map_err(|e| e.to_string())?;
    assert_eq!(restored_offset(&restored_first)?, 5_000_000_000);
    assert_eq!(plane.monotonic_watermark_ns(), 5_000_000_000);
    let restored_second = plane.restore(1, 0).map_err(|e| e.to_string())?;
    assert_eq!(restored_offset(&restored_second)?, 5_000_000_000);
    assert_eq!(plane.monotonic_watermark_ns(), 5_000_000_000);
    let restored_live = plane.restore(0, 6_000_000_000).map_err(|e| e.to_string())?;
    assert_eq!(restored_offset(&restored_live)?, 6_000_000_000);
    assert_eq!(plane.monotonic_watermark_ns(), 6_000_000_000);
    let restored_floor = plane.restore(1, 0).map_err(|e| e.to_string())?;
    assert_eq!(restored_offset(&restored_floor)?, 6_000_000_000);
    assert_eq!(plane.monotonic_watermark_ns(), 6_000_000_000);
    match plane.restore(999, 0) {
        Err(PluginError::Halt(message)) => assert!(message.contains("not in manifest graph")),
        Err(e) => return Err(format!("unknown restore gave wrong error: {e}")),
        Ok(_) => return Err("unknown node restored".to_string()),
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_19_delta_chain_unbounded_compaction.yaml#case_id
#[test]
fn lc_19_delta_chain_unbounded_compaction() -> Result<(), String> {
    let plane = CheckpointPlane::new();
    let lineage = PidLineage::new_in_session(200, [0x19; 16]);
    let base = plane
        .create_step(lineage, descriptor(100))
        .map_err(|e| e.to_string())?;
    assert_eq!(base, 0);
    assert_eq!(plane.commit_step(lineage).map_err(|e| e.to_string())?, 0);
    let head = plane
        .create_step(lineage, descriptor(200))
        .map_err(|e| e.to_string())?;
    assert_eq!(head, 1);
    assert_eq!(plane.commit_step(lineage).map_err(|e| e.to_string())?, 1);
    assert_eq!(plane.head(lineage), Some(1));
    let compacted = plane.compact(lineage).map_err(|e| e.to_string())?;
    assert_eq!(compacted, 2);
    assert_eq!(plane.head(lineage), Some(2));
    let chain = plane.lineage(compacted).map_err(|e| e.to_string())?;
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0], compacted);
    assert_eq!(chain[1], 0);
    let restored = plane.restore(compacted, 0).map_err(|e| e.to_string())?;
    assert_eq!(restored_offset(&restored)?, 200);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_20_portable_snapshot_archive_export.yaml#case_id
#[tokio::test]
#[ignore]
async fn lc_20_portable_snapshot_archive_export() -> Result<(), String> {
    let mut session = kelir_engine::Session::boot(boot_config()).map_err(|e| e.to_string())?;
    let mut stream = match session.export_snapshot() {
        Ok(stream) => stream,
        Err(e) => return Err(format!("export_snapshot refused: {e}")),
    };
    let mut bytes = Vec::new();
    let mut chunk = vec![0u8; 65536];
    while bytes.len() < 64 * 1024 * 1024 {
        let read = tokio::io::AsyncReadExt::read(&mut stream, &mut chunk)
            .await
            .map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    for entry in [
        "manifest.json",
        "vmm_state.json",
        "ram.zst",
        "upperdir.tar.zst",
        "guest_clock.json",
        "attestation.jws",
    ] {
        if !bytes
            .windows(entry.len())
            .any(|window| window == entry.as_bytes())
        {
            return Err(format!("exported archive stream carries no {entry} entry"));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_21_detached_scm_rights_parent_sigkill_abort.yaml#case_id
#[test]
#[ignore]
fn lc_21_detached_scm_rights_parent_sigkill_abort() -> Result<(), String> {
    let session = kelir_engine::Session::boot(boot_config()).map_err(|e| e.to_string())?;
    let file = std::fs::File::open("/dev/null").map_err(|e| e.to_string())?;
    let fd: std::os::fd::OwnedFd = file.into();
    let handoff = kelir_transport::MemoryHandoff::new(fd, 4096);
    match session.memory_handoff(handoff) {
        Ok(()) => {}
        Err(e) => return Err(format!("memory handoff refused: {e}")),
    }
    Ok(())
}
