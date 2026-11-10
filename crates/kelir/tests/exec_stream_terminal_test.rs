// see: cans/artifacts/test-plan/cases/terminal_plane/tp_09_exec_stream_sublane_stdio_fidelity.yaml#case_id
use std::path::{Path, PathBuf};

use kelir::KelirBuilder;
use kelir::interface::driver_interface::default_vm_config;
use kelir::interface::runtime_block::block_on;

fn session_logs_dir(session_id: &[u8; 16]) -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    let mut hex = String::with_capacity(32);
    for byte in session_id {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok(PathBuf::from(home)
        .join(".kelir")
        .join("sessions")
        .join(hex)
        .join("logs"))
}

fn drop_session_dir(session_id: &[u8; 16]) {
    if let Ok(dir) = session_logs_dir(session_id) {
        let _ = std::fs::remove_dir_all(dir.parent().map(Path::to_path_buf).unwrap_or_default());
    }
}

// see: cans/artifacts/test-plan/cases/terminal_plane/tp_09_exec_stream_sublane_stdio_fidelity.yaml#case_id
#[test]
#[ignore]
fn tp_09_exec_stream_sublane_stdio_fidelity() -> Result<(), String> {
    let config = default_vm_config(
        None,
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    );
    let session = block_on(KelirBuilder::new(config).boot()).map_err(|e| e.to_string())?;
    let session_id = session.session_id();
    let spawned = block_on(session.spawn("/bin/emit_raw_bytes"));
    let finished = match spawned {
        Ok(exec) => exec,
        Err(e) => {
            drop_session_dir(&session_id);
            return Err(format!(
                "guest exec must dispatch on the exec stream, got {e:?}"
            ));
        }
    };
    if finished.exit_code != 0 {
        drop_session_dir(&session_id);
        return Err(format!(
            "emit_raw_bytes must exit 0, got {}",
            finished.exit_code
        ));
    }
    let logs = session_logs_dir(&session_id)?;
    let expected: [u8; 7] = [0x00, 0xFF, 0x0A, 0x1B, 0x5B, 0x30, 0x6D];
    let mut found = false;
    let entries = std::fs::read_dir(&logs).map_err(|e| format!("read {}: {e}", logs.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name_matches = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("exec-") && name.ends_with(".log"));
        if !name_matches {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        if bytes == expected {
            found = true;
        }
    }
    drop_session_dir(&session_id);
    if !found {
        return Err(
            "no exec stream log carries the raw bytes [00 FF 0A 1B 5B 30 6D] with bitwise fidelity"
                .into(),
        );
    }
    Ok(())
}
