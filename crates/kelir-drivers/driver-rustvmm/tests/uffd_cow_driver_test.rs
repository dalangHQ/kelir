// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_05_kvm_userfaultfd_cow_tracking.yaml#case_id
use driver_rustvmm::RustVmmDriver;
use kelir_interface::driver_interface::VmmDriver;

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_05_kvm_userfaultfd_cow_tracking.yaml#case_id
#[tokio::test]
#[ignore]
async fn hd_05_kvm_userfaultfd_cow_tracking() -> Result<(), String> {
    let mut driver = RustVmmDriver::new().map_err(|e| e.to_string())?;
    match driver.checkpoint("hd_05-uffd-cow").await {
        Ok(descriptor) => {
            if descriptor.dirty_bytes == 0 {
                Ok(())
            } else {
                Err(format!(
                    "checkpoint of an unwritten guest reports {} dirty bytes",
                    descriptor.dirty_bytes
                ))
            }
        }
        Err(e) => Err(format!(
            "userfaultfd CoW dirty-page checkpoint refused, so no fault is ever tracked: {e}"
        )),
    }
}
