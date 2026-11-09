#![cfg(target_os = "linux")]

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_21_proot_driver_rejects_construction_without_opt_in.yaml#case_id
use driver_proot::ProotDriver;
// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_21_proot_driver_rejects_construction_without_opt_in.yaml#case_id
use kelir_interface::driver::{VmmDriver, VmmError};

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_21_proot_driver_rejects_construction_without_opt_in.yaml#case_id
fn open_fd_count() -> Result<usize, String> {
    let entries =
        std::fs::read_dir("/proc/self/fd").map_err(|e| format!("read /proc/self/fd: {e}"))?;
    Ok(entries.count())
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_21_proot_driver_rejects_construction_without_opt_in.yaml#case_id
fn child_pids() -> Result<String, String> {
    let pid = std::process::id();
    let path = format!("/proc/self/task/{pid}/children");
    std::fs::read_to_string(path).map_err(|e| format!("read children: {e}"))
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_21_proot_driver_rejects_construction_without_opt_in.yaml#case_id
#[test]
fn hd_21_proot_driver_rejects_construction_without_opt_in() -> Result<(), String> {
    let before = open_fd_count()?;
    let children_before = child_pids()?;
    match ProotDriver::new(false) {
        Err(VmmError::Unsupported(msg)) => assert_eq!(msg, "emulated fallback not allowed"),
        Err(other) => {
            return Err(format!("ProotDriver::new(false) wrong error: {other}"));
        }
        Ok(_) => {
            return Err("ProotDriver::new(false) succeeded without opt-in".to_string());
        }
    }
    let driver = match ProotDriver::new(true) {
        Ok(driver) => driver,
        Err(e) => return Err(format!("ProotDriver::new(true) failed: {e}")),
    };
    let caps = driver.capabilities();
    assert_eq!(caps.backend, "proot");
    assert!(!caps.requires_hardware_vmm);
    drop(driver);
    assert_eq!(open_fd_count()?, before);
    assert_eq!(child_pids()?, children_before);
    Ok(())
}
