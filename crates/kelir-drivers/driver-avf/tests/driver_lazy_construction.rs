#![cfg(target_os = "linux")]

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_23_avf_driver_lazy_construction_avf_fd_unset.yaml#case_id
use driver_avf::AvfDriver;
// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_23_avf_driver_lazy_construction_avf_fd_unset.yaml#case_id
use kelir_interface::driver::VmmDriver;

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_23_avf_driver_lazy_construction_avf_fd_unset.yaml#case_id
fn open_fd_count() -> Result<usize, String> {
    let entries =
        std::fs::read_dir("/proc/self/fd").map_err(|e| format!("read /proc/self/fd: {e}"))?;
    Ok(entries.count())
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_23_avf_driver_lazy_construction_avf_fd_unset.yaml#case_id
fn fd_table_has_avf() -> Result<bool, String> {
    let entries =
        std::fs::read_dir("/proc/self/fd").map_err(|e| format!("read /proc/self/fd: {e}"))?;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let target = match std::fs::read_link(entry.path()) {
            Ok(target) => target,
            Err(_) => continue,
        };
        if target.to_string_lossy() == "/dev/avf" {
            return Ok(true);
        }
    }
    Ok(false)
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_23_avf_driver_lazy_construction_avf_fd_unset.yaml#case_id
#[test]
fn hd_23_avf_driver_lazy_construction_avf_fd_unset() -> Result<(), String> {
    let before = open_fd_count()?;
    let driver = match AvfDriver::new() {
        Ok(driver) => driver,
        Err(e) => return Err(format!("AvfDriver::new failed: {e}")),
    };
    assert_eq!(driver.avf_fd(), -1);
    let caps = driver.capabilities();
    assert_eq!(caps.backend, "avf");
    assert!(caps.requires_hardware_vmm);
    assert!(!fd_table_has_avf()?);
    drop(driver);
    assert_eq!(open_fd_count()?, before);
    Ok(())
}
