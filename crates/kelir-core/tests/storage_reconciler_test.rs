// see: cans/artifacts/test-plan/cases/storage_plane/sp_02_symlink_traversal_escape_exdev.yaml#case_id
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use kelir_core::planes::storage::StoragePlane;
use kelir_core::planes::storage::fuse_storage::{
    FuseDispatcher, FuseOpcode, FuseReply, FuseRequest,
};
use kelir_core::planes::storage::io_storage::{
    HostFileLock, RootHandle, StorageIo, UnimplementedStorageIo,
};
use kelir_core::planes::storage::storage_reconciler::{
    FsReconcileRequest, Reconciler, TraversalGuard,
};
use kelir_core::planes::storage::{StorageConfig, WorkspaceEncapsulation};
use kelir_interface::plugin_model::{FsInterposer, HandlerCtx, PluginError};
use kelir_interface::protocol_wire::{PatchPayload, Verdict};
use std::os::unix::fs::PermissionsExt;

fn temp_workspace(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("kelir-et2-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    let _ = std::fs::create_dir_all(&path);
    path
}

fn drop_workspace(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

fn reconciler_for(workspace: &Path) -> Reconciler {
    Reconciler::new(
        workspace.join("broker"),
        workspace.to_path_buf(),
        TraversalGuard::StrictResolveBeneath,
        Arc::new(UnimplementedStorageIo),
        Arc::new(RwLock::new(HashSet::new())),
    )
}

fn reconciler_for_dirs(broker: &Path, host: &Path) -> Reconciler {
    Reconciler::new(
        broker.to_path_buf(),
        host.to_path_buf(),
        TraversalGuard::StrictResolveBeneath,
        Arc::new(UnimplementedStorageIo),
        Arc::new(RwLock::new(HashSet::new())),
    )
}

struct PatchInode;

impl FsInterposer for PatchInode {
    fn on_fs_lookup(&self, _path: &Path) -> Result<Verdict, PluginError> {
        Ok(Verdict::Patch(PatchPayload::FsInode {
            mode: 0o4755,
            uid: 1000,
            gid: 1000,
            size: 4,
            content: b"data".to_vec(),
        }))
    }
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_02_symlink_traversal_escape_exdev.yaml#case_id
#[test]
fn sp_02_symlink_traversal_escape_exdev() -> Result<(), String> {
    let workspace = temp_workspace("sp02");
    let reconciler = reconciler_for(&workspace);
    match reconciler.contain(&workspace.join("../../etc/shadow")) {
        Err(PluginError::Reject { errno, .. }) => assert_eq!(errno, "EXDEV"),
        Err(e) => return Err(format!("parent traversal gave wrong error: {e}")),
        Ok(()) => return Err("parent traversal accepted".to_string()),
    }
    match reconciler.contain(Path::new("/etc/passwd")) {
        Err(PluginError::Reject { errno, .. }) => assert_eq!(errno, "EXDEV"),
        Err(e) => return Err(format!("absolute escape gave wrong error: {e}")),
        Ok(()) => return Err("absolute escape accepted".to_string()),
    }
    reconciler
        .contain(&workspace.join("src/main.rs"))
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/etc", workspace.join("link")).map_err(|e| e.to_string())?;
        match reconciler.contain(&workspace.join("link/passwd")) {
            Err(PluginError::Reject { errno, .. }) => assert_eq!(errno, "EXDEV"),
            Err(e) => return Err(format!("symlink escape gave wrong error: {e}")),
            Ok(()) => return Err("symlink escape accepted".to_string()),
        }
    }
    drop_workspace(&workspace);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_03_setuid_bit_strip_invariant.yaml#case_id
#[test]
fn sp_03_setuid_bit_strip_invariant() -> Result<(), String> {
    let dispatcher = FuseDispatcher::new(Arc::new(PatchInode));
    let ctx = HandlerCtx::new([0u8; 16], 42, (), Vec::new());
    let mut request = FuseRequest {
        opcode: FuseOpcode::Lookup,
        guest_inode: 7,
        path: PathBuf::from("/workspace/exploit_bin"),
        pid: 42,
        data: Vec::new(),
        offset: 0,
        size: 0,
        blocking: false,
    };
    let response = dispatcher
        .dispatch(&mut request, &ctx)
        .map_err(|e| e.to_string())?;
    match response.reply {
        FuseReply::Inode(attr) => {
            assert_eq!(attr.mode, 0o755);
            assert_eq!(attr.mode & 0o6000, 0);
            assert_eq!(attr.uid, 1000);
            assert_eq!(attr.size, 4);
        }
        other => return Err(format!("lookup did not return patched inode: {other:?}")),
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_04_storage_quota_enforcement_enospc.yaml#case_id
#[test]
fn sp_04_storage_quota_enforcement_enospc() -> Result<(), String> {
    let workspace = temp_workspace("sp04");
    let plane = StoragePlane::new(
        workspace.join("root"),
        workspace.join("upper.img"),
        10 * 1024 * 1024,
        10,
    );
    match plane.check_quota(15 * 1024 * 1024, 0) {
        Err(PluginError::Reject { errno, .. }) => assert_eq!(errno, "ENOSPC"),
        Err(e) => return Err(format!("over-quota write gave wrong error: {e}")),
        Ok(()) => return Err("over-quota write accepted".to_string()),
    }
    assert_eq!(plane.allocated_bytes(), 0);
    plane.check_quota(1024, 0).map_err(|e| e.to_string())?;
    assert_eq!(plane.allocated_bytes(), 1024);
    drop_workspace(&workspace);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_05_storage_inode_quota_enospc.yaml#case_id
#[test]
fn sp_05_storage_inode_quota_enospc() -> Result<(), String> {
    let workspace = temp_workspace("sp05");
    let plane = StoragePlane::new(
        workspace.join("root"),
        workspace.join("upper.img"),
        10 * 1024 * 1024,
        10,
    );
    plane.check_quota(0, 10).map_err(|e| e.to_string())?;
    assert_eq!(plane.allocated_inodes(), 10);
    match plane.check_quota(0, 1) {
        Err(PluginError::Reject { errno, .. }) => assert_eq!(errno, "ENOSPC"),
        Err(e) => return Err(format!("over-quota create gave wrong error: {e}")),
        Ok(()) => return Err("eleventh inode accepted".to_string()),
    }
    assert_eq!(plane.allocated_inodes(), 10);
    drop_workspace(&workspace);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_01_host_cow_upper_discard_on_rollback.yaml#case_id
#[test]
#[ignore]
fn sp_01_host_cow_upper_discard_on_rollback() -> Result<(), String> {
    let workspace = temp_workspace("sp01d");
    let upper = workspace.join("upper.img");
    std::fs::create_dir_all(&workspace).map_err(|e| e.to_string())?;
    std::fs::write(&upper, vec![0xABu8; 65536]).map_err(|e| e.to_string())?;
    let plane = StoragePlane::new(
        workspace.join("root"),
        upper.clone(),
        64 * 1024 * 1024,
        1024,
    );
    assert!(
        plane.sparse_upper_blocks() > 0,
        "setup: sparse upper has no host blocks to punch"
    );
    plane
        .record_tombstone(Path::new("README.md"))
        .map_err(|e| e.to_string())?;
    assert_eq!(plane.tombstone_count(), 1);
    match plane.discard() {
        Ok(()) => {}
        Err(PluginError::Halt(message)) => {
            return Err(format!("rollback discard refused: {message}"));
        }
        Err(e) => return Err(format!("rollback discard gave wrong error: {e}")),
    }
    assert_eq!(
        plane.sparse_upper_blocks(),
        0,
        "upper sparse file still carries host blocks after discard"
    );
    assert_eq!(
        plane.tombstone_count(),
        0,
        "tombstones survived the discard"
    );
    drop_workspace(&workspace);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_08_reconciler_atomic_leaf_commit.yaml#case_id
#[test]
#[ignore]
fn sp_08_reconciler_atomic_leaf_commit() -> Result<(), String> {
    let workspace = temp_workspace("sp08");
    let broker = workspace.join("broker");
    std::fs::create_dir_all(broker.join("upper/src")).map_err(|e| e.to_string())?;
    std::fs::write(
        broker.join("upper/src/main.rs"),
        b"fn main() { rebuilt(); }\n",
    )
    .map_err(|e| e.to_string())?;
    let host = workspace.join("host");
    std::fs::create_dir_all(host.join("src")).map_err(|e| e.to_string())?;
    let host_file = host.join("src/main.rs");
    std::fs::write(&host_file, b"fn main() { old(); }\n").map_err(|e| e.to_string())?;
    let before_inode = std::fs::metadata(&host_file)
        .map(|meta| std::os::unix::fs::MetadataExt::ino(&meta))
        .map_err(|e| e.to_string())?;
    let reconciler = reconciler_for_dirs(&broker, &host);
    match reconciler.reconcile(FsReconcileRequest {
        cwd: "/workspace".to_string(),
        atomic_staging: true,
        fsync: true,
    }) {
        Ok(stats) => {
            assert_eq!(stats.files_changed, 1);
            assert_eq!(stats.tombstones_resolved, 0);
        }
        Err(PluginError::Halt(message)) => return Err(format!("reconcile refused: {message}")),
        Err(e) => return Err(format!("reconcile gave wrong error: {e}")),
    }
    assert_eq!(
        std::fs::read_to_string(&host_file).map_err(|e| e.to_string())?,
        "fn main() { rebuilt(); }\n"
    );
    let after_inode = std::fs::metadata(&host_file)
        .map(|meta| std::os::unix::fs::MetadataExt::ino(&meta))
        .map_err(|e| e.to_string())?;
    assert_ne!(
        before_inode, after_inode,
        "committed host file kept its inode; no atomic leaf swap happened"
    );
    let leftovers: Vec<_> = std::fs::read_dir(&host)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
        .collect();
    assert_eq!(
        leftovers.len(),
        0,
        "staging temporaries leaked into the host workspace"
    );
    drop_workspace(&workspace);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_09_reconciler_tombstone_unlink_commit.yaml#case_id
#[test]
#[ignore]
fn sp_09_reconciler_tombstone_unlink_commit() -> Result<(), String> {
    let workspace = temp_workspace("sp09");
    let broker = workspace.join("broker");
    std::fs::create_dir_all(broker.join("upper")).map_err(|e| e.to_string())?;
    let host = workspace.join("host");
    std::fs::create_dir_all(&host).map_err(|e| e.to_string())?;
    let obsolete = host.join("obsolete.rs");
    std::fs::write(&obsolete, b"deprecated\n").map_err(|e| e.to_string())?;
    let tombstones: Arc<RwLock<HashSet<PathBuf>>> = Arc::new(RwLock::new(HashSet::new()));
    tombstones
        .write()
        .map_err(|_| "tombstone lock poisoned".to_string())?
        .insert(PathBuf::from("obsolete.rs"));
    let reconciler = Reconciler::new(
        broker,
        host.clone(),
        TraversalGuard::StrictResolveBeneath,
        Arc::new(UnimplementedStorageIo),
        tombstones,
    );
    match reconciler.reconcile(FsReconcileRequest {
        cwd: "/workspace".to_string(),
        atomic_staging: true,
        fsync: true,
    }) {
        Ok(stats) => {
            assert_eq!(stats.tombstones_resolved, 1);
            assert_eq!(stats.files_changed, 0);
        }
        Err(PluginError::Halt(message)) => return Err(format!("reconcile refused: {message}")),
        Err(e) => return Err(format!("reconcile gave wrong error: {e}")),
    }
    assert!(
        !obsolete.exists(),
        "tombstoned host file was not unlinked by the commit"
    );
    drop_workspace(&workspace);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_10_reconciler_busy_lock_retry_backoff.yaml#case_id
struct EbusyThenOkIo {
    renames: std::sync::atomic::AtomicUsize,
}

impl StorageIo for EbusyThenOkIo {
    fn open_root(&self, root: &Path) -> Result<RootHandle, PluginError> {
        Ok(RootHandle {
            root: root.to_path_buf(),
        })
    }
    fn atomic_rename(&self, _temp: &Path, _target: &Path) -> Result<(), PluginError> {
        let attempt = self
            .renames
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if attempt == 0 {
            return Err(PluginError::reject(
                "EBUSY",
                "host inode locked by an external process".to_string(),
            ));
        }
        Ok(())
    }
    fn lock_exclusive(&self, path: &Path) -> Result<HostFileLock, PluginError> {
        Ok(HostFileLock {
            path: path.to_path_buf(),
        })
    }
    fn punch_hole(&self, _path: &Path, _offset: u64, _len: u64) -> Result<(), PluginError> {
        Ok(())
    }
    fn advise_dontneed(&self, _path: &Path) -> Result<(), PluginError> {
        Ok(())
    }
    fn remove_file(&self, path: &Path) -> Result<(), PluginError> {
        std::fs::remove_file(path)?;
        Ok(())
    }
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_10_reconciler_busy_lock_retry_backoff.yaml#case_id
#[test]
#[ignore]
fn sp_10_reconciler_busy_lock_retry_backoff() -> Result<(), String> {
    let workspace = temp_workspace("sp10");
    let broker = workspace.join("broker");
    std::fs::create_dir_all(broker.join("upper/src")).map_err(|e| e.to_string())?;
    std::fs::write(
        broker.join("upper/src/main.rs"),
        b"fn main() { retried(); }\n",
    )
    .map_err(|e| e.to_string())?;
    let host = workspace.join("host");
    std::fs::create_dir_all(host.join("src")).map_err(|e| e.to_string())?;
    std::fs::write(host.join("src/main.rs"), b"fn main() { old(); }\n")
        .map_err(|e| e.to_string())?;
    let io = Arc::new(EbusyThenOkIo {
        renames: std::sync::atomic::AtomicUsize::new(0),
    });
    let reconciler = Reconciler::new(
        broker,
        host,
        TraversalGuard::StrictResolveBeneath,
        Arc::clone(&io) as Arc<dyn StorageIo>,
        Arc::new(RwLock::new(HashSet::new())),
    );
    match reconciler.reconcile(FsReconcileRequest {
        cwd: "/workspace".to_string(),
        atomic_staging: true,
        fsync: true,
    }) {
        Ok(stats) => assert_eq!(stats.files_changed, 1),
        Err(PluginError::Halt(message)) => return Err(format!("reconcile refused: {message}")),
        Err(e) => return Err(format!("reconcile gave wrong error: {e}")),
    }
    let attempts = io.renames.load(std::sync::atomic::Ordering::SeqCst);
    if attempts < 2 {
        return Err(format!(
            "EBUSY leaf commit was not retried: atomic_rename saw {attempts} attempt(s)"
        ));
    }
    drop_workspace(&workspace);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_12_workspace_encapsulation_mode_000.yaml#case_id
#[test]
#[ignore]
fn sp_12_workspace_encapsulation_mode_000() -> Result<(), String> {
    let workspace = temp_workspace("sp12");
    let root = workspace.join("root");
    std::fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    let lib = root.join("src/lib.rs");
    std::fs::write(&lib, b"pub fn f() {}\n").map_err(|e| e.to_string())?;
    let _plane = StoragePlane::new(
        root.clone(),
        workspace.join("upper.img"),
        64 * 1024 * 1024,
        1024,
    )
    .with_wiring(StorageConfig {
        encapsulation: WorkspaceEncapsulation::Locked,
        ..StorageConfig::default()
    });
    match std::fs::read_to_string(&lib) {
        Ok(content) => {
            return Err(format!(
                "external host read succeeded under Locked encapsulation: {content:?}"
            ));
        }
        Err(e) => assert!(
            e.kind() == std::io::ErrorKind::PermissionDenied,
            "external host read failed with the wrong error: {e}"
        ),
    }
    let _ = std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700));
    drop_workspace(&workspace);
    Ok(())
}
