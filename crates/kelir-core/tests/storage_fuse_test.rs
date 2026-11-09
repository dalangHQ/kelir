use std::path::{Path, PathBuf};
use std::sync::Arc;

use kelir_core::planes::storage::fuse_storage::{
    FuseDispatcher, FuseOpcode, FuseReply, FuseRequest,
};
use kelir_core::planes::storage::storage_synthetic::SyntheticFile;
use kelir_interface::plugin_model::{FsInterposer, HandlerCtx};

fn temp_workspace(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("kelir-et2-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    let _ = std::fs::create_dir_all(&path);
    path
}

fn drop_workspace(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

struct PassFs;

impl FsInterposer for PassFs {}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_13_synthetic_fuse_inode_read_only_0400.yaml#case_id
#[test]
fn sp_13_synthetic_fuse_inode_read_only_0400() -> Result<(), String> {
    let workspace = temp_workspace("sp13");
    assert!(!workspace.join(".env").exists());
    let dispatcher = FuseDispatcher::new(Arc::new(PassFs))
        .with_synthetic_files(vec![SyntheticFile::static_(".env", "SECRET=example")]);
    let ctx = HandlerCtx::new([0u8; 16], 42, (), Vec::new());
    let mut lookup = FuseRequest {
        opcode: FuseOpcode::Lookup,
        guest_inode: 7,
        path: PathBuf::from(".env"),
        pid: 42,
        data: Vec::new(),
        offset: 0,
        size: 0,
        blocking: false,
    };
    let response = dispatcher
        .dispatch(&mut lookup, &ctx)
        .map_err(|e| e.to_string())?;
    match response.reply {
        FuseReply::Inode(attr) => {
            assert_eq!(attr.mode, 0o400);
            assert_eq!(attr.size, 14);
            assert_eq!(attr.content, b"SECRET=example");
        }
        other => return Err(format!("synthetic lookup did not return inode: {other:?}")),
    }
    let mut read = FuseRequest {
        opcode: FuseOpcode::Read,
        guest_inode: 7,
        path: PathBuf::from(".env"),
        pid: 42,
        data: Vec::new(),
        offset: 0,
        size: 100,
        blocking: false,
    };
    let read_response = dispatcher
        .dispatch(&mut read, &ctx)
        .map_err(|e| e.to_string())?;
    match read_response.reply {
        FuseReply::Payload(bytes) => assert_eq!(bytes, b"SECRET=example"),
        other => return Err(format!("synthetic read did not return content: {other:?}")),
    }
    assert!(!workspace.join(".env").exists());
    drop_workspace(&workspace);
    Ok(())
}
