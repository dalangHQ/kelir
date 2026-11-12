// see: cans/plugins/profile.md#Session-compute-schema-(profile)
// see: cans/15-operations.md#Multi-language-parity-gate
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use kelir::SandboxProfile;

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

// see: cans/15-operations.md#Multi-language-parity-gate
// see: cans/plugins/profile.md#Session-compute-schema-(profile)
fn ts_skip_ws(source: &str, index: &mut usize) {
    let bytes = source.as_bytes();
    while *index < bytes.len() && matches!(bytes[*index], b' ' | b'\n' | b'\t' | b'\r') {
        *index += 1;
    }
}

fn ts_skip_value(source: &str, index: &mut usize) {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    while *index < bytes.len() {
        match bytes[*index] {
            b'(' | b'{' | b'[' => depth += 1,
            b')' | b'}' | b']' => {
                if depth == 0 {
                    return;
                }
                depth -= 1;
            }
            b',' if depth == 0 => return,
            _ => {}
        }
        *index += 1;
    }
}

fn ts_object_paths(source: &str, index: &mut usize, prefix: &str, out: &mut BTreeSet<String>) {
    ts_skip_ws(source, index);
    if source.as_bytes().get(*index) != Some(&b'{') {
        return;
    }
    *index += 1;
    loop {
        ts_skip_ws(source, index);
        match source.as_bytes().get(*index) {
            Some(&b'}') => {
                *index += 1;
                return;
            }
            None => return,
            _ => {}
        }
        let key_start = *index;
        while matches!(
            source.as_bytes().get(*index),
            Some(c) if c.is_ascii_alphanumeric() || *c == b'_'
        ) {
            *index += 1;
        }
        if *index == key_start {
            return;
        }
        let key = source[key_start..*index].to_string();
        ts_skip_ws(source, index);
        if source.as_bytes().get(*index) != Some(&b':') {
            return;
        }
        *index += 1;
        ts_skip_ws(source, index);
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        let mut probe = *index;
        loop {
            ts_skip_ws(source, &mut probe);
            if source[probe..].starts_with("Type.Optional(") {
                probe += "Type.Optional(".len();
            } else {
                break;
            }
        }
        if source[probe..].starts_with("Type.Object(") {
            *index = probe + "Type.Object(".len();
            ts_object_paths(source, index, &path, out);
            loop {
                ts_skip_ws(source, index);
                match source.as_bytes().get(*index) {
                    Some(&b')') | Some(&b',') => *index += 1,
                    _ => break,
                }
            }
            continue;
        }
        out.insert(path);
        ts_skip_value(source, index);
        ts_skip_ws(source, index);
        match source.as_bytes().get(*index) {
            Some(&b',') => *index += 1,
            Some(&b'}') => {
                *index += 1;
                return;
            }
            _ => return,
        }
    }
}

// see: cans/15-operations.md#Multi-language-parity-gate
fn ts_schema_paths(source: &str) -> BTreeSet<String> {
    let anchor = "const ProfileSchema = Type.Object(";
    let Some(start) = source.find(anchor) else {
        return BTreeSet::new();
    };
    let mut index = start + anchor.len();
    let mut paths = BTreeSet::new();
    ts_object_paths(source, &mut index, "", &mut paths);
    paths
}

// see: cans/15-operations.md#Multi-language-parity-gate
#[allow(clippy::collapsible_if)]
fn py_model_fields(source: &str) -> BTreeMap<String, Vec<(String, String)>> {
    let mut classes: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in source.lines() {
        if let Some(rest) = line.strip_prefix("class ") {
            let name = rest.split('(').next().unwrap_or(rest).to_string();
            classes.entry(name.clone()).or_default();
            current = Some(name);
        } else if line.starts_with(' ') && !line.trim().is_empty() {
            if let Some(current) = &current {
                if let Some((field, annotation)) = line.trim().split_once(':') {
                    let field = field.trim();
                    let annotation = annotation.trim();
                    let valid = !field.is_empty()
                        && field
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
                        && !annotation.is_empty();
                    if valid {
                        classes
                            .entry(current.clone())
                            .or_default()
                            .push((field.to_string(), annotation.to_string()));
                    }
                }
            }
        } else if !line.trim().is_empty() {
            current = None;
        }
    }
    classes
}

