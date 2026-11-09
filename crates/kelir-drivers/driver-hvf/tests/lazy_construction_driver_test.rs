#![cfg(target_os = "macos")]

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_22_hvf_driver_lazy_construction_no_entitlement_touch.yaml#case_id
use driver_hvf::HvfDriver;
// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_22_hvf_driver_lazy_construction_no_entitlement_touch.yaml#case_id
use kelir_interface::driver_interface::VmmDriver;

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_22_hvf_driver_lazy_construction_no_entitlement_touch.yaml#case_id
fn open_fd_count() -> Result<usize, String> {
    let entries = std::fs::read_dir("/dev/fd").map_err(|e| format!("read /dev/fd: {e}"))?;
    Ok(entries.count())
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_22_hvf_driver_lazy_construction_no_entitlement_touch.yaml#case_id
#[test]
fn hd_22_hvf_driver_lazy_construction_no_entitlement_touch() -> Result<(), String> {
    let before = open_fd_count()?;
    let driver = match HvfDriver::new() {
        Ok(driver) => driver,
        Err(e) => return Err(format!("HvfDriver::new failed: {e}")),
    };
    assert_eq!(driver.vcpu_count(), 0);
    assert_eq!(driver.memory_mapped_bytes(), 0);
    let caps = driver.capabilities();
    assert_eq!(caps.backend, "hvf");
    assert!(caps.requires_hardware_vmm);
    drop(driver);
    assert_eq!(open_fd_count()?, before);
    Ok(())
}
