// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/03-sdk-architecture.md#Python-SDK
// see: cans/15-operations.md#Multi-language-parity-gate
mod fusion;
mod merkle;
mod model;
mod naming;
mod otlp;
mod py;
mod sdk;
mod sdk_py;
mod sdk_ts;
mod ts;
mod wire;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use model::parse_model;

// see: cans/15-operations.md#Multi-language-parity-gate
fn write_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    if fs::read_to_string(path).is_ok_and(|old| old == content) {
        return Ok(false);
    }
    fs::write(path, content).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(true)
}

// see: cans/15-operations.md#Multi-language-parity-gate
fn write_all(writes: &[(PathBuf, String)], root: &Path) -> Vec<String> {
    let mut changed = Vec::new();
    for (path, content) in writes {
        match write_if_changed(path, content) {
            Ok(true) => changed.push(
                path.strip_prefix(root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string(),
            ),
            Ok(false) => {}
            Err(e) => die(&e),
        }
    }
    changed
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn check_py_modules(root: &Path, expected: &HashSet<String>) -> Result<(), String> {
    let dir = root.join("packages/py/kelir/plugins");
    let entries = fs::read_dir(&dir).map_err(|e| format!("cannot list {}: {e}", dir.display()))?;
    let mut names: Vec<String> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot list {}: {e}", dir.display()))?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "py") {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    names.sort();
    for name in names {
        let stem = name.strip_suffix(".py").unwrap_or(&name);
        if stem != "__init__" && !expected.contains(stem) {
            return Err(format!("unexpected python plugin module: {name}"));
        }
    }
    Ok(())
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
    let fusion_model =
        fusion::parse_fusion(&interface.join("fusion.rs")).unwrap_or_else(|e| die(&e));
    let ts_fusion = fusion::emit_typescript(&fusion_model);
    let py_fusion = fusion::emit_python(&fusion_model);
    let projections = sdk::load_projections(&root).unwrap_or_else(|e| die(&e));
    let ts_plugins = sdk_ts::emit_ts(&projections).unwrap_or_else(|e| die(&e));

    let writes: Vec<(PathBuf, String)> = vec![
        (
            root.join("packages/ts/core/src/telemetry.attest.generated.ts"),
            ts_telemetry,
        ),
        (
            root.join("packages/ts/core/src/generated/protocol.ts"),
            ts::emit_protocol(),
        ),
        (
            root.join("packages/py/kelir/generated/telemetry.py"),
            py_telemetry,
        ),
        (
            root.join("packages/py/kelir/generated/protocol.py"),
            py::emit_protocol(),
        ),
        (
            root.join("packages/py/kelir/generated/__init__.py"),
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
        (
            root.join("packages/ts/core/src/generated/fusion-tables.ts"),
            ts_fusion,
        ),
        (
            root.join("packages/py/kelir/generated/fusion_tables.py"),
            py_fusion,
        ),
        (root.join("packages/ts/core/src/plugins.ts"), ts_plugins),
    ];
    let changed = write_all(&writes, &root);
    let mut sdk_written: Vec<String> = changed
        .iter()
        .filter(|rel| rel.ends_with("plugins.ts"))
        .cloned()
        .collect();
    let fusion_written: Vec<PathBuf> = changed
        .iter()
        .filter(|rel| rel.ends_with("fusion-tables.ts") || rel.ends_with("fusion_tables.py"))
        .map(|rel| root.join(rel))
        .collect();
    let expected_modules: HashSet<String> = projections
        .iter()
        .map(|proj| proj.krate.module_name.clone())
        .collect();
    check_py_modules(&root, &expected_modules).unwrap_or_else(|e| die(&e));
    let mut module_writes: Vec<(PathBuf, String)> = Vec::new();
    for proj in &projections {
        let content = sdk_py::emit_py_module(proj).unwrap_or_else(|e| die(&e));
        module_writes.push((
            root.join(format!(
                "packages/py/kelir/plugins/{}.py",
                proj.krate.module_name
            )),
            content,
        ));
    }
    module_writes.push((
        root.join("packages/py/kelir/plugins/__init__.py"),
        sdk_py::emit_py_barrel(&projections),
    ));
    sdk_written.extend(write_all(&module_writes, &root));
    for rel in &sdk_written {
        println!("wrote {rel}");
    }
    println!("plugin sdk projections: {} plugins ok", projections.len());
    for path in &fusion_written {
        println!("gen-fusion-tables: wrote {}", path.display());
    }
    ExitCode::SUCCESS
}
