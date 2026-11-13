// see: cans/artifacts/test-plan/cases/terminal_plane/tp_11_stream_replay_monotonic_cursor_read.yaml#case_id
use std::path::PathBuf;
use std::sync::Arc;

use kelir_interface::terminal::{LogStream, TerminalOps, TerminalWiring};
use kelir_otlp::registry_otlp::MetricRegistry;
use kelir_terminal::terminal_ops;

fn ops(session_id: [u8; 16]) -> Arc<dyn TerminalOps> {
    let ops = terminal_ops(Arc::new(MetricRegistry::new()));
    ops.bind_session(session_id);
    ops
}

fn session_dir(session_id: &[u8; 16]) -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    let mut hex = String::with_capacity(32);
    for byte in session_id {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok(PathBuf::from(home)
        .join(".kelir")
        .join("sessions")
        .join(hex))
}

fn drop_session_dir(session_id: &[u8; 16]) {
    if let Ok(dir) = session_dir(session_id) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

// see: cans/artifacts/test-plan/cases/terminal_plane/tp_07_vt100_wireframe_matrix_determinism.yaml#case_id
#[test]
fn tp_07_vt100_wireframe_matrix_determinism() -> Result<(), String> {
    let first = ops([0x70u8; 16]);
    let second = ops([0x70u8; 16]);
    first.resize(80, 24).map_err(|e| e.to_string())?;
    second.resize(80, 24).map_err(|e| e.to_string())?;
    let first_matrix = first.export_screen_matrix().map_err(|e| e.to_string())?;
    let second_matrix = second.export_screen_matrix().map_err(|e| e.to_string())?;
    assert_eq!(first_matrix, second_matrix);
    assert_eq!(first_matrix.len(), 1944);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/terminal_plane/tp_11_stream_replay_monotonic_cursor_read.yaml#case_id
#[test]
fn tp_11_stream_replay_monotonic_cursor_read() -> Result<(), String> {
    let session_id = [0x71u8; 16];
    drop_session_dir(&session_id);
    let plane = ops(session_id);
    let head = plane
        .append_stream_chunk(LogStream::Port(0), b"hello")
        .map_err(|e| e.to_string())?;
    assert_eq!(head, 5);
    let first = plane.read_from(0, 0).map_err(|e| e.to_string())?;
    let second = plane.read_from(0, 0).map_err(|e| e.to_string())?;
    assert_eq!(first.0, b"hello");
    assert_eq!(first.1, 5);
    assert_eq!(second.0, first.0);
    assert_eq!(second.1, first.1);
    drop_session_dir(&session_id);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/terminal_plane/tp_12_stream_replay_client_reconnect_resume.yaml#case_id
#[test]
fn tp_12_stream_replay_client_reconnect_resume() -> Result<(), String> {
    let session_id = [0x72u8; 16];
    drop_session_dir(&session_id);
    let plane = ops(session_id);
    let chunk = [b'x'; 1024];
    let head = plane
        .append_stream_chunk(LogStream::Port(0), &chunk)
        .map_err(|e| e.to_string())?;
    assert_eq!(head, 1024);
    let tail = plane.read_from(0, 512).map_err(|e| e.to_string())?;
    assert_eq!(tail.0.len(), 512);
    assert_eq!(tail.1, 1024);
    let whole = plane.read_from(0, 0).map_err(|e| e.to_string())?;
    assert_eq!(whole.0.len(), 1024);
    assert_eq!(whole.1, 1024);
    assert_eq!(&whole.0[512..], &tail.0[..]);
    drop_session_dir(&session_id);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/terminal_plane/tp_13_exec_log_file_per_exec_isolation.yaml#case_id
#[test]
fn tp_13_exec_log_file_per_exec_isolation() -> Result<(), String> {
    let session_id = [0x73u8; 16];
    drop_session_dir(&session_id);
    let plane = ops(session_id);
    let head = plane
        .append_stream_chunk(LogStream::Port(0), b"0123456789")
        .map_err(|e| e.to_string())?;
    assert_eq!(head, 10);
    plane
        .append_stream_chunk(LogStream::Exec { id: 9, lane: 1 }, b"out")
        .map_err(|e| e.to_string())?;
    plane
        .append_stream_chunk(LogStream::Exec { id: 9, lane: 2 }, b"stderr")
        .map_err(|e| e.to_string())?;
    let logs = session_dir(&session_id)?.join("logs");
    let port_log = logs.join("port-0.log");
    let stdout_log = logs.join("exec-9-1.log");
    let stderr_log = logs.join("exec-9-2.log");
    assert_eq!(
        std::fs::metadata(&port_log)
            .map_err(|e| e.to_string())?
            .len(),
        10
    );
    assert_eq!(
        std::fs::metadata(&stdout_log)
            .map_err(|e| e.to_string())?
            .len(),
        3
    );
    assert_eq!(
        std::fs::metadata(&stderr_log)
            .map_err(|e| e.to_string())?
            .len(),
        6
    );
    drop_session_dir(&session_id);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/terminal_plane/tp_14_window_resize_exec_control_propagation.yaml#case_id
#[test]
fn tp_14_window_resize_exec_control_propagation() -> Result<(), String> {
    let plane = ops([0x74u8; 16]);
    plane.resize(120, 40).map_err(|e| e.to_string())?;
    let matrix = plane.export_screen_matrix().map_err(|e| e.to_string())?;
    assert_eq!(matrix.len(), 4840);
    plane.resize(80, 24).map_err(|e| e.to_string())?;
    let small = plane.export_screen_matrix().map_err(|e| e.to_string())?;
    assert_eq!(small.len(), 1944);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/terminal_plane/tp_03_ansi_sanitizer_osc52_clipboard_drop.yaml#case_id
#[test]
#[ignore]
fn tp_03_ansi_sanitizer_osc52_clipboard_drop() -> Result<(), String> {
    let session_id = [0x75u8; 16];
    drop_session_dir(&session_id);
    let wiring = TerminalWiring {
        cols: 80,
        rows: 24,
        drop_osc52: true,
        strip_title: false,
        token_shield: None,
        prompts: Vec::new(),
    };
    let plane = ops(session_id);
    plane.apply_wiring(&wiring).map_err(|e| e.to_string())?;
    plane
        .record_output(
            LogStream::Port(0),
            b"echo '\x1b]52;c;ZXhwbG9pdA==\x07Normal Text'",
        )
        .map_err(|e| e.to_string())?;
    let (bytes, _) = plane.read_from(0, 0).map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    assert!(
        !text.contains("\x1b]52;"),
        "OSC 52 clipboard payload must be purged before the log, got: {text:?}"
    );
    assert!(
        text.contains("Normal Text"),
        "the non-OSC payload must survive the strip, got: {text:?}"
    );
    drop_session_dir(&session_id);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/terminal_plane/tp_04_ansi_sanitizer_title_spoof_strip.yaml#case_id
#[test]
#[ignore]
fn tp_04_ansi_sanitizer_title_spoof_strip() -> Result<(), String> {
    let session_id = [0x76u8; 16];
    drop_session_dir(&session_id);
    let wiring = TerminalWiring {
        cols: 80,
        rows: 24,
        drop_osc52: false,
        strip_title: true,
        token_shield: None,
        prompts: Vec::new(),
    };
    let plane = ops(session_id);
    plane.apply_wiring(&wiring).map_err(|e| e.to_string())?;
    plane
        .record_output(
            LogStream::Port(0),
            b"\x1b]0;root@host:~\x07Payload\x1b]2;fake_title\x1b\\Done",
        )
        .map_err(|e| e.to_string())?;
    let (bytes, _) = plane.read_from(0, 0).map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    assert!(
        !text.contains("\x1b]0;") && !text.contains("\x1b]2;"),
        "OSC 0/2 title sequences (BEL and ST terminated) must be stripped, got: {text:?}"
    );
    assert!(
        text.contains("Payload") && text.contains("Done"),
        "the payload around the title sequences must survive, got: {text:?}"
    );
    drop_session_dir(&session_id);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/terminal_plane/tp_06_placeholder_mask_carryover_window.yaml#case_id
#[test]
#[ignore]
fn tp_06_placeholder_mask_carryover_window() -> Result<(), String> {
    let session_id = [0x77u8; 16];
    drop_session_dir(&session_id);
    let wiring = TerminalWiring {
        cols: 80,
        rows: 24,
        drop_osc52: false,
        strip_title: false,
        token_shield: None,
        prompts: Vec::new(),
    };
    let plane = ops(session_id);
    plane.apply_wiring(&wiring).map_err(|e| e.to_string())?;
    plane
        .record_output(LogStream::Port(0), b"API key is sk-an")
        .map_err(|e| e.to_string())?;
    plane
        .record_output(LogStream::Port(0), b"t-12345 confirmed")
        .map_err(|e| e.to_string())?;
    let (bytes, _) = plane.read_from(0, 0).map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    assert!(
        !text.contains("sk-ant-12345"),
        "secret split across chunk boundaries must be redacted, got: {text:?}"
    );
    assert!(
        text.contains("[REDACTED]"),
        "the split secret must surface as [REDACTED], got: {text:?}"
    );
    drop_session_dir(&session_id);
    Ok(())
}
