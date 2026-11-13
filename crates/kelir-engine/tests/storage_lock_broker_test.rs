// see: cans/artifacts/test-plan/cases/storage_plane/sp_15_host_inode_lock_broker_flock_bridge.yaml#case_id
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kelir_engine::dispatch_plugin::PipelineEngine;
use kelir_engine::planes::storage::LockBrokerWiring;
use kelir_engine::planes::storage::lock_broker::{LockBroker, LockKey, LockKind, LockRequest};
use kelir_interface::plugin_model::{
    FsInterposer, HandlerCtx, MountScope, PipelineBinder, PluginError,
};

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

// see: cans/artifacts/test-plan/cases/storage_plane/sp_15_host_inode_lock_broker_flock_bridge.yaml#case_id
#[test]
#[ignore]
fn sp_15_host_inode_lock_broker_flock_bridge() -> Result<(), String> {
    let workspace = temp_workspace("sp15");
    let target = workspace.join("index.lock");
    std::fs::write(&target, b"staged index\n").map_err(|e| e.to_string())?;
    let broker = LockBroker::with_wiring(
        engine_with(Arc::new(PassFs)),
        LockBrokerWiring {
            scope_globs: vec!["**/*".to_string()],
            non_blocking: false,
        },
    );
    let ctx = HandlerCtx::new([0u8; 16], 42, (), Vec::new());
    let request = LockRequest {
        namespace: [0x15; 16],
        guest_inode: 7,
        host_path: target.clone(),
        kind: LockKind::Exclusive,
        blocking: true,
        pid: 42,
    };
    let outcome = broker.acquire(&request, &ctx);
    let grant = match outcome {
        Ok(grant) => grant,
        Err(PluginError::Halt(message)) => {
            drop_workspace(&workspace);
            return Err(format!("lock broker acquire refused: {message}"));
        }
        Err(e) => {
            drop_workspace(&workspace);
            return Err(format!("lock broker acquire gave wrong error: {e}"));
        }
    };
    assert_eq!(grant.kind, LockKind::Exclusive);
    let probe = std::process::Command::new("flock")
        .arg("-n")
        .arg(&target)
        .arg("true")
        .output()
        .map_err(|e| e.to_string())?;
    if probe.status.success() {
        drop_workspace(&workspace);
        return Err(
            "external host contender acquired the same inode; the broker holds no real host flock"
                .to_string(),
        );
    }
    broker
        .release(LockKey {
            namespace: request.namespace,
            guest_inode: 7,
        })
        .map_err(|e| e.to_string())?;
    drop_workspace(&workspace);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_19_host_inode_flock_contention_ewouldblock.yaml#case_id
#[test]
#[ignore]
fn sp_19_host_inode_flock_contention_ewouldblock() -> Result<(), String> {
    let workspace = temp_workspace("sp19");
    let target = workspace.join("index.lock");
    std::fs::write(&target, b"staged index\n").map_err(|e| e.to_string())?;
    let mut holder = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "exec 9<>{}; flock 9; exec sleep 30",
            target.display()
        ))
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut armed = false;
    for _ in 0..8192 {
        let probe = std::process::Command::new("flock")
            .arg("-n")
            .arg(&target)
            .arg("true")
            .output()
            .map_err(|e| e.to_string())?;
        if !probe.status.success() {
            armed = true;
            break;
        }
        std::thread::yield_now();
    }
    if !armed {
        let _ = holder.kill();
        let _ = holder.wait();
        drop_workspace(&workspace);
        return Err("external host holder never armed its flock".to_string());
    }
    let broker = LockBroker::with_wiring(
        engine_with(Arc::new(PassFs)),
        LockBrokerWiring {
            scope_globs: vec!["**/*".to_string()],
            non_blocking: true,
        },
    );
    let ctx = HandlerCtx::new([0u8; 16], 42, (), Vec::new());
    let request = LockRequest {
        namespace: [0x19; 16],
        guest_inode: 7,
        host_path: target.clone(),
        kind: LockKind::Exclusive,
        blocking: true,
        pid: 42,
    };
    let outcome = broker.acquire(&request, &ctx);
    let _ = holder.kill();
    let _ = holder.wait();
    match outcome {
        Err(PluginError::Halt(message)) => assert!(
            message.contains("EWOULDBLOCK"),
            "contention denial carried the wrong diagnostic: {message}"
        ),
        Err(e) => {
            drop_workspace(&workspace);
            return Err(format!("contention gave wrong error: {e}"));
        }
        Ok(_) => {
            drop_workspace(&workspace);
            return Err("contention silently granted the externally held lock".to_string());
        }
    }
    drop_workspace(&workspace);
    Ok(())
}
