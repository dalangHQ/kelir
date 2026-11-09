// see: cans/01-architecture.md#Host-runtime
// see: cans/01-architecture.md#Driver-backends
pub struct DriverRouter {
    // see: cans/01-architecture.md#Driver-backends
    #[allow(dead_code)]
    active_driver: Box<dyn kelir_interface::driver_interface::VmmDriver + Send + Sync>,
}

impl DriverRouter {
    // TODO (L3): probe the host backend (KVM/HVF/AVF, then emulated fallback if allowed) and load the native driver per cans/01-architecture.md#Driver-backends.
    pub fn load_native_driver(
        policy_wx: bool,
        allow_emulated: bool,
    ) -> Result<
        Box<dyn kelir_interface::driver_interface::VmmDriver + Send + Sync>,
        kelir_interface::driver_interface::VmmError,
    > {
        let _ = (policy_wx, allow_emulated);
        Err(kelir_interface::driver_interface::VmmError::Unsupported(
            "TODO (L3): DriverRouter::load_native_driver".into(),
        ))
    }

    pub fn new(
        driver: Box<dyn kelir_interface::driver_interface::VmmDriver + Send + Sync>,
    ) -> Self {
        Self {
            active_driver: driver,
        }
    }
}
