// see: cans/05-storage-plane.md#Storage-architecture
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use kelir_interface::storage::{
    CommitStats, FsReconcileRequest, ReconcilerOps, StorageIo, SyntheticFile,
};

use super::config::StorageConfig;
use super::storage_quota::StorageQuota;
use super::unbound_io::UnboundStorageIo;

pub struct StoragePlane {
    workspace_root: PathBuf,
    sparse_upper_path: PathBuf,
    tombstones: Arc<RwLock<HashSet<PathBuf>>>,
    quota: Arc<StorageQuota>,
    io: Arc<dyn StorageIo>,
    reconciler: OnceLock<Arc<dyn ReconcilerOps>>,
    wiring: StorageConfig,
    metrics: Arc<kelir_otlp::registry_otlp::MetricRegistry>,
    synthetic_files: Vec<SyntheticFile>,
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
            quota: Arc::new(StorageQuota::new(max_bytes, max_inodes)),
            io: Arc::new(UnboundStorageIo),
            reconciler: OnceLock::new(),
            wiring: StorageConfig::default(),
            metrics: Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new()),
            synthetic_files: Vec::new(),
        }
    }

    // see: cans/05-storage-plane.md#Storage-architecture
    // TODO (L3) [storage_plane]: plane engines consume wiring (idmap remap, mask-read scan, encapsulation chmod, shared-cache mounts) per cans/05; slop: wiring unconsumed, encapsulation and masking unenforced
    pub fn with_wiring(mut self, wiring: StorageConfig) -> Self {
        self.wiring = wiring;
        self
    }

    // see: cans/05-storage-plane.md#Storage-architecture
    pub fn wiring(&self) -> &StorageConfig {
        &self.wiring
    }

    pub fn with_metrics(mut self, metrics: Arc<kelir_otlp::registry_otlp::MetricRegistry>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> &Arc<kelir_otlp::registry_otlp::MetricRegistry> {
        &self.metrics
    }

    pub fn with_synthetic_files(mut self, files: Vec<SyntheticFile>) -> Self {
        self.synthetic_files = files;
        self
    }

    pub fn synthetic_files(&self) -> &[SyntheticFile] {
        &self.synthetic_files
    }

    pub fn with_io(mut self, storage_io: Arc<dyn StorageIo>) -> Self {
        self.io = storage_io;
        self
    }

    pub fn storage_io(&self) -> Arc<dyn StorageIo> {
        Arc::clone(&self.io)
    }

    pub fn reconciler(&self) -> Option<Arc<dyn ReconcilerOps>> {
        self.reconciler.get().map(Arc::clone)
    }

    // TODO (L3) [storage_plane]: anchor derives broker_dir from the live host broker journal once the broker owns its layout; slop: anchor without broker journal, broker_dir invented
    pub fn with_reconciler(self, reconciler: Arc<dyn ReconcilerOps>) -> Self {
        let _ = self.reconciler.set(reconciler);
        self
    }

    // see: cans/05-storage-plane.md#Workspace-commit
    pub fn reconcile(
        &self,
        req: FsReconcileRequest,
    ) -> Result<CommitStats, kelir_interface::plugin_model::PluginError> {
        match self.reconciler.get() {
            Some(reconciler) => reconciler.reconcile(req),
            None => Err(kelir_interface::plugin_model::PluginError::Halt(
                "StoragePlane::reconcile needs the anchored reconciler; call with_reconciler() first"
                    .into(),
            )),
        }
    }

    pub fn discard(&self) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [sp_01_host_cow_upper_discard_on_rollback]: drop the sparse upper file and tombstones; base untouched; slop: rollback Ok, upper st_blocks nonzero, tombstones left
        let _ = (&self.sparse_upper_path, &self.tombstones);
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_01_host_cow_upper_discard_on_rollback]: StoragePlane::discard needs the sparse upper drop; slop: rollback Ok, upper st_blocks nonzero, tombstones left".into(),
        ))
    }

    pub fn record_tombstone(
        &self,
        path: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let mut tombstones = self.tombstones.write().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("tombstone lock poisoned".into())
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
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        if let Err(err) = self.quota.check_quota(bytes_to_write, new_inodes) {
            self.metrics.record_backpressure();
            return Err(err);
        }
        self.metrics.add_dirty_pages(bytes_to_write.div_ceil(4096));
        Ok(())
    }

    pub fn punch_hole_sparse_upper(
        &self,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [sp_01_host_cow_upper_discard_on_rollback]: FALLOC_FL_PUNCH_HOLE + posix_fadvise(DONTNEED) on sparse_upper_path; slop: rollback Ok, upper st_blocks nonzero, tombstones left
        self.io.punch_hole(&self.sparse_upper_path, 0, u64::MAX)?;
        self.io.advise_dontneed(&self.sparse_upper_path)
    }

    // see: cans/05-storage-plane.md#Storage-quotas
    pub fn allocated_bytes(&self) -> u64 {
        self.quota.allocated_bytes()
    }

    // see: cans/05-storage-plane.md#Storage-quotas
    pub fn allocated_inodes(&self) -> u64 {
        self.quota.allocated_inodes()
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
    // TODO (L3) [sp_01_host_cow_upper_discard_on_rollback]: report the sparse upper allocation on non-Unix hosts; the constant 0 hides host block usage; slop: rollback Ok, upper st_blocks nonzero, tombstones left
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
