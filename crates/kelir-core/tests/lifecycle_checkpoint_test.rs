// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_05_monotonic_clock_clamp_on_resume.yaml#case_id
use kelir_core::planes::checkpoint::{CheckpointPlane, PidLineage};
use kelir_interface::driver_interface::{GuestClock, SnapshotDescriptor};
use kelir_interface::plugin_model::PluginError;

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
    let restored_first = plane.restore(0).map_err(|e| e.to_string())?;
    assert_eq!(restored_offset(&restored_first)?, 5_000_000_000);
    assert_eq!(plane.monotonic_watermark_ns(), 5_000_000_000);
    let restored_second = plane.restore(1).map_err(|e| e.to_string())?;
    assert_eq!(restored_offset(&restored_second)?, 5_000_000_000);
    assert_eq!(plane.monotonic_watermark_ns(), 5_000_000_000);
    match plane.restore(999) {
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
    let restored = plane.restore(compacted).map_err(|e| e.to_string())?;
    assert_eq!(restored_offset(&restored)?, 200);
    Ok(())
}
