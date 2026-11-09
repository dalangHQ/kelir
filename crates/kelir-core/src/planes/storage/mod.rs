// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/05-storage-plane.md#Host-CoW-broker
// see: cans/05-storage-plane.md#Storage-quotas
pub mod fuse;
pub mod io;
pub mod lock_broker;
pub mod reconciler;
pub mod synthetic;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

pub struct StorageQuota {
    allocated_bytes: AtomicU64,
    allocated_inodes: AtomicU64,
    max_bytes: u64,
    max_inodes: u64,
}

// see: cans/05-storage-plane.md#Storage-quotas
fn accrue_quota(
    counter: &AtomicU64,
    delta: u64,
    max: u64,
    kind: &str,
) -> Result<(), kelir_interface::plugin::PluginError> {
    let mut current = counter.load(Ordering::SeqCst);
    loop {
        let total = current.checked_add(delta).ok_or_else(|| {
            kelir_interface::plugin::PluginError::Halt(format!("quota {kind} arithmetic overflow"))
        })?;
        if total > max {
            return Err(kelir_interface::plugin::PluginError::reject(
                "ENOSPC",
                format!("storage {kind} quota exceeded"),
            ));
        }
        match counter.compare_exchange(current, total, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return Ok(()),
            Err(observed) => current = observed,
        }
    }
}

pub struct StoragePlane {
    workspace_root: PathBuf,
    sparse_upper_path: PathBuf,
    tombstones: Arc<RwLock<HashSet<PathBuf>>>,
    quota: StorageQuota,
    io: Arc<dyn io::StorageIo>,
    reconciler: OnceLock<reconciler::Reconciler>,
    lock_broker: OnceLock<Arc<lock_broker::LockBroker>>,
    lock_namespace: [u8; 16],
    fuse_worker: OnceLock<Mutex<fuse::FuseWorker>>,
    metrics: Arc<kelir_otlp::registry::MetricRegistry>,
    synthetic_files: Vec<synthetic::SyntheticFile>,
}

impl StoragePlane {
    pub fn new(
        workspace_root: PathBuf,
        sparse_upper: PathBuf,
        max_bytes: u64,
        max_inodes: u64,
    ) -> Self {
        Self {
            workspace_root,
            sparse_upper_path: sparse_upper,
            tombstones: Arc::new(RwLock::new(HashSet::new())),
            quota: StorageQuota {
                allocated_bytes: AtomicU64::new(0),
                allocated_inodes: AtomicU64::new(0),
                max_bytes,
                max_inodes,
            },
            io: Arc::new(io::UnimplementedStorageIo),
            reconciler: OnceLock::new(),
            lock_broker: OnceLock::new(),
            lock_namespace: [0; 16],
            fuse_worker: OnceLock::new(),
            metrics: Arc::new(kelir_otlp::registry::MetricRegistry::new()),
            synthetic_files: Vec::new(),
        }
    }

