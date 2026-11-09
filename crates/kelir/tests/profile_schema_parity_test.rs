// see: cans/plugins/profile.md#Session-compute-schema-(profile)
// see: cans/15-operations.md#Multi-language-parity-gate
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use kelir::SandboxProfile;

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn quoted_segments(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else {
            break;
        };
        out.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    out
}

// see: cans/15-operations.md#Multi-language-parity-gate
fn ts_profile_paths(source: &str) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    let mut rest = source;
    while let Some(start) = rest.find("path: [") {
        let after = &rest[start + "path: [".len()..];
        let Some(end) = after.find(']') else {
            break;
        };
        let segments = quoted_segments(&after[..end]);
        if !segments.is_empty() {
            paths.insert(segments.join("."));
        }
        rest = &after[end..];
    }
    paths
}

// see: cans/15-operations.md#Multi-language-parity-gate
fn py_profile_paths(source: &str) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    let mut rest = source;
    while let Some(start) = rest.find("ProfileField(") {
        let after = &rest[start + "ProfileField(".len()..];
        let Some(end) = after.find(')') else {
            break;
        };
        let segments = quoted_segments(&after[..end]);
        if !segments.is_empty() {
            paths.insert(segments.join("."));
        }
        rest = &after[end..];
    }
    paths
}

fn flatten_paths(value: &serde_yaml::Value, prefix: &str, out: &mut BTreeSet<String>) {
    if let serde_yaml::Value::Mapping(map) = value {
        for (key, item) in map {
            if let Some(name) = key.as_str() {
                let path = if prefix.is_empty() {
                    name.to_string()
                } else {
                    format!("{prefix}.{name}")
                };
                flatten_paths(item, &path, out);
            }
        }
        return;
    }
    out.insert(prefix.to_string());
}

