// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_03_hardware_rollback_timing_bound.yaml#case_id
use driver_rustvmm::RustVmmDriver;
use kelir_interface::driver_interface::{SnapshotDescriptor, VmmDriver};

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_03_hardware_rollback_timing_bound.yaml#case_id
fn hd_03_snapshot() -> SnapshotDescriptor {
    SnapshotDescriptor {
        id: "hd_03-rollback".to_string(),
        dirty_bytes: 2_097_152,
        created_at_ns: 0,
        parent_digest: None,
        guest_clock: None,
    }
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_03_hardware_rollback_timing_bound.yaml#case_id
#[tokio::test]
#[ignore]
async fn hd_03_hardware_rollback_timing_bound() -> Result<(), String> {
    let mut driver = RustVmmDriver::new().map_err(|e| e.to_string())?;
    let caps = driver.capabilities();
    if caps.rollback_budget_ms > 8 {
        return Err(format!(
            "rustvmm advertises a {}ms rollback budget; the hardware register+RAM reset bound is 8ms",
            caps.rollback_budget_ms
        ));
    }
    match driver.rollback(&hd_03_snapshot()).await {
        Ok(()) => Ok(()),
        Err(e) => Err(format!(
            "hardware rollback never reset RAM and vCPU registers, so the 8ms bound is unenforced: {e}"
        )),
    }
}
