// see: cans/plugins/profile.md#Driver-engine-schema-(profile)
use kelir_interface::driver_interface::VmConfig;
use kelir_interface::plugin_model::PluginError;

use crate::sandbox_runtime::SandboxProfile;

// see: cans/plugins/profile.md#Driver-engine-schema-(profile)
// L3-GAP: [hypervisor_driver]: consume max_vcpus, balloon, devices, watchdog timeout/action/cadence, kernel_boot_params, and shim once VmConfig grows driver fields.; slop: VmConfig fields silently dropped, driver boot unpinned
pub fn to_full_vm_config(profile: &SandboxProfile) -> VmConfig {
    let _ = profile;
    VmConfig::default()
}

// see: cans/plugins/profile.md#Driver-engine-schema-(profile)
// L3-GAP: [hd_fast_boot_backend_change]: route driver.fast_boot and storage.dax_window through KelirBuilder.; slop: fast_boot/dax flags dropped, cold path taken silently
pub fn route_fast_boot_and_dax_window(profile: &SandboxProfile) -> Result<(), PluginError> {
    let _ = profile;
    Err(PluginError::Halt(
        "route_fast_boot_and_dax_window: no native implementation (L3)".into(),
    ))
}
