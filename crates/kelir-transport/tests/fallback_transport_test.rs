// see: cans/artifacts/test-plan/cases/protocol_mux/pm_13_vsock_fallback_connect_fails_closed.yaml#case_id
use std::io::ErrorKind;

use kelir_transport::fallback_transport::{VsockAddress, VsockConfig, VsockFallbackTransport};

#[cfg(target_os = "linux")]
fn open_fd_count() -> Result<usize, String> {
    let entries = std::fs::read_dir("/proc/self/fd").map_err(|e| e.to_string())?;
    Ok(entries.count())
}

// see: cans/artifacts/test-plan/cases/protocol_mux/pm_13_vsock_fallback_connect_fails_closed.yaml#case_id
#[test]
#[ignore]
fn pm_13_vsock_fallback_connect_fails_closed() -> Result<(), String> {
    let config = VsockConfig {
        address: VsockAddress {
            cid: 0xFFFF_FFFF,
            port: 5252,
        },
    };
    #[cfg(target_os = "linux")]
    let before = open_fd_count()?;
    match VsockFallbackTransport::connect(config) {
        Err(e) if e.kind() == ErrorKind::ConnectionRefused => {}
        Err(e) => {
            return Err(format!(
                "unreachable vsock connect failed closed with error kind {:?} instead of ConnectionRefused: {e}",
                e.kind()
            ));
        }
        Ok(_) => return Err("unreachable vsock connect succeeded".to_string()),
    }
    #[cfg(target_os = "linux")]
    {
        let after = open_fd_count()?;
        assert_eq!(
            after, before,
            "failed vsock fallback connect leaked a host descriptor"
        );
    }
    Ok(())
}
