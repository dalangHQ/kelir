// see: cans/01-architecture.md#Host-runtime
// see: cans/01-architecture.md#Driver-backends
pub struct DriverRouter {
    // No reader in this part: the active driver is driven via Session, not the router.
    #[allow(dead_code)]
    active_driver: Box<dyn kelir_interface::driver::VmmDriver + Send + Sync>,
}

impl DriverRouter {
    pub fn load_native_driver(
        policy_wx: bool,
        allow_emulated: bool,
    ) -> Result<
        Box<dyn kelir_interface::driver::VmmDriver + Send + Sync>,
        kelir_interface::errors::VmmError,
    > {
        // TODO: probe host backend (KVM/HVF/AVF, then emulated fallback if allowed) and load it.
        let _ = (policy_wx, allow_emulated);
        Err(kelir_interface::errors::VmmError::Unsupported(
            "TODO: DriverRouter::load_native_driver".into(),
        ))
    }

    pub fn new(driver: Box<dyn kelir_interface::driver::VmmDriver + Send + Sync>) -> Self {
        Self {
            active_driver: driver,
        }
    }
}
