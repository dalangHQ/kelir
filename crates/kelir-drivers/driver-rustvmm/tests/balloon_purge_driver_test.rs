// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_16_virtio_balloon_pressure_reclaim.yaml#case_id
use driver_rustvmm::RustVmmDriver;
use kelir_interface::driver_interface::VmmDriver;

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_16_virtio_balloon_pressure_reclaim.yaml#case_id
#[tokio::test]
#[ignore]
async fn hd_16_virtio_balloon_pressure_reclaim() -> Result<(), String> {
    let mut driver = RustVmmDriver::new().map_err(|e| e.to_string())?;
    match driver.adjust_balloon(256).await {
        Ok(convergence) => {
            if convergence.target_mb == 256 && convergence.converged {
                Ok(())
            } else {
                Err(format!(
                    "balloon inflate to 256MB reported target_mb {} converged {} without the host madvise(MADV_DONTNEED) reclaim",
                    convergence.target_mb, convergence.converged
                ))
            }
        }
        Err(e) => Err(format!(
            "memory-pressure balloon reclaim never inflated the virtio balloon: {e}"
        )),
    }
}
