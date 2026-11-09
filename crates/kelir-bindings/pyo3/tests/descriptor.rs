use std::path::PathBuf;
use std::process::Command;

fn ensure_cdylib() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("failed to locate test binary: {e}"))?;
    let deps = exe
        .parent()
        .ok_or_else(|| "test binary has no parent dir".to_string())?;
    let profile = deps
        .parent()
        .ok_or_else(|| "test binary has no profile dir".to_string())?;
    let in_deps = profile.join("deps/libkelir_pyo3.so");
    if in_deps.exists() {
        return Ok(in_deps);
    }
    let top = profile.join("libkelir_pyo3.so");
    if top.exists() {
        return Ok(top);
    }
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut cmd = Command::new(cargo);
    cmd.arg("build").arg("-p").arg("kelir-pyo3");
    if profile.file_name().is_some_and(|n| n == "release") {
        cmd.arg("--release");
    }
    let output = cmd
        .output()
        .map_err(|e| format!("failed to spawn cargo build: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo build -p kelir-pyo3 failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    if top.exists() {
        return Ok(top);
    }
    if in_deps.exists() {
        return Ok(in_deps);
    }
    Err("built extension module missing after cargo build".to_string())
}

fn run_python(script: &str, workdir: &std::path::Path) -> Result<std::process::Output, String> {
    Command::new("python3")
        .arg("-c")
        .arg(script)
        .current_dir(workdir)
        .env("PYTHONPATH", workdir)
        .output()
        .map_err(|e| format!("failed to spawn python3: {e}"))
}

// see: cans/artifacts/test-plan/cases/bindings_py/bp_04_pyo3_descriptor_without_rings_fails_closed.yaml#case_id
#[test]
fn bp_04_pyo3_descriptor_without_rings_fails_closed() -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("kelir-bp04-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create temp dir: {e}"))?;
    let source = ensure_cdylib()?;
    std::fs::copy(&source, dir.join("_kelir.abi3.so"))
        .map_err(|e| format!("failed to stage extension module: {e}"))?;
    let script = r#"
import _kelir
builder = _kelir.PyKelirBuilder()
try:
    builder.fuse_hooks({})
    raise SystemExit("fuse_hooks({}) must fail")
except ValueError as e:
    assert "declares no `rings` list" in str(e), str(e)
try:
    builder.fuse_hooks({"rings": [{"ring": "bogus"}]})
    raise SystemExit("fuse_hooks(bogus ring) must fail")
except ValueError as e:
    assert "unknown ring" in str(e), str(e)
builder.fuse_hooks({"rings": [{"ring": "trap", "hooks": [], "hook_count": 0}]})
assert builder.trampoline_entry_count() == 1
print("bp_04 ok")
"#;
    let output = run_python(script, &dir)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    std::fs::remove_dir_all(&dir).map_err(|e| format!("failed to remove temp dir: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "python3 descriptor check failed, stdout: {stdout} stderr: {stderr}"
        ));
    }
    if !stdout.contains("bp_04 ok") {
        return Err(format!(
            "python3 descriptor check printed no success marker, stdout: {stdout}"
        ));
    }
    Ok(())
}
