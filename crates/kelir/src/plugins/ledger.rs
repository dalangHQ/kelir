// see: cans/plugins/ledger.md
pub use plugin_ledger::{LedgerPlugin, LedgerSink};
// TODO (L2): re-export LedgerConfig, OtelConfig, ReceiptConfig, LedgerCtx, FinalizeRequest, FinalReceipt from plugin-ledger once the crate owns its config and context surface per cans/plugins/ledger.md.
