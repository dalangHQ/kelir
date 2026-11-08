// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/03-sdk-architecture.md#Python-SDK
// see: cans/15-operations.md#Multi-language-parity-gate
mod merkle;
mod model;
mod naming;
mod otlp;
mod py;
mod ts;
mod wire;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use model::parse_model;

// see: cans/15-operations.md#Multi-language-parity-gate
fn write_if_changed(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    if fs::read_to_string(path).is_ok_and(|old| old == content) {
        return Ok(());
    }
    fs::write(path, content).map_err(|e| format!("write {}: {e}", path.display()))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .to_path_buf()
}

fn die(message: &str) -> ! {
    eprintln!("codegen: {message}");
    std::process::exit(1)
}

fn main() -> ExitCode {
    let root = repo_root();
    let interface = root.join("crates/kelir-interface/src");
    let mut model = parse_model(&interface.join("driver.rs")).unwrap_or_else(|e| die(&e));
    for path in [
        interface.join("telemetry/receipt.rs"),
        interface.join("telemetry/delta.rs"),
        root.join("crates/kelir-plugins/plugin-ledger/src/wal.rs"),
    ] {
        let parsed = parse_model(&path).unwrap_or_else(|e| die(&e));
        model.structs.extend(parsed.structs);
        model.enums.extend(parsed.enums);
    }

    const ORDER: &[&str] = &[
        "GuestClock",
        "SnapshotDescriptor",
        "TelemetryExtension",
        "PhaseTimings",
        "ExitKind",
        "ExecutionReceipt",
        "CausalDelta",
    ];
    const NATIVE_SKIP: &[&str] = &["TelemetryExtension"];

    let ts_telemetry = ts::emit_telemetry(&model, ORDER, NATIVE_SKIP).unwrap_or_else(|e| die(&e));
    let py_telemetry = py::emit_telemetry(&model, ORDER).unwrap_or_else(|e| die(&e));
    let otlp_records = otlp::emit_records(&model).unwrap_or_else(|e| die(&e));
    let domain_merkle = merkle::emit_domain_leaves(&model).unwrap_or_else(|e| die(&e));
    let packed_merkle = merkle::emit_packed_records(&model).unwrap_or_else(|e| die(&e));

    let writes: [(PathBuf, String); 9] = [
        (
            root.join("packages/ts/core/src/generated/telemetry.ts"),
            ts_telemetry,
        ),
        (
            root.join("packages/ts/core/src/generated/protocol.ts"),
            ts::emit_protocol(),
        ),
        (
            root.join("packages/py/kelir/_generated/telemetry.py"),
            py_telemetry,
        ),
        (
            root.join("packages/py/kelir/_generated/protocol.py"),
            py::emit_protocol(),
        ),
        (
            root.join("packages/py/kelir/_generated/__init__.py"),
            py::emit_package_init(ORDER),
        ),
        (
            root.join("crates/kelir-shim/include/kelir/wire.h"),
            wire::emit_wire_h(),
        ),
        (root.join("crates/kelir-otlp/src/records.rs"), otlp_records),
        (
            root.join("crates/kelir-interface/src/telemetry/merkle.rs"),
            domain_merkle,
        ),
        (
            root.join("crates/kelir-plugins/plugin-ledger/src/wal/merkle.rs"),
            packed_merkle,
        ),
    ];
    for (path, content) in &writes {
        write_if_changed(path, content).unwrap_or_else(|e| die(&e));
    }
    ExitCode::SUCCESS
}
