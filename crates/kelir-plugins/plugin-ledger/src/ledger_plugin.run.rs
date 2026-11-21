// see: cans/plugins/ledger.md#Plugin-ledger
use kelir_interface::plugin_model::LifecycleParticipant;

use crate::LedgerPlugin;

// see: cans/02-plugin-model.md#KelirPlugin-trait
impl LifecycleParticipant for LedgerPlugin {}

// TODO (L3) [telemetry_audit]: halt FUSE and ring ops closed when ledger.strict sees a stalled sink (LedgerCtx::check_backpressure / fail_closed_on_drops); slop: strict mode reports drops while the pipeline keeps running, audit loss stays silent
