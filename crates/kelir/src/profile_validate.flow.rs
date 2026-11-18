// see: cans/plugins/profile.md#Monotonic-limits
use std::path::PathBuf;

use kelir_catalog::{CatalogBackend, CatalogConfig};
use kelir_interface::driver_interface::{DriverPolicy, VmConfig};
use kelir_interface::plugin_model::PluginError;

use crate::profile_schema::EffectiveConfig;
use crate::sandbox_runtime::SandboxProfile;

fn halt(message: String) -> PluginError {
    PluginError::Halt(message)
}

fn security_violation(message: String) -> PluginError {
    PluginError::Halt(format!("ESecurityViolation: {message}"))
}

fn check_ceiling(path: &str, base: Option<u64>, overlay: Option<u64>) -> Result<(), PluginError> {
    if let (Some(base), Some(overlay)) = (base, overlay)
        && overlay > base
    {
        return Err(security_violation(format!(
            "overlay widens {path} from {base} to {overlay}"
        )));
    }
    Ok(())
}

fn check_guard_kept(
    path: &str,
    base: Option<bool>,
    overlay: Option<bool>,
) -> Result<(), PluginError> {
    if base == Some(true) && overlay == Some(false) {
        return Err(security_violation(format!(
            "overlay drops guard {path} from true to false"
        )));
    }
    Ok(())
}

fn check_guard_not_widened(
    path: &str,
    base: Option<bool>,
    overlay: Option<bool>,
) -> Result<(), PluginError> {
    if base == Some(false) && overlay == Some(true) {
        return Err(security_violation(format!(
            "overlay widens {path} from false to true"
        )));
    }
    Ok(())
}

impl SandboxProfile {
    // see: cans/plugins/profile.md#Session-identity-(profile)
    pub fn effective_tenant_id(&self) -> Option<String> {
        let session = self.session.as_ref()?;
        session.tenant_id.clone().or_else(|| session.org_id.clone())
    }

    // see: cans/plugins/profile.md#Driver-engine-schema-(profile)
    pub fn to_driver_policy(&self) -> DriverPolicy {
        let mut policy = DriverPolicy::default();
        if let Some(driver) = self.driver.as_ref() {
            if let Some(backend) = driver.backend.clone() {
                policy.driver = Some(backend);
            }
            if let Some(require) = driver.require_hardware_vmm {
                policy.require_hardware_vmm = require;
            }
            if let Some(allow) = driver.allow_emulated_fallback {
                policy.allow_emulated_fallback = allow;
            }
            if let Some(verify) = driver.verify_hw_lockfile {
                policy.verify_hw_lockfile = verify;
            }
            if let Some(wx) = driver.wx_enforcement {
                policy.wx_enforcement = wx;
            }
            if let Some(tier) = driver.emulated_tier_classification {
                policy.emulated_tier_classification = Some(tier.as_str().to_string());
            }
            if let Some(ffi) = driver.ffi_boundary.as_ref() {
                if let Some(threads) = ffi.worker_threads {
                    policy.ffi_worker_threads = threads as usize;
                }
                if let Some(ops) = ffi.max_ops_per_second {
                    policy.ffi_max_ops_per_sec = ops;
                }
                if let Some(queue) = ffi.queue_capacity {
                    policy.ffi_queue_capacity = queue as usize;
                }
                if let Some(buffer) = ffi.max_buffer_bytes {
                    policy.ffi_max_buffer_bytes = buffer as usize;
                }
                if let Some(threshold) = ffi.zero_copy_shm_threshold_bytes {
                    policy.ffi_zero_copy_shm_threshold_bytes = threshold as usize;
                }
                if let Some(yield_await) = ffi.allow_async_worker_yield {
                    policy.ffi_allow_async_worker_yield = yield_await;
                }
            }
        }
        policy
    }

