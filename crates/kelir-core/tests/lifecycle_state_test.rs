// see: cans/08-lifecycle.md#State-machine
use kelir_core::Session;
use kelir_interface::driver_interface::default_vm_config;
use kelir_interface::driver_interface::{ExternalSuspendKind, SuspendReason};
use kelir_interface::lifecycle_state::{LifecycleError, LifecycleState};

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
