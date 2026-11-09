#![cfg(target_os = "windows")]

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_24_openvmm_driver_lazy_construction_partition_unset.yaml#case_id
use driver_openvmm::OpenVmmDriver;
// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_24_openvmm_driver_lazy_construction_partition_unset.yaml#case_id
use kelir_interface::driver::VmmDriver;

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_24_openvmm_driver_lazy_construction_partition_unset.yaml#case_id
#[test]
fn hd_24_openvmm_driver_lazy_construction_partition_unset() -> Result<(), String> {
    let driver = match OpenVmmDriver::new() {
        Ok(driver) => driver,
        Err(e) => return Err(format!("OpenVmmDriver::new failed: {e}")),
    };
    assert_eq!(driver.partition_handle(), 0);
    let caps = driver.capabilities();
    assert_eq!(caps.backend, "openvmm");
    assert!(caps.requires_hardware_vmm);
    Ok(())
}
