// see: cans/01-architecture.md#Driver-backends
use kelir_interface::driver_interface::VmmError;

use crate::OpenVmmDriver;

impl OpenVmmDriver {
    // TODO (L3) [hd_07_openvmm_apic_restore_sequence]: create the WHP partition via WHvCreatePartition and commit Local APIC state before the TSC deadline; slop: TSC deadline set before APIC commit, restore order never logged
    pub(crate) fn create_partition(&mut self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "openvmm boot: WHvCreatePartition".into(),
        ))
    }
}
