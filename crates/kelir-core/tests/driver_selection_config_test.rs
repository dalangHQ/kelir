// see: cans/01-architecture.md#Driver-backends
// see: cans/01-architecture.md#Fallback-policy
use kelir_core::vmm::DriverRouter;
use kelir_interface::driver_interface::{
    DriverCapabilities, DriverPolicy, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver,
    VmmError, WxPolicy,
};

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
            matches!(result, Err(VmmError::Unsupported(_))),
            "{backend} should pass validation and reach the L3 stub"
        );
    }
    for backend in ["bwrap", "proot"] {
        let result = DriverRouter::load_native_driver(Some(backend), true, true);
        assert!(
            matches!(result, Err(VmmError::Unsupported(_))),
            "{backend} should pass validation and reach the L3 stub"
        );
    }
}

// see: cans/01-architecture.md#Fallback-policy
#[test]
fn driver_selection_config_is_emulated_defaults_to_unsupported() {
    struct ProbeDriver;
    #[async_trait::async_trait]
    impl VmmDriver for ProbeDriver {
        fn capabilities(&self) -> DriverCapabilities {
            DriverCapabilities {
                backend: "probe",
                cold_boot_budget_ms: 0,
                rollback_budget_ms: 0,
                fork_budget_ms: 0,
                requires_hardware_vmm: true,
                supports_cow_fork: false,
            }
        }
        async fn boot(&mut self, _config: VmConfig) -> Result<(), VmmError> {
            Ok(())
        }
        async fn set_entropy_backend(&mut self, _backend: EntropyBackend) -> Result<(), VmmError> {
            Err(VmmError::Unsupported(
                "TODO (L3): set_entropy_backend".into(),
            ))
        }
        async fn entropy_position(&self) -> Result<EntropyStreamPosition, VmmError> {
            Err(VmmError::Unsupported("TODO (L3): entropy_position".into()))
        }
        async fn restore_entropy_position(
            &mut self,
            _position: EntropyStreamPosition,
        ) -> Result<(), VmmError> {
            Err(VmmError::Unsupported(
                "TODO (L3): restore_entropy_position".into(),
            ))
        }
    }
    let driver = ProbeDriver;
    assert!(matches!(
        driver.is_emulated(),
        Err(VmmError::Unsupported(_))
    ));
}

// see: cans/01-architecture.md#Driver-backends
#[test]
fn driver_selection_config_policy_carries_driver_field() {
    let selected = policy(Some("auto"), false);
    assert_eq!(selected.driver.as_deref(), Some("auto"));
    let unset = policy(None, false);
    assert_eq!(unset.driver, None);
}
