// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_08_avf_pkvm_boundary_enforcement.yaml#case_id
use driver_avf::AvfDriver;
use kelir_interface::driver_interface::{VmmDriver, VmmError};

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_08_avf_pkvm_boundary_enforcement.yaml#case_id
#[tokio::test]
#[ignore]
async fn hd_08_avf_pkvm_boundary_enforcement() -> Result<(), String> {
    let mut driver = AvfDriver::new().map_err(|e| e.to_string())?;
    match driver.map_shared_region("guest-ram", 4096).await {
        Ok(_) => Err(
            "mapping a host range never donated to pKVM succeeded instead of trapping".to_string(),
        ),
        Err(VmmError::Unsupported(message)) => {
            if message.contains("E_PKVM_NON_DONATED_MEMORY") {
                Ok(())
            } else {
                Err(format!(
                    "non-donated memory map must fail closed with the E_PKVM_NON_DONATED_MEMORY diagnostic; got: {message}"
                ))
            }
        }
        Err(other) => Err(format!(
            "non-donated memory map gave the wrong error kind: {other}"
        )),
    }
}
