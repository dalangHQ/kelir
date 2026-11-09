use std::path::{Path, PathBuf};
use std::process::Command;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_kelir-plugin-cli"))
}

fn run_cli(args: &[&str]) -> Result<std::process::Output, String> {
    Command::new(cli_bin())
        .args(args)
        .output()
        .map_err(|e| format!("failed to spawn kelir-plugin-cli: {e}"))
}

fn write_fixtures_toml(dir: &Path, kernel_path: &Path, digest: &str) -> Result<PathBuf, String> {
    let fixtures_path = dir.join("fixtures.toml");
    let text = format!(
        "[fixtures.kernel]\npath = \"{}\"\nblake3 = \"{digest}\"\n",
        kernel_path.display()
    );
    std::fs::write(&fixtures_path, text)
        .map_err(|e| format!("failed to write fixtures.toml: {e}"))?;
    Ok(fixtures_path)
}

// see: cans/artifacts/test-plan/cases/governance_tools/gt_13_cli_doctor_reports_missing_kvm.yaml#case_id
#[test]
fn gt_13_cli_doctor_reports_missing_kvm() -> Result<(), String> {
    let output = run_cli(&["doctor", "--json"])?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if Path::new("/dev/kvm").exists() {
        if !stdout.contains("\"probe\":\"kvm\",\"status\":\"pass\"") {
            return Err(format!(
                "doctor --json with /dev/kvm present must report kvm pass, got stdout: {stdout}"
            ));
        }
        return Ok(());
    }
    if output.status.success() {
        return Err(format!(
            "doctor --json must fail without /dev/kvm, got stdout: {stdout}"
        ));
    }
    if !stdout.contains("\"probe\":\"kvm\",\"status\":\"fail\"") {
        return Err(format!(
            "doctor --json must carry the kvm fail probe, got stdout: {stdout}"
        ));
    }
    if !stdout.contains("\"detail\":\"/dev/kvm is absent\"") {
        return Err(format!(
            "doctor --json kvm probe must detail the absent device, got stdout: {stdout}"
        ));
    }
    if !stderr.contains("kvm: /dev/kvm is absent") {
        return Err(format!(
            "doctor failure must name the absent device, got stderr: {stderr}"
        ));
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/governance_tools/gt_14_cli_fixtures_digest_mismatch_fails_closed.yaml#case_id
#[test]
fn gt_14_cli_fixtures_digest_mismatch_fails_closed() -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("kelir-gt14-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create temp dir: {e}"))?;
    let kernel_path = dir.join("kernel");
    std::fs::write(&kernel_path, b"kelir fixture kernel bytes")
        .map_err(|e| format!("failed to write kernel fixture: {e}"))?;
    let wrong_digest = "0000000000000000000000000000000000000000000000000000000000000000";
    let fixtures_path = write_fixtures_toml(&dir, &kernel_path, wrong_digest)?;
    let output = run_cli(&["fixtures", "--verify", &fixtures_path.to_string_lossy()])?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        return Err("fixtures --verify must fail on a digest mismatch".to_string());
    }
    if !stderr.contains("fixture kernel: digest mismatch: expected 0000000000000000000000000000000000000000000000000000000000000000") {
        return Err(format!(
            "fixtures --verify must report the expected digest, got stderr: {stderr}"
        ));
    }
    if !stderr.contains(", got ") {
        return Err(format!(
            "fixtures --verify must report the actual digest, got stderr: {stderr}"
        ));
    }
    std::fs::write(&kernel_path, b"")
        .map_err(|e| format!("failed to empty kernel fixture: {e}"))?;
    let empty_blake3 = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";
    let fixtures_path = write_fixtures_toml(&dir, &kernel_path, empty_blake3)?;
    let output = run_cli(&["fixtures", "--verify", &fixtures_path.to_string_lossy()])?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        return Err(format!(
            "fixtures --verify must pass on an exact digest match, got stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    if !stdout.contains("fixtures: 1 fixture(s) verified") {
        return Err(format!(
            "fixtures --verify must confirm the verified count, got stdout: {stdout}"
        ));
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("failed to remove temp dir: {e}"))?;
    Ok(())
}
