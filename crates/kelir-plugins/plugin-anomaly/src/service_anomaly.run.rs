// see: cans/plugins/anomaly.md#Plugin-anomaly
use kelir_interface::plugin_model::LifecycleParticipant;

use crate::AnomalyPlugin;

// see: cans/02-plugin-model.md#KelirPlugin-trait
impl LifecycleParticipant for AnomalyPlugin {}
