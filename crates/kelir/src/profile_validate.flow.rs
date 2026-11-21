// see: cans/plugins/profile.md#Monotonic-limits
use std::path::PathBuf;

use kelir_catalog::{CatalogBackend, CatalogConfig};
use kelir_interface::driver_interface::{DriverPolicy, VmBalloonConfig, VmConfig};
use kelir_interface::plugin_model::PluginError;

use crate::profile_schema::EffectiveConfig;
use crate::sandbox_runtime::SandboxProfile;

// see: cans/plugins/profile.md#Driver-engine-schema-(profile)
// TODO: [hypervisor_driver] fill to_full_vm_config in the L3 wave; consume max_vcpus, balloon, devices, watchdog timeout/action/cadence, kernel_boot_params, and shim once VmConfig grows driver fields.; slop: VmConfig fields silently dropped, driver boot unpinned
pub fn to_full_vm_config(profile: &SandboxProfile) -> VmConfig {
    let _ = profile;
    VmConfig::default()
}

// see: cans/plugins/profile.md#Driver-engine-schema-(profile)
// TODO: [hd_fast_boot_backend_change] fill route_fast_boot_and_dax_window in the L3 wave; route driver.fast_boot and storage.dax_window through KelirBuilder.; slop: fast_boot/dax flags dropped, cold path taken silently
pub fn route_fast_boot_and_dax_window(profile: &SandboxProfile) -> Result<(), PluginError> {
    let _ = profile;
    Err(PluginError::Halt(
        "route_fast_boot_and_dax_window: deferred to the L3 wave".into(),
    ))
}

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

