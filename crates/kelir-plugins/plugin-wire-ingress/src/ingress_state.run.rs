// see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
use kelir_interface::plugin_model::LifecycleParticipant;

use crate::ingress_state::WireIngressPlugin;

impl LifecycleParticipant for WireIngressPlugin {
    // TODO (L3) [wire_plane]: implement wire-ingress lifecycle participation; all hooks use the trait defaults.; slop: wire-ingress never participates in the lifecycle, hooks stay on trait defaults, ingress state is never driven.
}
