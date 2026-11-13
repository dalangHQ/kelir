// see: cans/08-lifecycle.md#State-machine
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use kelir_engine::Session;
use kelir_interface::driver_interface::default_vm_config;
use kelir_interface::driver_interface::{
    DriverCapabilities, DriverError, EntropyBackend, EntropyStreamPosition, ExternalSuspendKind,
    SuspendReason, VmConfig, VmmDriver, WatchdogAction,
};
use kelir_interface::lifecycle_state::{LifecycleError, LifecycleState};
use kelir_interface::plugin_model::{LifecycleCtx, PluginError, VetoList};

fn boot_config() -> kelir_interface::driver_interface::VmConfig {
    default_vm_config(
        None,
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    )
}

fn parked() -> LifecycleState {
    LifecycleState::Suspended(SuspendReason::External(
        ExternalSuspendKind::WireConnectionParking,
    ))
}

// see: cans/08-lifecycle.md#State-machine
#[test]
fn lifecycle_legal_edges_transition() -> Result<(), String> {
    let edges = [
        (LifecycleState::Initializing, LifecycleState::Running),
        (LifecycleState::Running, LifecycleState::Draining),
        (LifecycleState::Running, LifecycleState::Terminated),
        (LifecycleState::Running, parked()),
        (LifecycleState::Draining, LifecycleState::Terminated),
        (parked(), LifecycleState::Running),
        (parked(), LifecycleState::Terminated),
    ];
    for (from, to) in edges {
        match from.transition(to) {
            Ok(next) => assert_eq!(next, to),
            Err(error) => return Err(format!("legal edge {from:?} -> {to:?} rejected: {error:?}")),
        }
    }
    Ok(())
}

// see: cans/08-lifecycle.md#State-machine
#[test]
fn lifecycle_illegal_edges_reject_with_typed_error() -> Result<(), String> {
    let edges = [
        (LifecycleState::Initializing, LifecycleState::Draining),
        (LifecycleState::Initializing, parked()),
        (LifecycleState::Initializing, LifecycleState::Terminated),
        (LifecycleState::Running, LifecycleState::Initializing),
        (LifecycleState::Running, LifecycleState::Running),
        (LifecycleState::Draining, LifecycleState::Running),
        (LifecycleState::Draining, parked()),
        (parked(), LifecycleState::Draining),
        (parked(), parked()),
        (LifecycleState::Terminated, LifecycleState::Running),
        (LifecycleState::Terminated, LifecycleState::Terminated),
    ];
    for (from, to) in edges {
        match from.transition(to) {
            Ok(next) => {
                return Err(format!(
                    "illegal edge {from:?} -> {to:?} accepted as {next:?}"
                ));
            }
            Err(LifecycleError::IllegalTransition { from: f, to: t }) => {
                assert_eq!((f, t), (from, to));
            }
        }
    }
    Ok(())
}

// see: cans/08-lifecycle.md#State-machine
#[test]
fn lifecycle_boot_completes_to_running() -> Result<(), String> {
    let session = Session::boot(boot_config()).map_err(|e| e.to_string())?;
    assert_eq!(session.lifecycle_state(), LifecycleState::Running);
    assert!(!session.is_detached());
    Ok(())
}

// see: cans/08-lifecycle.md#Hypervisor-primitives
#[tokio::test]
async fn lifecycle_suspend_holds_for_driver_backend() -> Result<(), String> {
    let mut session = Session::boot(boot_config()).map_err(|e| e.to_string())?;
    match session
        .suspend(SuspendReason::External(
            ExternalSuspendKind::HumanAuthorizationHold,
        ))
        .await
    {
        Err(PluginError::Halt(message)) => assert!(
            message.contains("TODO (L3)"),
            "suspend halt lacks the L3 marker: {message}"
        ),
        Err(e) => return Err(format!("suspend gave wrong error: {e}")),
        Ok(()) => {
            return Err(
                "Session::suspend silently advanced the state machine without a driver backend"
                    .to_string(),
            );
        }
    }
    assert_eq!(session.lifecycle_state(), LifecycleState::Running);
    Ok(())
}

// see: cans/08-lifecycle.md#Watchdog
struct WatchdogProbeDriver;

