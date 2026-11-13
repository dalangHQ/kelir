// see: cans/01-architecture.md#Host-runtime
// see: cans/01-architecture.md#Driver-backends
pub struct DriverRouter {
    // see: cans/01-architecture.md#Driver-backends
    active_driver: Box<dyn kelir_interface::driver_interface::VmmDriver>,
}

impl DriverRouter {
    // see: cans/01-architecture.md#Driver-backends
    pub fn load_native_driver(
        driver: Option<&str>,
        policy_wx: bool,
        allow_emulated: bool,
    ) -> Result<
        Box<dyn kelir_interface::driver_interface::VmmDriver>,
        kelir_interface::driver_interface::VmmError,
    > {
        use kelir_interface::driver_interface::VmmError;
        let requested = driver.unwrap_or("auto");
        let hardware_backends = ["auto", "rustvmm", "hvf", "openvmm", "avf"];
        let emulated_backends = ["bwrap", "proot"];
        if !hardware_backends.contains(&requested) && !emulated_backends.contains(&requested) {
            return Err(VmmError::InvalidState(format!(
                "unknown driver `{requested}`; expected one of auto/rustvmm/hvf/openvmm/avf/bwrap/proot"
            )));
        }
        if emulated_backends.contains(&requested) && !allow_emulated {
            return Err(VmmError::InvalidState(format!(
                "driver `{requested}` is an unprivileged emulation tier; DriverPolicy.allow_emulated_fallback must opt in per cans/01-architecture.md#Fallback-policy"
            )));
        }
        let _ = policy_wx;
        // TODO (L3) [hd_01_nohardware_vmm_fatal_abort]: probe the host backend (KVM/HVF/AVF) and load the native driver per cans/01-architecture.md#Driver-backends; missing hardware returns VmmError::NoHardwareVmm, never a silent downgrade; slop: silent emulated fallback or Unsupported, never E_NOHARDWAREVMM
        Err(VmmError::Unsupported(
            "TODO (L3) [hd_01_nohardware_vmm_fatal_abort]: DriverRouter::load_native_driver; slop: silent emulated fallback or Unsupported, never E_NOHARDWAREVMM".into(),
        ))
    }

    pub fn new(driver: Box<dyn kelir_interface::driver_interface::VmmDriver>) -> Self {
        Self {
            active_driver: driver,
        }
    }
}
