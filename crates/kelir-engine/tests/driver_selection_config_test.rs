// see: cans/01-architecture.md#Driver-backends
// see: cans/01-architecture.md#Fallback-policy
use kelir_engine::vmm::DriverRouter;
use kelir_interface::driver_interface::{DriverPolicy, VmmError, WxPolicy};

// see: cans/01-architecture.md#Driver-backends
fn policy(driver: Option<&str>, allow_emulated: bool) -> DriverPolicy {
    DriverPolicy {
        driver: driver.map(str::to_string),
        require_hardware_vmm: false,
        allow_emulated_fallback: allow_emulated,
        verify_hw_lockfile: false,
        wx_enforcement: WxPolicy::Strict,
        ffi_worker_threads: 1,
        ffi_max_ops_per_sec: 0,
    }
}

// see: cans/01-architecture.md#Driver-backends
#[test]
fn driver_selection_config_unknown_backend_rejects() {
    let message = match DriverRouter::load_native_driver(Some("hyperv"), true, true) {
        Err(VmmError::InvalidState(message)) => message,
        _ => String::new(),
    };
    assert!(message.contains("unknown driver `hyperv`"));
}

// see: cans/01-architecture.md#Fallback-policy
#[test]
fn driver_selection_config_emulated_tier_requires_opt_in() {
    for backend in ["bwrap", "proot"] {
        let message = match DriverRouter::load_native_driver(Some(backend), true, false) {
            Err(VmmError::InvalidState(message)) => message,
            _ => String::new(),
        };
        assert!(message.contains("allow_emulated_fallback"));
    }
}

// see: cans/01-architecture.md#Driver-backends
#[test]
fn driver_selection_config_known_backends_pass_validation() {
    for backend in ["auto", "rustvmm", "hvf", "openvmm", "avf"] {
        let result = DriverRouter::load_native_driver(Some(backend), true, false);
        assert!(
            !matches!(result, Err(VmmError::InvalidState(_))),
            "{backend} must pass selection validation"
        );
    }
    for backend in ["bwrap", "proot"] {
        let result = DriverRouter::load_native_driver(Some(backend), true, true);
        assert!(
            !matches!(result, Err(VmmError::InvalidState(_))),
            "{backend} must pass selection validation under emulated opt-in"
        );
    }
}

// see: cans/01-architecture.md#Driver-backends
#[test]
fn driver_selection_config_policy_carries_driver_field() {
    let selected = policy(Some("auto"), false);
    assert_eq!(selected.driver.as_deref(), Some("auto"));
    let unset = policy(None, false);
    assert_eq!(unset.driver, None);
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_04_cold_boot_budget_enforcement.yaml#case_id
#[test]
#[ignore]
fn hd_04_cold_boot_budget_enforcement() -> Result<(), String> {
    let driver = match DriverRouter::load_native_driver(Some("auto"), true, false) {
        Ok(driver) => driver,
        Err(e) => {
            return Err(format!(
                "cold-boot budget check never ran: auto driver selection is an L3 stub: {e}"
            ));
        }
    };
    let caps = driver.capabilities();
    if caps.cold_boot_budget_ms > 50 {
        return Err(format!(
            "auto-selected driver `{}` advertises a {}ms cold-boot budget; first guest workload exec is pinned at 50ms",
            caps.backend, caps.cold_boot_budget_ms
        ));
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_09_bwrap_emulated_fallback_opt_in.yaml#case_id
#[test]
#[ignore]
fn hd_09_bwrap_emulated_fallback_opt_in() -> Result<(), String> {
    let driver = match DriverRouter::load_native_driver(Some("bwrap"), true, true) {
        Ok(driver) => driver,
        Err(e) => {
            return Err(format!(
                "bwrap boot with allow_emulated_fallback=true never loaded the emulated driver: {e}"
            ));
        }
    };
    match driver.is_emulated() {
        Ok(true) => Ok(()),
        Ok(false) => Err("bwrap loaded under opt-in but reports a non-emulated tier".to_string()),
        Err(e) => Err(format!("bwrap is_emulated probe is missing: {e}")),
    }
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_01_nohardware_vmm_fatal_abort.yaml#case_id
#[test]
#[ignore]
fn hd_01_nohardware_vmm_fatal_abort() -> Result<(), String> {
    match DriverRouter::load_native_driver(Some("rustvmm"), true, false) {
        Err(VmmError::NoHardwareVmm(diagnostic)) => {
            assert!(
                diagnostic.contains("E_NOHARDWAREVMM"),
                "diagnostic code missing: {diagnostic}"
            );
        }
        Err(VmmError::Unsupported(message)) => {
            return Err(format!(
                "no-hardware boot halted with the L3 stub instead of NoHardwareVmm: {message}"
            ));
        }
        Err(other) => {
            return Err(format!(
                "no-hardware boot gave the wrong error kind: {other}"
            ));
        }
        Ok(_) => {
            return Err(
                "driver loaded on a no-hardware host without emulated opt-in (silent downgrade)"
                    .to_string(),
            );
        }
    }
    Ok(())
}
