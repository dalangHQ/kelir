// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_06_hvf_stage2_chunk_promoter.yaml#case_id
use driver_hvf::HvfDriver;
use kelir_interface::driver_interface::VmmDriver;

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_06_hvf_stage2_chunk_promoter.yaml#case_id
#[tokio::test]
#[ignore]
async fn hd_06_hvf_stage2_chunk_promoter() -> Result<(), String> {
    let driver = HvfDriver::new().map_err(|e| e.to_string())?;
    match driver.fork().await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!(
            "HVF Stage-2 CoW fork (hv_vm_protect plus the 2MB chunk promoter) refused: {e}"
        )),
    }
}