fn py_walk(
    class: &str,
    prefix: &str,
    classes: &BTreeMap<String, Vec<(String, String)>>,
    out: &mut BTreeSet<String>,
) {
    let Some(fields) = classes.get(class) else {
        return;
    };
    for (field, annotation) in fields {
        let path = if prefix.is_empty() {
            field.clone()
        } else {
            format!("{prefix}.{field}")
        };
        let head = annotation.split(['|', '[', '<', ' ']).next().unwrap_or("");
        if classes.contains_key(head) {
            py_walk(head, &path, classes, out);
        } else {
            out.insert(path);
        }
    }
}

// see: cans/15-operations.md#Multi-language-parity-gate
fn py_schema_paths(source: &str) -> BTreeSet<String> {
    let classes = py_model_fields(source);
    let mut paths = BTreeSet::new();
    py_walk("ProfileConfig", "", &classes, &mut paths);
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

// see: cans/plugins/profile.md#Inversion-of-Control-configuration-router
const PASS_THROUGH_SECTIONS: [&str; 9] = [
    "invariants",
    "driver",
    "storage",
    "wire",
    "terminal",
    "checkpoint",
    "host",
    "plugins",
    "attestation",
];

fn yaml_root_keys(text: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let root: serde_yaml::Value = serde_yaml::from_str(text)?;
    let serde_yaml::Value::Mapping(map) = root else {
        return Err("profile root is not a mapping".into());
    };
    let mut keys = Vec::new();
    for (key, _) in map {
        if let Some(name) = key.as_str() {
            keys.push(name.to_string());
        }
    }
    Ok(keys)
}

#[tokio::test]
async fn sandbox_profile_round_trips_every_kelir_yaml_section()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = repo_path("../../cans/artifacts/examples/fixtures/kelir.yaml");
    let out = std::env::temp_dir().join(format!("kelir-b8-roundtrip-{}.yaml", std::process::id()));
    let sandbox = kelir::sandbox_runtime::Sandbox::from_config(&fixture).await?;
    sandbox.export_spec(&out).await?;
    let original = fs::read_to_string(&fixture)?;
    let exported = fs::read_to_string(&out)?;
    let first: SandboxProfile = serde_yaml::from_str(&original)?;
    let second: SandboxProfile = serde_yaml::from_str(&exported)?;
    let once = serde_yaml::to_string(&first)?;
    let twice = serde_yaml::to_string(&second)?;
    assert_eq!(
        once, twice,
        "export_spec lost or reordered profile data across the kelir.yaml round-trip"
    );
    let fixture_keys = yaml_root_keys(&original)?;
    let exported_keys = yaml_root_keys(&exported)?;
    assert_eq!(
        fixture_keys, exported_keys,
        "export_spec dropped or reordered top-level sections of the kelir.yaml fixture"
    );
    assert_eq!(
        fixture_keys.len(),
        11,
        "fixture grew new top-level sections"
    );
    let _ = fs::remove_file(&out);
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
    for section in PASS_THROUGH_SECTIONS {
        expected.insert(section.to_string());
    }
    let ts = fs::read_to_string(repo_path("../../packages/ts/core/src/profile.schema.ts"))?;
    assert_eq!(
        ts_schema_paths(&ts),
        expected,
        "TS ProfileSchema drifted from the SSOT session-compute field list"
    );
    let py = fs::read_to_string(repo_path("../../packages/py/kelir/profile_config.py"))?;
    assert_eq!(
        py_schema_paths(&py),
        expected,
        "Py ProfileConfig drifted from the SSOT session-compute field list"
    );
    Ok(())
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[tokio::test]
async fn sandbox_profile_parses_kelir_toml_session_budget() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = repo_path("../../cans/artifacts/examples/fixtures/kelir.toml");
    let sandbox = kelir::sandbox_runtime::Sandbox::from_config(&fixture).await?;
    let Some(budget) = sandbox
        .profile()
        .session
        .as_ref()
        .and_then(|session| session.budget.as_ref())
    else {
        return Err("kelir.toml [session.budget] did not parse".into());
    };
    assert_eq!(budget.exec_seconds, Some(300));
    Ok(())
}
