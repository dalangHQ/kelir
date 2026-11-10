// see: cans/artifacts/test-plan/cases/virtio_rings/vr_07_virtio_shm_unmap_on_teardown.yaml#case_id
use kelir_interface::driver_interface::VmmDriver;
use kelir_interface::driver_interface::WatchdogAction;

fn maps_cover(ptr: usize, len: usize) -> Result<bool, String> {
    let text = std::fs::read_to_string("/proc/self/maps").map_err(|e| e.to_string())?;
    for line in text.lines() {
        let Some((start, rest)) = line.split_once('-') else {
            continue;
        };
        let Some(end) = rest.split_whitespace().next() else {
            continue;
        };
        let start = usize::from_str_radix(start, 16).map_err(|e| e.to_string())?;
        let end = usize::from_str_radix(end, 16).map_err(|e| e.to_string())?;
        if start < ptr.wrapping_add(len) && ptr < end {
            return Ok(true);
        }
    }
    Ok(false)
}

fn fd_open(raw: std::os::unix::io::RawFd) -> bool {
    std::fs::read_link(format!("/proc/self/fd/{raw}")).is_ok()
}

// see: cans/artifacts/test-plan/cases/virtio_rings/vr_07_virtio_shm_unmap_on_teardown.yaml#case_id
#[tokio::test]
#[ignore]
async fn vr_07_virtio_shm_unmap_on_teardown() -> Result<(), String> {
    let mut driver = driver_rustvmm::RustVmmDriver::new().map_err(|e| e.to_string())?;
    let handle_a = match driver.map_shared_region("shm-teardown-a", 65536).await {
        Ok(handle) => handle,
        Err(e) => return Err(format!("map_shared_region refused: {e}")),
    };
    let handle_b = match driver.map_shared_region("shm-teardown-b", 65536).await {
        Ok(handle) => handle,
        Err(e) => return Err(format!("map_shared_region refused: {e}")),
    };
    if handle_a.host_ptr.is_null() || handle_b.host_ptr.is_null() {
        return Err("virtio-shm handle carries a null host_ptr".into());
    }
    match driver.terminate().await {
        Ok(()) => {}
        Err(e) => return Err(format!("terminate refused: {e}")),
    }
    if maps_cover(handle_a.host_ptr as usize, handle_a.size_bytes)? {
        return Err("virtio-shm region A still mapped after session teardown".into());
    }
    if maps_cover(handle_b.host_ptr as usize, handle_b.size_bytes)? {
        return Err("virtio-shm region B still mapped after session teardown".into());
    }
    if fd_open(handle_a.memfd_raw) || fd_open(handle_b.memfd_raw) {
        return Err("virtio-shm backing memfd still open after session teardown".into());
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/virtio_rings/vr_08_virtio_wdt_ping_cadence.yaml#case_id
#[tokio::test]
#[ignore]
async fn vr_08_virtio_wdt_ping_cadence() -> Result<(), String> {
    let mut driver = driver_rustvmm::RustVmmDriver::new().map_err(|e| e.to_string())?;
    match driver
        .set_watchdog(
            std::time::Duration::from_secs(5),
            WatchdogAction::Terminate {
                emit_diagnostic: true,
            },
        )
        .await
    {
        Ok(()) => {}
        Err(e) => return Err(format!("set_watchdog refused: {e}")),
    }
    Ok(())
}
