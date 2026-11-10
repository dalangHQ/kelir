// see: cans/artifacts/test-plan/cases/storage_plane/sp_02_symlink_traversal_escape_exdev.yaml#case_id
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use kelir_core::planes::storage::StoragePlane;
use kelir_core::planes::storage::fuse_storage::{
    FuseDispatcher, FuseOpcode, FuseReply, FuseRequest,
};
use kelir_core::planes::storage::io_storage::UnimplementedStorageIo;
use kelir_core::planes::storage::storage_reconciler::{Reconciler, TraversalGuard};
use kelir_interface::plugin_model::{FsInterposer, HandlerCtx, PluginError};
use kelir_interface::protocol_wire::{PatchPayload, Verdict};

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
