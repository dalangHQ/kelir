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

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
fn parse_yaml_fixture() -> Result<SandboxProfile, Box<dyn std::error::Error>> {
    let yaml = fs::read_to_string(repo_path(
        "../../cans/artifacts/examples/fixtures/kelir.yaml",
    ))?;
    Ok(serde_yaml::from_str(&yaml)?)
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
fn parse_toml_fixture() -> Result<SandboxProfile, Box<dyn std::error::Error>> {
    let toml = fs::read_to_string(repo_path(
        "../../cans/artifacts/examples/fixtures/kelir.toml",
    ))?;
    Ok(toml::from_str(&toml)?)
}

// see: cans/plugins/profile.md#Monotonic-limits
#[test]
fn kelir_toml_and_yaml_parse_to_identical_effective_config()
-> Result<(), Box<dyn std::error::Error>> {
    let from_yaml = parse_yaml_fixture()?.effective_config()?;
    let from_toml = parse_toml_fixture()?.effective_config()?;
    assert_eq!(
        from_yaml, from_toml,
        "kelir.toml and kelir.yaml do not parse to the same effective config"
    );
    assert_eq!(
        from_yaml.vm_config.tenant_id.as_deref(),
        Some("org_enterprise_aegis_prod")
    );
    assert_eq!(from_yaml.vm_config.vcpus, 4);
    assert_eq!(from_yaml.vm_config.memory_mb, 4096);
    assert_eq!(from_yaml.driver_policy.ffi_worker_threads, 8);
    assert_eq!(from_yaml.driver_policy.ffi_max_ops_per_sec, 50000);
    assert_eq!(from_yaml.driver_policy.ffi_queue_capacity, 16384);
    assert_eq!(from_yaml.driver_policy.ffi_max_buffer_bytes, 16777216);
    assert_eq!(
        from_yaml.driver_policy.ffi_zero_copy_shm_threshold_bytes,
        65536
    );
    assert!(from_yaml.driver_policy.ffi_allow_async_worker_yield);
    assert!(!from_yaml.driver_policy.require_hardware_vmm);
    assert!(from_yaml.driver_policy.allow_emulated_fallback);
    assert_eq!(
        from_yaml
            .driver_policy
            .emulated_tier_classification
            .as_deref(),
        Some("Class2_RootlessUserNS")
    );
    Ok(())
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[test]
fn ws_d1_every_new_key_maps_to_an_effective_config_field() -> Result<(), Box<dyn std::error::Error>>
{
    let effective = parse_yaml_fixture()?.effective_config()?;
    let envelope = effective
        .isolation_envelope
        .as_ref()
        .ok_or("session.isolation_envelope missing")?;
    assert_eq!(envelope.enforce_openat2_beneath, Some(true));
    assert_eq!(
        envelope.cgroup_parent.as_deref(),
        Some("aegis/org_enterprise_aegis_prod")
    );
    assert_eq!(effective.granular_task_freeze, Some(true));
    assert!(matches!(
        effective.rejection_mode,
        Some(kelir::sandbox_runtime::RejectionMode::SyntheticRefusal)
    ));
    let outbox = effective
        .outbox_deferral
        .as_ref()
        .ok_or("wire.outbox_deferral missing")?;
    assert_eq!(outbox.enabled, Some(true));
    assert_eq!(outbox.buffer_capacity_bytes, Some(67108864));
    assert_eq!(outbox.release_on_step_verification, Some(true));
    assert_eq!(outbox.drop_on_rollback, Some(true));
    assert_eq!(outbox.compensator_webhook_timeout_ms, Some(5000));
    assert_eq!(effective.l7_protocol_inspection, Some(true));
    let dag = effective
        .step_dag
        .as_ref()
        .ok_or("checkpoint.step_dag missing")?;
    assert_eq!(dag.enabled, Some(true));
    assert_eq!(dag.require_signed_attribution, Some(true));
    assert_eq!(dag.require_separation_of_duties, Some(true));
    assert!(matches!(
        dag.storage,
        Some(kelir::sandbox_runtime::StepDagStorage::ContentAddressed)
    ));
    let catalog = effective.catalog.as_ref().ok_or("host.catalog missing")?;
    assert!(matches!(
        catalog.backend,
        Some(kelir_catalog::CatalogBackend::Postgres)
    ));
    assert_eq!(
        catalog.endpoint.as_deref(),
        Some("postgres://kelir_admin@pg.internal.net:5432/kelir_catalog")
    );
    assert_eq!(catalog.schema_tenant_partitioning, Some(true));
    assert_eq!(
        catalog.fallback_sqlite_path.as_deref(),
        Some("~/.kelir/catalog.db")
    );
    let ring = effective
        .telemetry_ring
        .as_ref()
        .ok_or("host.telemetry_ring missing")?;
    assert_eq!(ring.feature_extraction, Some(true));
    assert_eq!(ring.sliding_window_depth, Some(300));
    assert_eq!(ring.export_feature_vector, Some(true));
    let wal = effective
        .ledger_merkle_wal
        .as_ref()
        .ok_or("plugins.ledger.merkle_wal missing")?;
    assert!(matches!(
        wal.hashing_standard,
        Some(kelir::sandbox_runtime::HashingStandard::Rfc6962DualBlake3Sha256)
    ));
    assert_eq!(wal.record_pre_redaction_hash, Some(true));
    assert_eq!(wal.include_hook_spans_in_root, Some(true));
    assert_eq!(
        wal.witness_cosigning_endpoint.as_deref(),
        Some("https://witness.internal.net/v1/sth")
    );
    let otel = effective
        .ledger_otel
        .as_ref()
        .ok_or("plugins.ledger.otel missing")?;
    assert_eq!(
        otel.endpoint.as_deref(),
        Some("http://localhost:4318/v1/traces")
    );
    let guard = effective
        .tool_guard_interposition
        .as_ref()
        .ok_or("plugins.tool_guard.mcp_interposition missing")?;
    assert_eq!(guard.max_frame_bytes, Some(16777216));
    assert_eq!(guard.detect_semantic_rug_pull, Some(true));
    assert_eq!(guard.strict_ast_parameter_validation, Some(true));
    let taint = effective
        .tool_guard_taint
        .as_ref()
        .ok_or("plugins.tool_guard.taint_governance missing")?;
    assert_eq!(taint.propagate_byte_range_taints, Some(true));
    let mcp = effective.mcp.as_ref().ok_or("plugins.mcp missing")?;
    assert_eq!(mcp.max_frame_bytes, Some(16777216));
    let mapped = parse_yaml_fixture()?.to_catalog_config();
    assert_eq!(mapped.backend, kelir_catalog::CatalogBackend::Postgres);
    assert_eq!(
        mapped.endpoint.as_deref(),
        Some("postgres://kelir_admin@pg.internal.net:5432/kelir_catalog")
    );
    assert_eq!(
        mapped.fallback_sqlite_path.as_deref(),
        Some("~/.kelir/catalog.db")
    );
    Ok(())
}

// see: cans/plugins/profile.md#Monotonic-limits
#[test]
fn overlay_widening_a_budget_ceiling_fails_with_e_security_violation()
-> Result<(), Box<dyn std::error::Error>> {
    let base = parse_yaml_fixture()?;
    let mut widened = base.clone();
    widened
        .driver
        .as_mut()
        .ok_or("driver missing")?
        .ffi_boundary
        .as_mut()
        .ok_or("ffi_boundary missing")?
        .max_buffer_bytes = Some(33_554_432);
    let err = match widened.validate_overlay(&base) {
        Ok(()) => return Err("widened max_buffer_bytes passed validation".into()),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("ESecurityViolation"),
        "widening must fail with ESecurityViolation, got {err}"
    );
    let mut dropped_guard = base.clone();
    dropped_guard
        .plugins
        .as_mut()
        .ok_or("plugins missing")?
        .ledger
        .as_mut()
        .ok_or("ledger missing")?
        .merkle_wal
        .as_mut()
        .ok_or("merkle_wal missing")?
        .record_pre_redaction_hash = Some(false);
    let err = match dropped_guard.validate_overlay(&base) {
        Ok(()) => return Err("dropped pre-redaction guard passed validation".into()),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("ESecurityViolation"),
        "dropping a guard must fail with ESecurityViolation, got {err}"
    );
    let mut tightened = base.clone();
    tightened
        .driver
        .as_mut()
        .ok_or("driver missing")?
        .ffi_boundary
        .as_mut()
        .ok_or("ffi_boundary missing")?
        .max_buffer_bytes = Some(8_388_608);
    tightened.validate_overlay(&base)?;
    base.clone().validate_overlay(&base)?;
    Ok(())
}

// see: cans/plugins/profile.md#Monotonic-limits
#[test]
fn profile_rejects_disagreeing_tenant_and_org_ids() -> Result<(), Box<dyn std::error::Error>> {
    let text = "[session]\ntenant_id = \"tenant-a\"\norg_id = \"tenant-b\"\n";
    let profile: SandboxProfile = toml::from_str(text)?;
    let err = match profile.validate() {
        Ok(()) => return Err("tenant/org disagreement passed validation".into()),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("disagrees"),
        "unexpected validation error: {err}"
    );
    Ok(())
}
