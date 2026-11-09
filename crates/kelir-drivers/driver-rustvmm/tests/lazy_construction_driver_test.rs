#![cfg(target_os = "linux")]

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_19_rustvmm_driver_lazy_construction_no_kvm_open.yaml#case_id
use driver_rustvmm::RustVmmDriver;
// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_19_rustvmm_driver_lazy_construction_no_kvm_open.yaml#case_id
use kelir_interface::driver_interface::VmmDriver;

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_19_rustvmm_driver_lazy_construction_no_kvm_open.yaml#case_id
fn open_fd_count() -> Result<usize, String> {
    let entries =
        std::fs::read_dir("/proc/self/fd").map_err(|e| format!("read /proc/self/fd: {e}"))?;
    Ok(entries.count())
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_19_rustvmm_driver_lazy_construction_no_kvm_open.yaml#case_id
fn fd_table_has_kvm() -> Result<bool, String> {
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
        let target = target.to_string_lossy();
        if target == "/dev/kvm" || target.starts_with("anon_inode:[kvm") {
            return Ok(true);
        }
    }
    Ok(false)
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_19_rustvmm_driver_lazy_construction_no_kvm_open.yaml#case_id
#[test]
fn hd_19_rustvmm_driver_lazy_construction_no_kvm_open() -> Result<(), String> {
    let before = open_fd_count()?;
    let driver = match RustVmmDriver::new() {
        Ok(driver) => driver,
        Err(e) => return Err(format!("RustVmmDriver::new failed: {e}")),
    };
    let caps = driver.capabilities();
    assert_eq!(caps.backend, "rustvmm");
    assert!(caps.requires_hardware_vmm);
    assert!(!fd_table_has_kvm()?);
    drop(driver);
    assert_eq!(open_fd_count()?, before);
    Ok(())
}