    pub fn with_metrics(mut self, metrics: Arc<kelir_otlp::registry::MetricRegistry>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> &Arc<kelir_otlp::registry::MetricRegistry> {
        &self.metrics
    }

    pub fn with_synthetic_files(mut self, files: Vec<synthetic::SyntheticFile>) -> Self {
        self.synthetic_files = files;
        self
    }

    pub fn synthetic_files(&self) -> &[synthetic::SyntheticFile] {
        &self.synthetic_files
    }

    pub fn with_io(mut self, storage_io: Arc<dyn io::StorageIo>) -> Self {
        self.io = storage_io;
        self
    }

    // TODO (L3): first construction wins; later calls with different broker_dir/guard return the anchored reconciler.
    pub fn reconciler(
        &self,
        broker_dir: PathBuf,
        guard: reconciler::TraversalGuard,
    ) -> &reconciler::Reconciler {
        self.reconciler.get_or_init(|| {
            reconciler::Reconciler::new(
                broker_dir,
                self.workspace_root.clone(),
                guard,
                self.io.clone(),
                self.tombstones.clone(),
            )
        })
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub fn set_lock_namespace(&mut self, namespace: [u8; 16]) {
        self.lock_namespace = namespace;
    }

    pub fn lock_broker(
        &self,
        interposer: Arc<dyn kelir_interface::plugin::FsInterposer>,
    ) -> &lock_broker::LockBroker {
        self.lock_broker_arc(interposer)
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub fn lock_broker_coordination(
        &self,
        interposer: Arc<dyn kelir_interface::plugin::FsInterposer>,
    ) -> Arc<dyn kelir_interface::plugin::LockBrokerCoordination> {
        let broker: Arc<lock_broker::LockBroker> = Arc::clone(self.lock_broker_arc(interposer));
        broker
    }

    fn lock_broker_arc(
        &self,
        interposer: Arc<dyn kelir_interface::plugin::FsInterposer>,
    ) -> &Arc<lock_broker::LockBroker> {
        self.lock_broker.get_or_init(|| {
            Arc::new(lock_broker::LockBroker::new(interposer).with_namespace(self.lock_namespace))
        })
    }

    pub fn fuse_worker(
        &self,
        receiver: std::sync::mpsc::Receiver<fuse::FuseRequest>,
        interposer: Arc<dyn kelir_interface::plugin::FsInterposer>,
        session_id: [u8; 16],
    ) -> &Mutex<fuse::FuseWorker> {
        self.fuse_worker.get_or_init(|| {
            Mutex::new(
                fuse::FuseWorker::new(receiver, interposer, session_id)
                    .with_synthetic_files(self.synthetic_files.clone()),
            )
        })
    }

    // TODO (L3): wire boot to anchor the reconciler with the broker dir and traversal guard; no in-tree caller does, so this guard rejects every reconcile.
    pub fn reconcile(
        &self,
        req: reconciler::FsReconcileRequest,
    ) -> Result<reconciler::CommitStats, kelir_interface::plugin::PluginError> {
        match self.reconciler.get() {
            Some(reconciler) => reconciler.reconcile(req),
            None => Err(kelir_interface::plugin::PluginError::Halt(
                "StoragePlane::reconcile needs the anchored reconciler; call reconciler() first"
                    .into(),
            )),
        }
    }

    pub fn discard(&self) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): drop the sparse upper file and tombstones; base untouched.
        let _ = (&self.sparse_upper_path, &self.tombstones);
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): StoragePlane::discard needs the sparse upper drop".into(),
        ))
    }

    pub fn record_tombstone(
        &self,
        path: &Path,
    ) -> Result<(), kelir_interface::plugin::PluginError> {
        let mut tombstones = self.tombstones.write().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("tombstone lock poisoned".into())
        })?;
        tombstones.insert(path.to_path_buf());
        Ok(())
    }

    // see: cans/05-storage-plane.md#Storage-quotas
    // see: cans/14-observability.md#Metrics-pipeline
    pub fn check_quota(
        &self,
        bytes_to_write: u64,
        new_inodes: u64,
    ) -> Result<(), kelir_interface::plugin::PluginError> {
        if let Err(err) = accrue_quota(
            &self.quota.allocated_bytes,
            bytes_to_write,
            self.quota.max_bytes,
            "byte",
        ) {
            self.metrics.record_backpressure();
            return Err(err);
        }
        if let Err(err) = accrue_quota(
            &self.quota.allocated_inodes,
            new_inodes,
            self.quota.max_inodes,
            "inode",
        ) {
            self.metrics.record_backpressure();
            self.quota
                .allocated_bytes
                .fetch_sub(bytes_to_write, Ordering::SeqCst);
            return Err(err);
        }
        self.metrics.add_dirty_pages(bytes_to_write.div_ceil(4096));
        Ok(())
    }

    pub fn punch_hole_sparse_upper(&self) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): FALLOC_FL_PUNCH_HOLE + posix_fadvise(DONTNEED) on sparse_upper_path.
        self.io.punch_hole(&self.sparse_upper_path, 0, u64::MAX)?;
        self.io.advise_dontneed(&self.sparse_upper_path)
    }

    // see: cans/05-storage-plane.md#Storage-quotas
    pub fn allocated_bytes(&self) -> u64 {
        self.quota.allocated_bytes.load(Ordering::SeqCst)
    }

    // see: cans/05-storage-plane.md#Storage-quotas
    pub fn allocated_inodes(&self) -> u64 {
        self.quota.allocated_inodes.load(Ordering::SeqCst)
    }

    // see: cans/05-storage-plane.md#Host-CoW-broker
    pub fn tombstone_count(&self) -> usize {
        self.tombstones.read().map(|guard| guard.len()).unwrap_or(0)
    }

    // see: cans/05-storage-plane.md#Host-CoW-broker
    #[cfg(unix)]
    pub fn sparse_upper_blocks(&self) -> u64 {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(&self.sparse_upper_path)
            .map(|metadata| metadata.blocks())
            .unwrap_or(0)
    }

    // see: cans/05-storage-plane.md#Host-CoW-broker
    // TODO (L3): report the sparse upper allocation on non-Unix hosts; the constant 0 hides host block usage.
    #[cfg(not(unix))]
    pub fn sparse_upper_blocks(&self) -> u64 {
        0
    }

    // see: cans/05-storage-plane.md#Host-CoW-broker
    pub fn sparse_upper_len(&self) -> u64 {
        std::fs::metadata(&self.sparse_upper_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0)
    }
}
