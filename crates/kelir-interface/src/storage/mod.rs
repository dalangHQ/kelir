// see: cans/05-storage-plane.md#Storage-architecture
#[path = "fuse_ops.flow.rs"]
pub mod fuse;
#[path = "storage_io.rs"]
pub mod io;
#[path = "isolation_envelope.flow.rs"]
pub mod isolation_envelope;
#[path = "storage_reconciler.rs"]
pub mod reconciler;
#[path = "storage_snoop.flow.rs"]
pub mod snoop;
#[path = "storage_synthetic.flow.rs"]
pub mod synthetic;

pub use fuse::{FuseDirEntry, FuseOpcode, FuseOps, FuseReply, FuseRequest, FuseResponse};
pub use io::{HostFileLock, RootHandle, StorageIo};
pub use isolation_envelope::{
    ContainedOpenOps, IsolationEnvelope, IsolationError, OrgScope, StoragePartition,
};
pub use reconciler::{CommitStats, FsReconcileRequest, ReconcilerOps, TraversalGuard};
pub use snoop::{SnoopOnWrite, SnoopRule};
pub use synthetic::{DynamicFile, DynamicFileGenerator, SyntheticFile};
