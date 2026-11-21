// see: cans/01-architecture.md#Host-state-root
// see: cans/06-storage-catalog.md#Entity-model
pub use crate::session_paths_flow::{
    reap_ephemeral_session_dir, register_incarnation_proof_row, session_dir,
    sweep_orphaned_session_dirs, upgrade_detached_lease_row,
};
