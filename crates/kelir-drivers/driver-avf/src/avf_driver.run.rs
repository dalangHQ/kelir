// see: cans/01-architecture.md#Driver-backends
use kelir_interface::driver_interface::VmmError;

use crate::AvfDriver;

impl AvfDriver {
    // TODO (L3) [hd_08_avf_pkvm_boundary_enforcement]: connect /dev/avf or the pKVM node; slop: un-donated memory maps without E_PKVM_NON_DONATED_MEMORY fault
    pub(crate) fn open_node(&mut self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported("avf boot: /dev/avf open".into()))
    }
}
