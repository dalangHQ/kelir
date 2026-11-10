// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_17_mach_vm_deallocate_footprint_purge.yaml#case_id
use driver_hvf::HvfDriver;
use kelir_interface::driver_interface::VmmDriver;

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_17_mach_vm_deallocate_footprint_purge.yaml#case_id
#[tokio::test]
#[ignore]
async fn hd_17_mach_vm_deallocate_footprint_purge() -> Result<(), String> {
    let mut driver = HvfDriver::new().map_err(|e| e.to_string())?;
    match driver.adjust_balloon(128).await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!(
            "macOS pressure purge of the 128MB guest footprint via mach_vm_deallocate refused: {e}"
        )),
    }
}
