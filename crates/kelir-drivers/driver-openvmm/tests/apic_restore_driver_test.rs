// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_07_openvmm_apic_restore_sequence.yaml#case_id
use driver_openvmm::OpenVmmDriver;
use kelir_interface::driver_interface::{SnapshotDescriptor, VmmDriver};

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_07_openvmm_apic_restore_sequence.yaml#case_id
#[tokio::test]
#[ignore]
async fn hd_07_openvmm_apic_restore_sequence() -> Result<(), String> {
    let mut driver = OpenVmmDriver::new().map_err(|e| e.to_string())?;
    let snapshot = SnapshotDescriptor {
        id: "hd_07-apic-restore".to_string(),
        dirty_bytes: 0,
        created_at_ns: 0,
        parent_digest: None,
        guest_clock: None,
    };
    match driver.rollback(&snapshot).await {
        Ok(()) => Ok(()),
        Err(e) => Err(format!(
            "WHP vCPU register restore never committed Local APIC state before the TSC deadline MSRs: {e}"
        )),
    }
}
