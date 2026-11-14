use std::path::{Path, PathBuf};
use std::sync::Arc;

use kelir_engine::dispatch_plugin::PipelineEngine;
use kelir_engine::planes::storage::fuse::FuseDispatcher;
use kelir_interface::plugin_model::{FsInterposer, HandlerCtx, MountScope, PipelineBinder};
use kelir_interface::storage::{FuseOpcode, FuseReply, FuseRequest, SyntheticFile};

fn temp_workspace(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("kelir-et2-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    let _ = std::fs::create_dir_all(&path);
    path
}

fn drop_workspace(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

fn engine_with(interposer: Arc<dyn FsInterposer>) -> Arc<PipelineEngine> {
    let mut engine = PipelineEngine::new();
    engine.bind_fs_scoped(interposer, MountScope::default());
    Arc::new(engine)
}

struct PassFs;

impl FsInterposer for PassFs {}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_13_synthetic_fuse_inode_read_only_0400.yaml#case_id
#[test]
fn sp_13_synthetic_fuse_inode_read_only_0400() -> Result<(), String> {
    let workspace = temp_workspace("sp13");
    assert!(!workspace.join(".env").exists());
    let dispatcher = FuseDispatcher::new(kelir_storage::fuse_ops_with_synthetic_files(
        engine_with(Arc::new(PassFs)),
        vec![SyntheticFile::static_(".env", "SECRET=example")],
    ));
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

// see: cans/artifacts/test-plan/cases/storage_plane/sp_11_host_fifo_socket_lookup_enxio.yaml#case_id
#[test]
#[ignore]
fn sp_11_host_fifo_socket_lookup_enxio() -> Result<(), String> {
    let workspace = temp_workspace("sp11");
    let sock = workspace.join("relay.sock");
    let listener = std::os::unix::net::UnixListener::bind(&sock).map_err(|e| e.to_string())?;
    drop(listener);
    let metadata = std::fs::metadata(&sock).map_err(|e| e.to_string())?;
    assert!(
        std::os::unix::fs::FileTypeExt::is_socket(&metadata.file_type()),
        "host path is not a socket"
    );
    let dispatcher = FuseDispatcher::new(kelir_storage::fuse_ops(engine_with(Arc::new(PassFs))));
    let ctx = HandlerCtx::new([0u8; 16], 42, (), Vec::new());
    let mut request = FuseRequest {
        opcode: FuseOpcode::Lookup,
        guest_inode: 7,
        path: sock.clone(),
        pid: 42,
        data: Vec::new(),
        offset: 0,
        size: 0,
        blocking: false,
    };
    let outcome = dispatcher.dispatch(&mut request, &ctx);
    drop_workspace(&workspace);
    match outcome {
        Err(kelir_interface::plugin_model::PluginError::Halt(message)) => assert!(
            message.contains("ENXIO"),
            "host socket lookup denied with the wrong diagnostic: {message}"
        ),
        Ok(response) => match response.reply {
            FuseReply::Errno(code) => assert_eq!(code, -libc::ENXIO),
            other => {
                return Err(format!(
                    "host socket lookup returned {other:?} instead of ENXIO"
                ));
            }
        },
        Err(e) => return Err(format!("host socket lookup gave wrong error: {e}")),
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_14_in_flight_read_masking_pattern_replace.yaml#case_id
struct MaskingFs;

impl FsInterposer for MaskingFs {
    fn on_fs_read(
        &self,
        _path: &Path,
        buffer: &[u8],
    ) -> Result<Vec<u8>, kelir_interface::plugin_model::PluginError> {
        let mut out = buffer.to_vec();
        if let Some(at) = out
            .windows(b"AKIAABCDEFGHIJKLMNOP".len())
            .position(|window| window == b"AKIAABCDEFGHIJKLMNOP")
        {
            out[at..at + 20].copy_from_slice(b"REDACTED.aws.key.000");
        }
        Ok(out)
    }
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_14_in_flight_read_masking_pattern_replace.yaml#case_id
#[test]
#[ignore]
fn sp_14_in_flight_read_masking_pattern_replace() -> Result<(), String> {
    let workspace = temp_workspace("sp14");
    let config = workspace.join("config.toml");
    std::fs::write(&config, b"token = 'AKIAABCDEFGHIJKLMNOP'\n").map_err(|e| e.to_string())?;
    let masked: &[u8] = b"token = 'REDACTED.aws.key.000'\n";
    let dispatcher = FuseDispatcher::new(kelir_storage::fuse_ops(engine_with(Arc::new(MaskingFs))));
    let ctx = HandlerCtx::new([0u8; 16], 42, (), Vec::new());
    let mut request = FuseRequest {
        opcode: FuseOpcode::Read,
        guest_inode: 7,
        path: config.clone(),
        pid: 42,
        data: b"token = 'AKIAABCDEFGHIJKLMNOP'\n".to_vec(),
        offset: 0,
        size: 28,
        blocking: false,
    };
    let response = dispatcher
        .dispatch(&mut request, &ctx)
        .map_err(|e| e.to_string())?;
    assert_eq!(
        request.data, masked,
        "on_fs_read did not redact the in-flight buffer"
    );
    match response.reply {
        FuseReply::Payload(bytes) => assert_eq!(
            bytes, masked,
            "guest read payload did not carry the masked content"
        ),
        other => return Err(format!("guest read did not return a payload: {other:?}")),
    }
    assert_eq!(
        std::fs::read(&config).map_err(|e| e.to_string())?,
        b"token = 'AKIAABCDEFGHIJKLMNOP'\n",
        "host disk file was mutated instead of masked in flight"
    );
    drop_workspace(&workspace);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_18_write_chunk_64kb_transparent_stream.yaml#case_id
struct CountingFs {
    writes: std::sync::Mutex<Vec<usize>>,
}

impl FsInterposer for CountingFs {
    fn on_fs_write(
        &self,
        _path: &Path,
        data: &mut [u8],
        _pid: u32,
        _ctx: &HandlerCtx,
    ) -> Result<
        kelir_interface::protocol_wire::FilterAction,
        kelir_interface::plugin_model::PluginError,
    > {
        let mut writes = self.writes.lock().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("counter poisoned".into())
        })?;
        writes.push(data.len());
        Ok(kelir_interface::protocol_wire::FilterAction::Forward)
    }
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_18_write_chunk_64kb_transparent_stream.yaml#case_id
#[test]
#[ignore]
fn sp_18_write_chunk_64kb_transparent_stream() -> Result<(), String> {
    let workspace = temp_workspace("sp18");
    let target = workspace.join("src/main.rs");
    std::fs::create_dir_all(workspace.join("src")).map_err(|e| e.to_string())?;
    std::fs::write(&target, b"fn main() {}\n").map_err(|e| e.to_string())?;
    let counter = Arc::new(CountingFs {
        writes: std::sync::Mutex::new(Vec::new()),
    });
    let interposer: Arc<dyn FsInterposer> = counter.clone();
    let dispatcher = FuseDispatcher::new(kelir_storage::fuse_ops(engine_with(interposer)));
    let ctx = HandlerCtx::new([0u8; 16], 42, (), Vec::new());
    let mut request = FuseRequest {
        opcode: FuseOpcode::Write,
        guest_inode: 7,
        path: target,
        pid: 42,
        data: vec![0u8; 262144],
        offset: 0,
        size: 262144,
        blocking: false,
    };
    dispatcher
        .dispatch(&mut request, &ctx)
        .map_err(|e| e.to_string())?;
    let writes = counter
        .writes
        .lock()
        .map_err(|_| "counter poisoned".to_string())?;
    if writes.len() != 4 {
        return Err(format!(
            "262144-byte guest write was not chunked into four 64KB on_fs_write calls: saw {} call(s) with sizes {writes:?}",
            writes.len()
        ));
    }
    assert_eq!(*writes, vec![65536, 65536, 65536, 65536]);
    drop_workspace(&workspace);
    Ok(())
}