    pub fn to_vm_config(&self) -> VmConfig {
        let session = self.session.as_ref();
        let compute = session.and_then(|s| s.compute.as_ref());
        let artifacts = session.and_then(|s| s.artifacts.as_ref());
        let clock = session.and_then(|s| s.clock.as_ref());
        let watchdog = session.and_then(|s| s.watchdog.as_ref());
        let mut config = VmConfig {
            driver_policy: self.to_driver_policy(),
            tenant_id: self.effective_tenant_id(),
            ..VmConfig::default()
        };
        if let Some(vcpus) = compute.and_then(|c| c.vcpus) {
            config.vcpus = vcpus;
        }
        if let Some(memory_mb) = compute.and_then(|c| c.memory_mb) {
            config.memory_mb = memory_mb;
        }
        if let Some(kernel) = artifacts.and_then(|a| a.kernel_path.clone()) {
            config.kernel_path = PathBuf::from(kernel);
        }
        if let Some(rootfs) = artifacts.and_then(|a| a.rootfs_erofs_path.clone()) {
            config.rootfs_erofs_path = PathBuf::from(rootfs);
        }
        if let Some(tsc_locked) = clock.and_then(|c| c.tsc_locked) {
            config.tsc_locked = tsc_locked;
        }
        if let Some(watchdog) = watchdog.and_then(|w| w.enabled) {
            config.enable_watchdog = watchdog;
        }
        if let Some(page_size_kb) = compute.and_then(|c| c.page_size_kb) {
            config.page_size_kb = page_size_kb;
        }
        if let Some(deterministic_clock) = clock.and_then(|c| c.deterministic_clock) {
            config.deterministic_clock = deterministic_clock;
        }
        if let Some(pinned) = clock.and_then(|c| c.pinned_clocksource.clone()) {
            config.pinned_clocksource = Some(pinned);
        }
        if let Some(grace) = clock.and_then(|c| c.preemption_grace_ms) {
            config.preemption_grace_ms = grace;
        }
        config
    }

    // see: cans/plugins/profile.md#Monotonic-limits
    pub fn effective_config(&self) -> Result<EffectiveConfig, PluginError> {
        self.validate()?;
        let session = self.session.as_ref();
        let shim = session.and_then(|s| s.shim.as_ref());
        let wire = self.wire.as_ref();
        let checkpoint = self.checkpoint.as_ref();
        let host = self.host.as_ref();
        let plugins = self.plugins.as_ref();
        let ledger = plugins.and_then(|p| p.ledger.as_ref());
        let tool_guard = plugins.and_then(|p| p.tool_guard.as_ref());
        Ok(EffectiveConfig {
            policy_plane_mutability: self
                .invariants
                .as_ref()
                .and_then(|i| i.policy_plane_mutability),
            inheritance_lattice: self.invariants.as_ref().and_then(|i| i.inheritance_lattice),
            operator_lease: self
                .invariants
                .as_ref()
                .and_then(|i| i.operator_lease.clone()),
            embed_mode: self
                .driver
                .as_ref()
                .and_then(|d| d.ffi_boundary.as_ref())
                .and_then(|f| f.embed_mode),
            driver_policy: self.to_driver_policy(),
            vm_config: self.to_vm_config(),
            tenant_id: self.effective_tenant_id(),
            org_id: session.and_then(|s| s.org_id.clone()),
            isolation_envelope: session.and_then(|s| s.isolation_envelope.clone()),
            granular_task_freeze: shim.and_then(|s| s.granular_task_freeze),
            task_freeze_timeout_ms: shim.and_then(|s| s.task_freeze_timeout_ms),
            rejection_mode: shim.and_then(|s| s.rejection_mode),
            cow_broker: self.storage.as_ref().and_then(|s| s.cow_broker.clone()),
            userspace_netstack: wire.and_then(|w| w.userspace_netstack),
            default_egress: wire.and_then(|w| w.default_egress.clone()),
            l4_diversion: wire.and_then(|w| w.l4_diversion.clone()),
            l7_protocol_inspection: wire.and_then(|w| w.l7_protocol_inspection),
            outbox_deferral: wire.and_then(|w| w.outbox_deferral.clone()),
            step_dag: checkpoint.and_then(|c| c.step_dag.clone()),
            catalog: host.and_then(|h| h.catalog.clone()),
            telemetry_ring: host.and_then(|h| h.telemetry_ring.clone()),
            ledger_merkle_wal: ledger.and_then(|l| l.merkle_wal.clone()),
            ledger_otel: ledger.and_then(|l| l.otel.clone()),
            tool_guard_interposition: tool_guard.and_then(|t| t.mcp_interposition.clone()),
            tool_guard_taint: tool_guard.and_then(|t| t.taint_governance.clone()),
            mcp: plugins.and_then(|p| p.mcp.clone()),
        })
    }