fn check_list_kept(
    path: &str,
    base: Option<&Vec<String>>,
    overlay: Option<&Vec<String>>,
) -> Result<(), PluginError> {
    if let (Some(base), Some(overlay)) = (base, overlay) {
        for entry in base {
            if !overlay.contains(entry) {
                return Err(security_violation(format!(
                    "overlay removes {path} entry {entry}"
                )));
            }
        }
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
        if let Some(balloon) = session.and_then(|s| s.balloon.as_ref()) {
            let mut vm_balloon = VmBalloonConfig::default();
            if let Some(enabled) = balloon.enabled {
                vm_balloon.enabled = enabled;
            }
            if let Some(initial_mb) = balloon.initial_mb {
                vm_balloon.initial_mb = initial_mb;
            }
            if let Some(pressure_reclaim) = balloon.pressure_reclaim {
                vm_balloon.pressure_reclaim = pressure_reclaim;
            }
            if let Some(reclaim_strategy) = balloon.reclaim_strategy.clone() {
                vm_balloon.reclaim_strategy = reclaim_strategy;
            }
            config.balloon = Some(vm_balloon);
        }
        if let Some(mask) = session
            .and_then(|s| s.shim.as_ref())
            .and_then(|s| s.mask_proc_environ)
        {
            config.mask_proc_environ = mask;
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
            per_exec_cgroup: shim.and_then(|s| s.per_exec_cgroup),
            deny_abi_compat: shim.and_then(|s| s.deny_abi_compat),
            blocked_syscalls: shim.and_then(|s| s.blocked_syscalls.clone()),
            blocked_socket_families: shim.and_then(|s| s.blocked_socket_families.clone()),
            steering: session.and_then(|s| s.steering.clone()),
            dns_to_ip_pinning: wire.and_then(|w| w.dns_to_ip_pinning),
            blocking_wait_timeout_ms: self
                .storage
                .as_ref()
                .and_then(|s| s.cow_broker.as_ref())
                .and_then(|b| b.blocking_wait_timeout_ms),
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
        if let Some(compute) = self.session.as_ref().and_then(|s| s.compute.as_ref())
            && let Some(balloon) = self.session.as_ref().and_then(|s| s.balloon.as_ref())
            && let (Some(memory_mb), Some(initial_mb)) = (compute.memory_mb, balloon.initial_mb)
            && initial_mb > memory_mb
        {
            return Err(halt(format!(
                "session.balloon.initial_mb {initial_mb} exceeds session.compute.memory_mb {memory_mb}"
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
        if let Some(shim) = self.session.as_ref().and_then(|s| s.shim.as_ref()) {
            for (path, entries) in [
                (
                    "session.shim.blocked_syscalls",
                    shim.blocked_syscalls.as_ref(),
                ),
                (
                    "session.shim.blocked_socket_families",
                    shim.blocked_socket_families.as_ref(),
                ),
            ] {
                if let Some(entries) = entries
                    && entries.iter().any(|entry| entry.is_empty())
                {
                    return Err(halt(format!("{path} must not contain empty entries")));
                }
            }
        }
        if let Some(broker) = self.storage.as_ref().and_then(|s| s.cow_broker.as_ref())
            && broker.blocking_wait_timeout_ms == Some(0)
        {
            return Err(halt(
                "storage.cow_broker.blocking_wait_timeout_ms must be greater than zero".to_string(),
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
            "session.balloon.initial_mb",
            base.session
                .as_ref()
                .and_then(|s| s.balloon.as_ref())
                .and_then(|b| b.initial_mb.map(u64::from)),
            self.session
                .as_ref()
                .and_then(|s| s.balloon.as_ref())
                .and_then(|b| b.initial_mb.map(u64::from)),
        )?;
        check_guard_kept(
            "session.balloon.pressure_reclaim",
            base.session
                .as_ref()
                .and_then(|s| s.balloon.as_ref())
                .and_then(|b| b.pressure_reclaim),
            self.session
                .as_ref()
                .and_then(|s| s.balloon.as_ref())
                .and_then(|b| b.pressure_reclaim),
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
            "session.shim.mask_proc_environ",
            base.session
                .as_ref()
                .and_then(|s| s.shim.as_ref())
                .and_then(|s| s.mask_proc_environ),
            self.session
                .as_ref()
                .and_then(|s| s.shim.as_ref())
                .and_then(|s| s.mask_proc_environ),
        )?;
        check_guard_kept(
            "wire.l7_protocol_inspection",
            base.wire.as_ref().and_then(|w| w.l7_protocol_inspection),
            self.wire.as_ref().and_then(|w| w.l7_protocol_inspection),
        )?;
        check_guard_kept(
            "wire.block_abstract_unix_sockets",
            base.wire
                .as_ref()
                .and_then(|w| w.block_abstract_unix_sockets),
            self.wire
                .as_ref()
                .and_then(|w| w.block_abstract_unix_sockets),
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
        let base_shim = base.session.as_ref().and_then(|s| s.shim.as_ref());
        let shim = self.session.as_ref().and_then(|s| s.shim.as_ref());
        check_guard_kept(
            "session.shim.per_exec_cgroup",
            base_shim.and_then(|s| s.per_exec_cgroup),
            shim.and_then(|s| s.per_exec_cgroup),
        )?;
        check_guard_kept(
            "session.shim.deny_abi_compat",
            base_shim.and_then(|s| s.deny_abi_compat),
            shim.and_then(|s| s.deny_abi_compat),
        )?;
        check_list_kept(
            "session.shim.blocked_syscalls",
            base_shim.and_then(|s| s.blocked_syscalls.as_ref()),
            shim.and_then(|s| s.blocked_syscalls.as_ref()),
        )?;
        check_list_kept(
            "session.shim.blocked_socket_families",
            base_shim.and_then(|s| s.blocked_socket_families.as_ref()),
            shim.and_then(|s| s.blocked_socket_families.as_ref()),
        )?;
        check_guard_kept(
            "wire.dns_to_ip_pinning",
            base.wire.as_ref().and_then(|w| w.dns_to_ip_pinning),
            self.wire.as_ref().and_then(|w| w.dns_to_ip_pinning),
        )?;
        let base_steering = base.session.as_ref().and_then(|s| s.steering.as_ref());
        let steering = self.session.as_ref().and_then(|s| s.steering.as_ref());
        check_guard_kept(
            "session.steering.steer_on_deny",
            base_steering.and_then(|s| s.steer_on_deny),
            steering.and_then(|s| s.steer_on_deny),
        )?;
        check_guard_kept(
            "session.steering.steer_on_redirect",
            base_steering.and_then(|s| s.steer_on_redirect),
            steering.and_then(|s| s.steer_on_redirect),
        )?;
        check_guard_kept(
            "session.steering.inject_exec_stderr",
            base_steering.and_then(|s| s.inject_exec_stderr),
            steering.and_then(|s| s.inject_exec_stderr),
        )?;
        check_ceiling(
            "storage.cow_broker.blocking_wait_timeout_ms",
            base.storage
                .as_ref()
                .and_then(|s| s.cow_broker.as_ref())
                .and_then(|b| b.blocking_wait_timeout_ms),
            self.storage
                .as_ref()
                .and_then(|s| s.cow_broker.as_ref())
                .and_then(|b| b.blocking_wait_timeout_ms),
        )?;
        Ok(())
    }
}
