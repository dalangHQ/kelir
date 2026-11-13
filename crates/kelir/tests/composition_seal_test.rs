// see: cans/01-architecture.md#Host-runtime
use std::path::PathBuf;
use std::sync::Arc;

use kelir_engine::Session;
use kelir_engine::dispatch_plugin::PipelineEngine;
use kelir_engine::session_lifecycle::PlaneWiring;
use kelir_interface::driver_interface::default_vm_config;
use kelir_interface::plugin_model::{FsInterposer, HandlerCtx, MountScope, PipelineBinder};
use kelir_interface::storage::{FuseOpcode, FuseReply, FuseRequest, SyntheticFile};

fn boot_config() -> kelir_interface::driver_interface::VmConfig {
    default_vm_config(
        None,
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    )
}

fn full_wiring() -> PlaneWiring {
    let metrics = || Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new());
    let dns_ops: Arc<dyn kelir_interface::wire::dns::DnsOps> = kelir_wire::dns_ops();
    PlaneWiring {
        terminal_ops: Some(kelir_terminal::terminal_ops(metrics())),
        checkpoint_ops: Some(kelir_checkpoint::checkpoint_ops(metrics())),
        host_ops: Some(kelir_host::host_ops(metrics())),
        net_ops: Some(kelir_wire::net_ops(Arc::clone(&dns_ops))),
        dns_ops: Some(dns_ops),
        udp_nat_ops: Some(kelir_wire::udp_nat_ops()),
        l4_ops: Some(kelir_wire::l4_ops()),
        storage_io: Some(kelir_storage_io::storage_io()),
        ..Default::default()
    }
}

struct PassFs;

impl FsInterposer for PassFs {}

// see: cans/01-architecture.md#Host-runtime
#[test]
fn unbound_terminal_ops_fails_boot() -> Result<(), String> {
    let wiring = PlaneWiring {
        terminal_ops: None,
        ..full_wiring()
    };
    match Session::boot_with_plugins(boot_config(), &[], None, Vec::new(), wiring) {
        Err(kelir_interface::plugin_model::PluginError::Halt(message)) => {
            assert!(
                message.contains("TerminalOps unbound at boot"),
                "missing TerminalOps gave the wrong diagnostic: {message}"
            );
            Ok(())
        }
        Err(error) => Err(format!("missing TerminalOps gave the wrong error: {error}")),
        Ok(_) => Err("boot accepted a session with TerminalOps unbound".into()),
    }
}

// see: cans/01-architecture.md#Host-runtime
#[test]
fn unbound_net_ops_fails_boot() -> Result<(), String> {
    let wiring = PlaneWiring {
        net_ops: None,
        ..full_wiring()
    };
    match Session::boot_with_plugins(boot_config(), &[], None, Vec::new(), wiring) {
        Err(kelir_interface::plugin_model::PluginError::Halt(message)) => {
            assert!(
                message.contains("NetOps unbound at boot"),
                "missing NetOps gave the wrong diagnostic: {message}"
            );
            Ok(())
        }
        Err(error) => Err(format!("missing NetOps gave the wrong error: {error}")),
        Ok(_) => Err("boot accepted a session with NetOps unbound".into()),
    }
}

// see: cans/01-architecture.md#Host-runtime
#[test]
fn unbound_fuse_ops_has_no_dispatcher() -> Result<(), String> {
    let session = Session::boot_with_plugins(boot_config(), &[], None, Vec::new(), full_wiring())
        .map_err(|error| error.to_string())?;
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
    match session.dispatch_fuse(&mut lookup, &ctx) {
        Err(kelir_interface::plugin_model::PluginError::Halt(message)) => {
            assert!(
                message.contains("FuseOps unbound at boot"),
                "unbound FuseOps gave the wrong diagnostic: {message}"
            );
            Ok(())
        }
        Err(error) => Err(format!("unbound FuseOps gave the wrong error: {error}")),
        Ok(_) => Err("FUSE dispatched with FuseOps unbound".into()),
    }
}

// see: cans/01-architecture.md#Host-runtime
#[test]
fn bound_fuse_ops_dispatches_synthetic_lookup() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    engine.bind_fs_scoped(Arc::new(PassFs), MountScope::default());
    let fuse_ops = kelir_storage::fuse_ops_with_synthetic_files(
        Arc::new(engine),
        vec![SyntheticFile::static_(".env", "SECRET=<redacted>")],
    );
    let wiring = PlaneWiring {
        fuse_ops: Some(fuse_ops),
        ..full_wiring()
    };
    let session = Session::boot_with_plugins(boot_config(), &[], None, Vec::new(), wiring)
        .map_err(|error| error.to_string())?;
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
    match session
        .dispatch_fuse(&mut lookup, &ctx)
        .map_err(|error| error.to_string())?
        .reply
    {
        FuseReply::Inode(attr) => {
            assert_eq!(attr.mode, 0o400);
            assert_eq!(attr.size, 17);
            assert_eq!(attr.content, b"SECRET=<redacted>");
            Ok(())
        }
        other => Err(format!(
            "bound FuseOps did not return the synthetic inode: {other:?}"
        )),
    }
}