    // see: cans/06-storage-catalog.md#Entity-model
    pub fn to_catalog_config(&self) -> CatalogConfig {
        let mut config = CatalogConfig::default();
        if let Some(catalog) = self.host.as_ref().and_then(|h| h.catalog.as_ref()) {
            if let Some(backend) = catalog.backend {
                config.backend = backend;
            }
            config.endpoint = catalog.endpoint.clone();
            if let Some(partitioning) = catalog.schema_tenant_partitioning {
                config.schema_tenant_partitioning = partitioning;
            }
            config.fallback_sqlite_path = catalog.fallback_sqlite_path.clone();
        }
        config
    }

    // see: cans/plugins/ledger.md#Plugin-ledger
    #[cfg(feature = "plugin-ledger")]
    pub fn to_ledger_config(&self) -> plugin_ledger::LedgerConfig {
        let mut config = plugin_ledger::LedgerConfig::default();
        if let Some(ledger) = self.plugins.as_ref().and_then(|p| p.ledger.as_ref()) {
            if let Some(wal) = ledger.merkle_wal.as_ref() {
                if let Some(store) = wal.store_path.clone() {
                    config.store = store;
                }
                if let Some(enabled) = wal.enabled {
                    config.merkle_chain = enabled;
                }
                if let Some(standard) = wal.hashing_standard {
                    config.hashing_standard = standard.as_str().to_string();
                }
                if let Some(record) = wal.record_pre_redaction_hash {
                    config.record_pre_redaction_hash = record;
                }
                if let Some(include) = wal.include_hook_spans_in_root {
                    config.include_hook_spans_in_root = include;
                }
                config.witness_cosigning_endpoint = wal.witness_cosigning_endpoint.clone();
            }
            if let Some(otel) = ledger.otel.as_ref() {
                let mut otel_config = plugin_ledger::OtelConfig::default();
                if let Some(endpoint) = otel.endpoint.clone() {
                    otel_config.endpoint = endpoint;
                }
                if let Some(service) = otel.service_name.clone() {
                    otel_config.service_name = service;
                }
                if let Some(propagate) = otel.propagate_w3c_traceparent {
                    otel_config.propagate_w3c_traceparent = propagate;
                }
                if let Some(gauges) = otel.export_diagnostic_pmu_gauges {
                    otel_config.export_diagnostic_pmu_gauges = gauges;
                }
                config.otel = Some(otel_config);
            }
        }
        config
    }

    // see: cans/plugins/tool-guard.md#Plugin-tool-guard
    #[cfg(feature = "plugin-tool-guard")]
    pub fn to_tool_guard_config(&self) -> plugin_tool_guard::ToolGuardConfig {
        let mut config = plugin_tool_guard::ToolGuardConfig::default();
        if let Some(tool_guard) = self.plugins.as_ref().and_then(|p| p.tool_guard.as_ref()) {
            if let Some(interposition) = tool_guard.mcp_interposition.as_ref() {
                if let Some(pin) = interposition.pin_schemas {
                    config.pin_schemas = pin;
                }
                if let Some(max) = interposition.max_invocations_per_trace {
                    config.max_invocations_per_trace = max;
                }
                if let Some(bytes) = interposition.max_frame_bytes {
                    config.max_frame_bytes = bytes;
                }
                if let Some(detect) = interposition.detect_semantic_rug_pull {
                    config.detect_semantic_rug_pull = detect;
                }
                if let Some(strict) = interposition.strict_ast_parameter_validation {
                    config.strict_ast_parameter_validation = strict;
                }
            }
            if let Some(taint) = tool_guard.taint_governance.as_ref()
                && let Some(propagate) = taint.propagate_byte_range_taints
            {
                config.propagate_byte_range_taints = propagate;
            }
        }
        config
    }

    // see: cans/plugins/mcp.md#Plugin-mcp
    #[cfg(feature = "plugin-mcp")]
    pub fn to_mcp_config(&self) -> plugin_mcp::McpConfig {
        let mut config = plugin_mcp::McpConfig::default();
        if let Some(mcp) = self.plugins.as_ref().and_then(|p| p.mcp.as_ref())
            && let Some(bytes) = mcp.max_frame_bytes
        {
            config.max_frame_bytes = bytes;
        }
        config
    }