#[async_trait]
impl VmmDriver for WatchdogProbeDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "watchdog-probe-test",
            cold_boot_budget_ms: 0,
            rollback_budget_ms: 0,
            fork_budget_ms: 0,
            requires_hardware_vmm: false,
            supports_cow_fork: false,
        }
    }

    async fn boot(&mut self, _config: VmConfig) -> Result<(), DriverError> {
        Ok(())
    }

    async fn set_entropy_backend(&mut self, _backend: EntropyBackend) -> Result<(), DriverError> {
        Err(DriverError::Unsupported(
            "watchdog probe test driver has no entropy backend".into(),
        ))
    }

    async fn entropy_position(&self) -> Result<EntropyStreamPosition, DriverError> {
        Err(DriverError::Unsupported(
            "watchdog probe test driver has no entropy backend".into(),
        ))
    }

    async fn restore_entropy_position(
        &mut self,
        _position: EntropyStreamPosition,
    ) -> Result<(), DriverError> {
        Err(DriverError::Unsupported(
            "watchdog probe test driver has no entropy backend".into(),
        ))
    }
}

// see: cans/plugins/replay.md#Invariants-and-policy-(replay)
#[test]
fn lifecycle_boot_mounts_wire_plane_drop_all_when_network_off() -> Result<(), String> {
    let mut config = boot_config();
    config.network_off = true;
    let session = Session::boot(config).map_err(|e| e.to_string())?;
    let wiring = session
        .wire
        .wiring()
        .ok_or_else(|| "network_off boot left the wire plane unwired".to_string())?;
    assert!(
        wiring.sinkhole_drop_all,
        "network_off=true did not mount the wire plane in drop-all mode"
    );
    Ok(())
}

// see: cans/plugins/replay.md#Invariants-and-policy-(replay)
#[test]
fn lifecycle_boot_forces_drop_all_over_explicit_wiring_when_network_off() -> Result<(), String> {
    let mut config = boot_config();
    config.network_off = true;
    let forced = Session::boot_with_plugins(
        config,
        &[],
        None,
        Vec::new(),
        kelir_engine::session_lifecycle::PlaneWiring {
            wire: Some(kelir_interface::wire::net::WireWiring::default()),
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let forced_wiring = forced
        .wire
        .wiring()
        .ok_or_else(|| "network_off boot left the wire plane unwired".to_string())?;
    assert!(
        forced_wiring.sinkhole_drop_all,
        "network_off=true did not force drop-all over explicit wiring"
    );
    let mut offline = boot_config();
    offline.network_off = false;
    let preserved = Session::boot_with_plugins(
        offline,
        &[],
        None,
        Vec::new(),
        kelir_engine::session_lifecycle::PlaneWiring {
            wire: Some(kelir_interface::wire::net::WireWiring::default()),
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let preserved_wiring = preserved
        .wire
        .wiring()
        .ok_or_else(|| "boot left the wire plane unwired".to_string())?;
    assert!(
        !preserved_wiring.sinkhole_drop_all,
        "network_off=false clobbered the caller's explicit wiring"
    );
    Ok(())
}

// see: cans/01-architecture.md#Page-size-dynamic-boot-selection
#[test]
fn lifecycle_boot_warns_but_completes_with_non_default_boot_contract() -> Result<(), String> {
    let mut config = boot_config();
    config.page_size_kb = 16;
    config.pinned_clocksource = Some("kvm-clock".to_string());
    config.preemption_grace_ms = 250;
    let session = Session::boot(config).map_err(|e| e.to_string())?;
    assert_eq!(
        session.lifecycle_state(),
        LifecycleState::Running,
        "non-default boot-contract fields must warn, not halt boot"
    );
    Ok(())
}

// see: cans/08-lifecycle.md#Watchdog
#[tokio::test]
async fn lifecycle_watchdog_probe_records_set_watchdog_invocation() -> Result<(), String> {
    let config = boot_config();
    let session_id = "watchdog-probe".to_string();
    let mut vetoes: VetoList = Vec::new();
    let probe = Arc::new(AtomicBool::new(false));
    let mut driver = WatchdogProbeDriver;
    let mut ctx = LifecycleCtx::new_with_watchdog_probe(
        &config,
        &session_id,
        &mut driver,
        &mut vetoes,
        Arc::clone(&probe),
    );
    assert!(
        !probe.load(Ordering::Acquire),
        "fresh watchdog probe already armed"
    );
    let _ = ctx
        .set_watchdog(Duration::from_secs(1), WatchdogAction::Reboot)
        .await;
    assert!(
        probe.load(Ordering::Acquire),
        "set_watchdog invocation did not flip the armed probe"
    );
    Ok(())
}