#[test]
fn ssot_fixture_session_fields_deserialize_into_sandbox_profile()
-> Result<(), Box<dyn std::error::Error>> {
    let yaml = fs::read_to_string(repo_path(
        "../../cans/artifacts/examples/fixtures/kelir.yaml",
    ))?;
    let profile: SandboxProfile = serde_yaml::from_str(&yaml)?;
    assert!(
        profile.version.is_some(),
        "version missing from the SSOT fixture"
    );
    let session = profile
        .session
        .as_ref()
        .ok_or("session missing from the SSOT fixture")?;
    assert!(
        session.id.is_some(),
        "session.id missing from the SSOT fixture"
    );
    assert!(
        session.topology.is_some(),
        "session.topology missing from the SSOT fixture"
    );
    let compute = session
        .compute
        .as_ref()
        .ok_or("session.compute missing from the SSOT fixture")?;
    assert!(
        compute.vcpus.is_some(),
        "session.compute.vcpus missing from the SSOT fixture"
    );
    assert!(
        compute.max_vcpus.is_some(),
        "session.compute.max_vcpus missing from the SSOT fixture"
    );
    assert!(
        compute.memory_mb.is_some(),
        "session.compute.memory_mb missing from the SSOT fixture"
    );
    assert!(
        compute.page_size_kb.is_some(),
        "session.compute.page_size_kb missing from the SSOT fixture"
    );
    let balloon = session
        .balloon
        .as_ref()
        .ok_or("session.balloon missing from the SSOT fixture")?;
    assert!(
        balloon.enabled.is_some(),
        "session.balloon.enabled missing from the SSOT fixture"
    );
    assert!(
        balloon.initial_mb.is_some(),
        "session.balloon.initial_mb missing from the SSOT fixture"
    );
    assert!(
        balloon.pressure_reclaim.is_some(),
        "session.balloon.pressure_reclaim missing from the SSOT fixture"
    );
    assert!(
        balloon.reclaim_strategy.is_some(),
        "session.balloon.reclaim_strategy missing from the SSOT fixture"
    );
    let clock = session
        .clock
        .as_ref()
        .ok_or("session.clock missing from the SSOT fixture")?;
    assert!(
        clock.tsc_locked.is_some(),
        "session.clock.tsc_locked missing from the SSOT fixture"
    );
    assert!(
        clock.deterministic_clock.is_some(),
        "session.clock.deterministic_clock missing from the SSOT fixture"
    );
    assert!(
        clock.pinned_clocksource.is_some(),
        "session.clock.pinned_clocksource missing from the SSOT fixture"
    );
    assert!(
        clock.preemption_grace_ms.is_some(),
        "session.clock.preemption_grace_ms missing from the SSOT fixture"
    );
    let watchdog = session
        .watchdog
        .as_ref()
        .ok_or("session.watchdog missing from the SSOT fixture")?;
    assert!(
        watchdog.enabled.is_some(),
        "session.watchdog.enabled missing from the SSOT fixture"
    );
    assert!(
        watchdog.timeout_seconds.is_some(),
        "session.watchdog.timeout_seconds missing from the SSOT fixture"
    );
    assert!(
        watchdog.action.is_some(),
        "session.watchdog.action missing from the SSOT fixture"
    );
    assert!(
        watchdog.heartbeat_cadence_seconds.is_some(),
        "session.watchdog.heartbeat_cadence_seconds missing from the SSOT fixture"
    );
    let devices = session
        .devices
        .as_ref()
        .ok_or("session.devices missing from the SSOT fixture")?;
    let rng = devices
        .virtio_rng
        .as_ref()
        .ok_or("session.devices.virtio_rng missing from the SSOT fixture")?;
    assert!(
        rng.enabled.is_some(),
        "session.devices.virtio_rng.enabled missing from the SSOT fixture"
    );
    assert!(
        rng.source.is_some(),
        "session.devices.virtio_rng.source missing from the SSOT fixture"
    );
    let shm = devices
        .virtio_shm
        .as_ref()
        .ok_or("session.devices.virtio_shm missing from the SSOT fixture")?;
    assert!(
        shm.enabled.is_some(),
        "session.devices.virtio_shm.enabled missing from the SSOT fixture"
    );
    let region = shm
        .regions
        .as_ref()
        .and_then(|regions| regions.first())
        .ok_or("session.devices.virtio_shm.regions missing from the SSOT fixture")?;
    assert!(
        region.tag.is_some(),
        "session.devices.virtio_shm.regions[].tag missing from the SSOT fixture"
    );
    assert!(
        region.size_bytes.is_some(),
        "session.devices.virtio_shm.regions[].size_bytes missing from the SSOT fixture"
    );
    let artifacts = session
        .artifacts
        .as_ref()
        .ok_or("session.artifacts missing from the SSOT fixture")?;
    assert!(
        artifacts.kernel_path.is_some(),
        "session.artifacts.kernel_path missing from the SSOT fixture"
    );
    assert!(
        artifacts.rootfs_erofs_path.is_some(),
        "session.artifacts.rootfs_erofs_path missing from the SSOT fixture"
    );
    assert!(
        artifacts
            .kernel_boot_params
            .as_ref()
            .is_some_and(|params| !params.is_empty()),
        "session.artifacts.kernel_boot_params missing from the SSOT fixture"
    );
    let shim = session
        .shim
        .as_ref()
        .ok_or("session.shim missing from the SSOT fixture")?;
    assert!(
        shim.max_exec_slots.is_some(),
        "session.shim.max_exec_slots missing from the SSOT fixture"
    );
    assert!(
        shim.hide_pid.is_some(),
        "session.shim.hide_pid missing from the SSOT fixture"
    );
    assert!(
        shim.anti_toctou_cgroups.is_some(),
        "session.shim.anti_toctou_cgroups missing from the SSOT fixture"
    );
    Ok(())
}

#[test]
fn ts_and_py_profile_fields_match_the_ssot_fixture() -> Result<(), Box<dyn std::error::Error>> {
    let yaml = fs::read_to_string(repo_path(
        "../../cans/artifacts/examples/fixtures/kelir.yaml",
    ))?;
    let root: serde_yaml::Value = serde_yaml::from_str(&yaml)?;
    let mut expected = BTreeSet::new();
    if let Some(version) = root.get("version") {
        flatten_paths(version, "version", &mut expected);
    }
    if let Some(session) = root.get("session") {
        flatten_paths(session, "session", &mut expected);
    }
    let ts = fs::read_to_string(repo_path("../../packages/ts/core/src/sandbox.runtime.ts"))?;
    assert_eq!(
        ts_profile_paths(&ts),
        expected,
        "TS PROFILE_FIELDS drifted from the SSOT session-compute field list"
    );
    let py = fs::read_to_string(repo_path("../../packages/py/kelir/lifecycle_sandbox.py"))?;
    assert_eq!(
        py_profile_paths(&py),
        expected,
        "Py PROFILE_FIELDS drifted from the SSOT session-compute field list"
    );
    Ok(())
}