    // see: cans/plugins/profile.md#Monotonic-limits
    pub fn validate(&self) -> Result<(), PluginError> {
        if let (Some(tenant), Some(org)) = (
            self.session.as_ref().and_then(|s| s.tenant_id.as_ref()),
            self.session.as_ref().and_then(|s| s.org_id.as_ref()),
        ) && tenant != org
        {
            return Err(halt(format!(
                "session.tenant_id `{tenant}` disagrees with session.org_id `{org}`"
            )));
        }
        if let Some(compute) = self.session.as_ref().and_then(|s| s.compute.as_ref())
            && let (Some(vcpus), Some(max)) = (compute.vcpus, compute.max_vcpus)
            && max < vcpus
        {
            return Err(halt(format!(
                "session.compute.max_vcpus {max} is below vcpus {vcpus}"
            )));
        }
        if let Some(ffi) = self.driver.as_ref().and_then(|d| d.ffi_boundary.as_ref()) {
            for (path, value) in [
                (
                    "driver.ffi_boundary.worker_threads",
                    ffi.worker_threads.map(u64::from),
                ),
                (
                    "driver.ffi_boundary.max_ops_per_second",
                    ffi.max_ops_per_second.map(u64::from),
                ),
                (
                    "driver.ffi_boundary.queue_capacity",
                    ffi.queue_capacity.map(u64::from),
                ),
                ("driver.ffi_boundary.max_buffer_bytes", ffi.max_buffer_bytes),
                (
                    "driver.ffi_boundary.zero_copy_shm_threshold_bytes",
                    ffi.zero_copy_shm_threshold_bytes,
                ),
            ] {
                if value == Some(0) {
                    return Err(halt(format!("{path} must be greater than zero")));
                }
            }
            if let (Some(threshold), Some(buffer)) =
                (ffi.zero_copy_shm_threshold_bytes, ffi.max_buffer_bytes)
                && threshold > buffer
            {
                return Err(halt(format!(
                    "driver.ffi_boundary.zero_copy_shm_threshold_bytes {threshold} exceeds max_buffer_bytes {buffer}"
                )));
            }
        }
        if let Some(outbox) = self.wire.as_ref().and_then(|w| w.outbox_deferral.as_ref()) {
            if outbox.buffer_capacity_bytes == Some(0) {
                return Err(halt(
                    "wire.outbox_deferral.buffer_capacity_bytes must be greater than zero"
                        .to_string(),
                ));
            }
            if outbox.compensator_webhook_timeout_ms == Some(0) {
                return Err(halt(
                    "wire.outbox_deferral.compensator_webhook_timeout_ms must be greater than zero"
                        .to_string(),
                ));
            }
        }
        if let Some(catalog) = self.host.as_ref().and_then(|h| h.catalog.as_ref()) {
            if catalog.lease_ttl_seconds == Some(0) {
                return Err(halt(
                    "host.catalog.lease_ttl_seconds must be greater than zero".to_string(),
                ));
            }
            if catalog.busy_timeout_ms == Some(0) {
                return Err(halt(
                    "host.catalog.busy_timeout_ms must be greater than zero".to_string(),
                ));
            }
            if catalog.backend == Some(CatalogBackend::Postgres) && catalog.endpoint.is_none() {
                return Err(halt(
                    "host.catalog.backend postgres requires host.catalog.endpoint".to_string(),
                ));
            }
            if catalog.backend == Some(CatalogBackend::Sqlite)
                && catalog.fallback_sqlite_path.is_none()
            {
                return Err(halt(
                    "host.catalog.backend sqlite requires host.catalog.fallback_sqlite_path"
                        .to_string(),
                ));
            }
        }
        if let Some(ring) = self.host.as_ref().and_then(|h| h.telemetry_ring.as_ref())
            && ring.sliding_window_depth == Some(0)
        {
            return Err(halt(
                "host.telemetry_ring.sliding_window_depth must be greater than zero".to_string(),
            ));
        }
        if let Some(interposition) = self
            .plugins
            .as_ref()
            .and_then(|p| p.tool_guard.as_ref())
            .and_then(|t| t.mcp_interposition.as_ref())
            && interposition.max_frame_bytes == Some(0)
        {
            return Err(halt(
                "plugins.tool_guard.mcp_interposition.max_frame_bytes must be greater than zero"
                    .to_string(),
            ));
        }
        if let Some(mcp) = self.plugins.as_ref().and_then(|p| p.mcp.as_ref())
            && mcp.max_frame_bytes == Some(0)
        {
            return Err(halt(
                "plugins.mcp.max_frame_bytes must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }

    // see: cans/plugins/profile.md#Monotonic-limits
    pub fn validate_overlay(&self, base: &SandboxProfile) -> Result<(), PluginError> {
        self.validate()?;
        base.validate()?;
        if let (Some(lattice), Some(base_lattice)) = (
            self.invariants.as_ref().and_then(|i| i.inheritance_lattice),
            base.invariants.as_ref().and_then(|i| i.inheritance_lattice),
        ) && lattice != base_lattice
        {
            return Err(security_violation(
                "overlay changes invariants.inheritance_lattice".to_string(),
            ));
        }
        if let (Some(mutable), Some(base_mutable)) = (
            self.invariants
                .as_ref()
                .and_then(|i| i.policy_plane_mutability),
            base.invariants
                .as_ref()
                .and_then(|i| i.policy_plane_mutability),
        ) && mutable != base_mutable
        {
            return Err(security_violation(
                "overlay changes invariants.policy_plane_mutability".to_string(),
            ));
        }
        let base_ffi = base.driver.as_ref().and_then(|d| d.ffi_boundary.as_ref());
        let ffi = self.driver.as_ref().and_then(|d| d.ffi_boundary.as_ref());
        check_ceiling(
            "driver.ffi_boundary.worker_threads",
            base_ffi.and_then(|f| f.worker_threads.map(u64::from)),
            ffi.and_then(|f| f.worker_threads.map(u64::from)),
        )?;
        check_ceiling(
            "driver.ffi_boundary.max_ops_per_second",
            base_ffi.and_then(|f| f.max_ops_per_second.map(u64::from)),
            ffi.and_then(|f| f.max_ops_per_second.map(u64::from)),
        )?;
        check_ceiling(
            "driver.ffi_boundary.queue_capacity",
            base_ffi.and_then(|f| f.queue_capacity.map(u64::from)),
            ffi.and_then(|f| f.queue_capacity.map(u64::from)),
        )?;
        check_ceiling(
            "driver.ffi_boundary.max_buffer_bytes",
            base_ffi.and_then(|f| f.max_buffer_bytes),
            ffi.and_then(|f| f.max_buffer_bytes),
        )?;
        check_ceiling(
            "driver.ffi_boundary.zero_copy_shm_threshold_bytes",
            base_ffi.and_then(|f| f.zero_copy_shm_threshold_bytes),
            ffi.and_then(|f| f.zero_copy_shm_threshold_bytes),
        )?;
        check_guard_not_widened(
            "driver.allow_emulated_fallback",
            base.driver.as_ref().and_then(|d| d.allow_emulated_fallback),
            self.driver.as_ref().and_then(|d| d.allow_emulated_fallback),
        )?;
        check_guard_kept(
            "driver.require_hardware_vmm",
            base.driver.as_ref().and_then(|d| d.require_hardware_vmm),
            self.driver.as_ref().and_then(|d| d.require_hardware_vmm),
        )?;
        let base_compute = base.session.as_ref().and_then(|s| s.compute.as_ref());
        let compute = self.session.as_ref().and_then(|s| s.compute.as_ref());
        check_ceiling(
            "session.compute.max_vcpus",
            base_compute.and_then(|c| c.max_vcpus.map(u64::from)),
            compute.and_then(|c| c.max_vcpus.map(u64::from)),
        )?;
        check_ceiling(
            "session.compute.memory_mb",
            base_compute.and_then(|c| c.memory_mb.map(u64::from)),
            compute.and_then(|c| c.memory_mb.map(u64::from)),
        )?;
        check_ceiling(
            "session.shim.max_exec_slots",
            base.session
                .as_ref()
                .and_then(|s| s.shim.as_ref())
                .and_then(|s| s.max_exec_slots.map(u64::from)),
            self.session
                .as_ref()
                .and_then(|s| s.shim.as_ref())
                .and_then(|s| s.max_exec_slots.map(u64::from)),
        )?;
        check_ceiling(
            "session.budget.exec_seconds",
            base.session
                .as_ref()
                .and_then(|s| s.budget.as_ref())
                .and_then(|b| b.exec_seconds),
            self.session
                .as_ref()
                .and_then(|s| s.budget.as_ref())
                .and_then(|b| b.exec_seconds),
        )?;
        check_ceiling(
            "session.watchdog.timeout_seconds",
            base.session
                .as_ref()
                .and_then(|s| s.watchdog.as_ref())
                .and_then(|w| w.timeout_seconds),
            self.session
                .as_ref()
                .and_then(|s| s.watchdog.as_ref())
                .and_then(|w| w.timeout_seconds),
        )?;
        check_guard_kept(
            "session.isolation_envelope.enforce_openat2_beneath",
            base.session
                .as_ref()
                .and_then(|s| s.isolation_envelope.as_ref())
                .and_then(|e| e.enforce_openat2_beneath),
            self.session
                .as_ref()
                .and_then(|s| s.isolation_envelope.as_ref())
                .and_then(|e| e.enforce_openat2_beneath),
        )?;
        check_guard_kept(
            "session.shim.granular_task_freeze",
            base.session
                .as_ref()
                .and_then(|s| s.shim.as_ref())
                .and_then(|s| s.granular_task_freeze),
            self.session
                .as_ref()
                .and_then(|s| s.shim.as_ref())
                .and_then(|s| s.granular_task_freeze),
        )?;
        check_guard_kept(
            "wire.l7_protocol_inspection",
            base.wire.as_ref().and_then(|w| w.l7_protocol_inspection),
            self.wire.as_ref().and_then(|w| w.l7_protocol_inspection),
        )?;
        let base_outbox = base.wire.as_ref().and_then(|w| w.outbox_deferral.as_ref());
        let outbox = self.wire.as_ref().and_then(|w| w.outbox_deferral.as_ref());
        check_ceiling(
            "wire.outbox_deferral.buffer_capacity_bytes",
            base_outbox.and_then(|o| o.buffer_capacity_bytes),
            outbox.and_then(|o| o.buffer_capacity_bytes),
        )?;
        check_ceiling(
            "wire.outbox_deferral.compensator_webhook_timeout_ms",
            base_outbox.and_then(|o| o.compensator_webhook_timeout_ms),
            outbox.and_then(|o| o.compensator_webhook_timeout_ms),
        )?;
        check_guard_kept(
            "wire.outbox_deferral.release_on_step_verification",
            base_outbox.and_then(|o| o.release_on_step_verification),
            outbox.and_then(|o| o.release_on_step_verification),
        )?;
        check_guard_kept(
            "wire.outbox_deferral.drop_on_rollback",
            base_outbox.and_then(|o| o.drop_on_rollback),
            outbox.and_then(|o| o.drop_on_rollback),
        )?;
        let base_dag = base.checkpoint.as_ref().and_then(|c| c.step_dag.as_ref());
        let dag = self.checkpoint.as_ref().and_then(|c| c.step_dag.as_ref());
        check_guard_kept(
            "checkpoint.step_dag.require_signed_attribution",
            base_dag.and_then(|d| d.require_signed_attribution),
            dag.and_then(|d| d.require_signed_attribution),
        )?;
        check_guard_kept(
            "checkpoint.step_dag.require_separation_of_duties",
            base_dag.and_then(|d| d.require_separation_of_duties),
            dag.and_then(|d| d.require_separation_of_duties),
        )?;
        let base_catalog = base.host.as_ref().and_then(|h| h.catalog.as_ref());
        let catalog = self.host.as_ref().and_then(|h| h.catalog.as_ref());
        check_ceiling(
            "host.catalog.lease_ttl_seconds",
            base_catalog.and_then(|c| c.lease_ttl_seconds),
            catalog.and_then(|c| c.lease_ttl_seconds),
        )?;
        check_ceiling(
            "host.catalog.busy_timeout_ms",
            base_catalog.and_then(|c| c.busy_timeout_ms),
            catalog.and_then(|c| c.busy_timeout_ms),
        )?;
        check_guard_kept(
            "host.catalog.schema_tenant_partitioning",
            base_catalog.and_then(|c| c.schema_tenant_partitioning),
            catalog.and_then(|c| c.schema_tenant_partitioning),
        )?;
        check_ceiling(
            "host.telemetry_ring.sliding_window_depth",
            base.host
                .as_ref()
                .and_then(|h| h.telemetry_ring.as_ref())
                .and_then(|r| r.sliding_window_depth),
            self.host
                .as_ref()
                .and_then(|h| h.telemetry_ring.as_ref())
                .and_then(|r| r.sliding_window_depth),
        )?;
        let base_wal = base
            .plugins
            .as_ref()
            .and_then(|p| p.ledger.as_ref())
            .and_then(|l| l.merkle_wal.as_ref());
        let wal = self
            .plugins
            .as_ref()
            .and_then(|p| p.ledger.as_ref())
            .and_then(|l| l.merkle_wal.as_ref());
        check_guard_kept(
            "plugins.ledger.merkle_wal.sign_leaves",
            base_wal.and_then(|w| w.sign_leaves),
            wal.and_then(|w| w.sign_leaves),
        )?;
        check_guard_kept(
            "plugins.ledger.merkle_wal.fsync_on_reap",
            base_wal.and_then(|w| w.fsync_on_reap),
            wal.and_then(|w| w.fsync_on_reap),
        )?;
        check_guard_kept(
            "plugins.ledger.merkle_wal.record_pre_redaction_hash",
            base_wal.and_then(|w| w.record_pre_redaction_hash),
            wal.and_then(|w| w.record_pre_redaction_hash),
        )?;
        check_guard_kept(
            "plugins.ledger.merkle_wal.include_hook_spans_in_root",
            base_wal.and_then(|w| w.include_hook_spans_in_root),
            wal.and_then(|w| w.include_hook_spans_in_root),
        )?;
        let base_guard = base.plugins.as_ref().and_then(|p| p.tool_guard.as_ref());
        let guard = self.plugins.as_ref().and_then(|p| p.tool_guard.as_ref());
        let base_interposition = base_guard.and_then(|g| g.mcp_interposition.as_ref());
        let interposition = guard.and_then(|g| g.mcp_interposition.as_ref());
        check_ceiling(
            "plugins.tool_guard.mcp_interposition.max_frame_bytes",
            base_interposition.and_then(|i| i.max_frame_bytes),
            interposition.and_then(|i| i.max_frame_bytes),
        )?;
        check_ceiling(
            "plugins.tool_guard.mcp_interposition.max_invocations_per_trace",
            base_interposition.and_then(|i| i.max_invocations_per_trace.map(u64::from)),
            interposition.and_then(|i| i.max_invocations_per_trace.map(u64::from)),
        )?;
        check_guard_kept(
            "plugins.tool_guard.mcp_interposition.pin_schemas",
            base_interposition.and_then(|i| i.pin_schemas),
            interposition.and_then(|i| i.pin_schemas),
        )?;
        check_guard_kept(
            "plugins.tool_guard.mcp_interposition.halt_on_schema_drift",
            base_interposition.and_then(|i| i.halt_on_schema_drift),
            interposition.and_then(|i| i.halt_on_schema_drift),
        )?;
        check_guard_kept(
            "plugins.tool_guard.mcp_interposition.detect_semantic_rug_pull",
            base_interposition.and_then(|i| i.detect_semantic_rug_pull),
            interposition.and_then(|i| i.detect_semantic_rug_pull),
        )?;
        check_guard_kept(
            "plugins.tool_guard.mcp_interposition.strict_ast_parameter_validation",
            base_interposition.and_then(|i| i.strict_ast_parameter_validation),
            interposition.and_then(|i| i.strict_ast_parameter_validation),
        )?;
        let base_taint = base_guard.and_then(|g| g.taint_governance.as_ref());
        let taint = guard.and_then(|g| g.taint_governance.as_ref());
        check_guard_kept(
            "plugins.tool_guard.taint_governance.untrusted_ingress_taints",
            base_taint.and_then(|t| t.untrusted_ingress_taints),
            taint.and_then(|t| t.untrusted_ingress_taints),
        )?;
        check_guard_kept(
            "plugins.tool_guard.taint_governance.propagate_byte_range_taints",
            base_taint.and_then(|t| t.propagate_byte_range_taints),
            taint.and_then(|t| t.propagate_byte_range_taints),
        )?;
        check_guard_kept(
            "plugins.tool_guard.taint_governance.mutating_verbs_require_approval",
            base_taint.and_then(|t| t.mutating_verbs_require_approval),
            taint.and_then(|t| t.mutating_verbs_require_approval),
        )?;
        check_ceiling(
            "plugins.mcp.max_frame_bytes",
            base.plugins
                .as_ref()
                .and_then(|p| p.mcp.as_ref())
                .and_then(|m| m.max_frame_bytes),
            self.plugins
                .as_ref()
                .and_then(|p| p.mcp.as_ref())
                .and_then(|m| m.max_frame_bytes),
        )?;
        Ok(())
    }
}
