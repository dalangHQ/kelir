// see: cans/plugins/transaction.md#Plugin-transaction
#![forbid(unsafe_code)]

// TODO (L3) [relational_transaction]: tests/test_transaction.rs per cans/artifacts/config/project-structure.txt; slop: test_transaction.rs missing, 2PC paths untested, Ok(()) stubs pass vacuously
#[path = "coordination.flow.rs"]
pub mod coordination;
#[path = "service_transaction.flow.rs"]
pub mod service_transaction;

pub use coordination::{TransactionCtx, TransactionPlugin};
pub use service_transaction::{Savepoint, TransactionHandle, TransactionScope, TransactionService};

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.transaction.two_phase_commit
#[derive(Clone, Debug)]
pub struct TransactionConfig {
    pub timeout_ms: u64,
    pub auto_rollback_on_taint: bool,
    pub flush_fuse_before_commit: bool,
}
