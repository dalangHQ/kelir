// see: cans/plugins/ledger.md#Plugin-ledger
use kelir_interface::plugin_model::LifecycleParticipant;

use crate::LedgerPlugin;

// see: cans/02-plugin-model.md#KelirPlugin-trait
impl LifecycleParticipant for LedgerPlugin {}
