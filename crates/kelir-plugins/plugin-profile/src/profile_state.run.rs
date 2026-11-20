// see: cans/plugins/profile.md#Inversion-of-Control-configuration-router:
use kelir_interface::plugin_model::LifecycleParticipant;

use crate::profile_state::ProfilePlugin;

// see: cans/plugins/profile.md#Plugin-profile
impl LifecycleParticipant for ProfilePlugin {
    // TODO (L3) [bindings_py]: fill the lifecycle hooks when the profile plugin needs them.; slop: lifecycle hooks are never filled, the profile plugin never participates in lifecycle, profile state is never driven.
}
