// see: cans/artifacts/test-plan/cases/attestation_receipts/ar_01_execution_receipt_blake3_binding.yaml#case_id
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use kelir::builder_sdk::KelirBuilder;
use kelir::interface::driver_interface::default_vm_config;
use kelir::interface::driver_interface::{
    ExternalSuspendKind, GuestClock, SnapshotDescriptor, SuspendReason,
};
use kelir::plugin_model::PluginError;

fn builder_config() -> kelir::interface::driver_interface::VmConfig {
    default_vm_config(
        None,
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    )
}

fn step_descriptor(offset_ns: u64) -> SnapshotDescriptor {
    SnapshotDescriptor {
        id: "snap-lc".to_string(),
        dirty_bytes: 0,
        created_at_ns: 0,
        parent_digest: None,
        guest_clock: Some(GuestClock {
            absolute_wall_clock_ns: 0,
            monotonic_offset_ns: offset_ns,
            timezone: "UTC".to_string(),
            boot_params: String::new(),
        }),
    }
}

// see: cans/artifacts/test-plan/cases/attestation_receipts/ar_01_execution_receipt_blake3_binding.yaml#case_id
#[tokio::test]
#[ignore]
async fn ar_01_execution_receipt_blake3_binding() -> Result<(), String> {
    let session = KelirBuilder::new(builder_config())
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    match session.spawn("/bin/true", &[]).await {
        Ok(result) => {
            assert_eq!(result.exit_code, 0);
        }
        Err(e) => {
            return Err(format!(
                "spawn /bin/true never produced an exec receipt: {e}"
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_04_draining_state_rejection_ebusy.yaml#case_id
#[tokio::test]
#[ignore]
async fn lc_04_draining_state_rejection_ebusy() -> Result<(), String> {
    let mut session = KelirBuilder::new(builder_config())
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    session.drain(5000).map_err(|e| e.to_string())?;
    match session.spawn("echo new_task", &[]).await {
        Err(PluginError::Halt(message)) => {
            assert!(
                message.contains("EBUSY"),
                "draining-state exec rejected with the wrong diagnostic: {message}"
            );
        }
        Err(e) => return Err(format!("draining-state exec gave wrong error: {e}")),
        Ok(_) => {
            return Err(
                "new exec accepted on a Draining microVM instead of refusing with EBUSY"
                    .to_string(),
            );
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_07_cow_fork_uncloned_descriptor_veto.yaml#case_id
#[tokio::test]
#[ignore]
async fn lc_07_cow_fork_uncloned_descriptor_veto() -> Result<(), String> {
    let mut builder = KelirBuilder::new(builder_config());
    builder
        .lifecycle_mut()
        .register_veto(kelir::plugin_model::VetoEvent::Fork, Box::new(|| false));
    let session = builder.boot().await.map_err(|e| e.to_string())?;
    match session.fork().await {
        Err(PluginError::Halt(message)) => {
            assert!(
                message.contains("ESessionVeto"),
                "vetoed fork aborted with the wrong diagnostic: {message}"
            );
        }
        Err(e) => return Err(format!("vetoed fork gave wrong error: {e}")),
        Ok(_) => {
            return Err(
                "fork succeeded and silently dropped the registered Fork veto instead of aborting with ESessionVeto"
                    .to_string(),
            )
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_01_cow_fork_warm_memory_sharing.yaml#case_id
#[tokio::test]
#[ignore]
async fn lc_01_cow_fork_warm_memory_sharing() -> Result<(), String> {
    let parent = KelirBuilder::new(builder_config())
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    let lineage = parent.lineage(0x11);
    let base = parent
        .checkpoint
        .create_step(lineage, step_descriptor(5_000_000_000))
        .map_err(|e| e.to_string())?;
    assert_eq!(base, 0);
    parent
        .checkpoint
        .commit_step(lineage)
        .map_err(|e| e.to_string())?;
    let child = parent.fork().await.map_err(|e| e.to_string())?;
    match child.checkpoint.head(lineage) {
        Some(head) => assert_eq!(head, base),
        None => {
            return Err(
                "fork handed the child a fresh checkpoint plane; the committed parent chain (warm memory state) is not inherited"
                    .to_string(),
            );
        }
    }
    assert_eq!(child.metrics().snapshot().dirty_pages, 0);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_02_suspend_zero_rss_purge.yaml#case_id
#[tokio::test]
#[ignore]
async fn lc_02_suspend_zero_rss_purge() -> Result<(), String> {
    let mut session = KelirBuilder::new(builder_config())
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    let before = match session.host.refresh_rss() {
        Some(bytes) => bytes,
        None => return Err("host RSS unobservable on this platform".to_string()),
    };
    session
        .suspend(SuspendReason::External(
            ExternalSuspendKind::HumanAuthorizationHold,
        ))
        .await
        .map_err(|e| e.to_string())?;
    let after = match session.host.refresh_rss() {
        Some(bytes) => bytes,
        None => return Err("host RSS unobservable after suspend".to_string()),
    };
    if after >= before {
        return Err(format!(
            "suspend left host RSS unpurged: {before} -> {after} bytes; no page-dump flush to disk ran"
        ));
    }
    assert!(
        after < 15 * 1024 * 1024,
        "host RSS above the supervisor floor after suspend: {after}"
    );
    Ok(())
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_09_graceful_shutdown_syncfs_flushed.yaml#case_id
#[tokio::test]
#[ignore]
async fn lc_09_graceful_shutdown_syncfs_flushed() -> Result<(), String> {
    let fired = Arc::new(AtomicBool::new(false));
    let observer = Arc::clone(&fired);
    let mut session = KelirBuilder::new(builder_config())
        .on_vm_shutdown(move |_ctx| {
            let observer = Arc::clone(&observer);
            Box::pin(async move {
                observer.store(true, Ordering::SeqCst);
                Ok(())
            })
        })
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    session
        .stop_with_timeout(std::time::Duration::from_secs(5))
        .await
        .map_err(|e| e.to_string())?;
    if !fired.load(Ordering::SeqCst) {
        return Err(
            "graceful stop_with_timeout returned without firing on_vm_shutdown; no MSG_SHUTDOWN_REQUEST -> syncfs -> ACK Terminate flush exists"
                .to_string(),
        );
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_10_abrupt_kill_dirty_shutdown_receipt.yaml#case_id
#[tokio::test]
#[ignore]
async fn lc_10_abrupt_kill_dirty_shutdown_receipt() -> Result<(), String> {
    let fired = Arc::new(AtomicBool::new(false));
    let observer = Arc::clone(&fired);
    let mut session = KelirBuilder::new(builder_config())
        .on_vm_shutdown(move |_ctx| {
            let observer = Arc::clone(&observer);
            Box::pin(async move {
                observer.store(true, Ordering::SeqCst);
                Ok(())
            })
        })
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    session.kill().map_err(|e| e.to_string())?;
    if !fired.load(Ordering::SeqCst) {
        return Err(
            "session.kill() returned without firing on_vm_shutdown; no abrupt SIGKILL teardown (ExitKind::HardKilled receipt path) exists"
                .to_string(),
        );
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_12_host_reaper_pdeathsig_enforcement.yaml#case_id
#[tokio::test]
#[ignore]
async fn lc_12_host_reaper_pdeathsig_enforcement() -> Result<(), String> {
    let session = KelirBuilder::new(builder_config())
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    let mut parent = std::process::Command::new("sh")
        .arg("-c")
        .arg("sleep 30")
        .spawn()
        .map_err(|e| e.to_string())?;
    let armed = session.host.arm_host_reaper(parent.id());
    let _ = parent.kill();
    let _ = parent.wait();
    match armed {
        Ok(()) => {}
        Err(PluginError::Halt(message)) => {
            return Err(format!(
                "arm_host_reaper refused to arm the platform reaper: {message}"
            ));
        }
        Err(e) => return Err(format!("arm_host_reaper gave wrong error: {e}")),
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_14_detached_session_lease_heartbeat.yaml#case_id
#[tokio::test]
#[ignore]
async fn lc_14_detached_session_lease_heartbeat() -> Result<(), String> {
    let session = KelirBuilder::new(builder_config())
        .detached(true)
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    let db =
        kelir_catalog::CatalogDb::open(kelir_catalog::catalog_path().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    match db
        .lookup_session(&session.session_id())
        .map_err(|e| e.to_string())?
    {
        None => return Err("detached session registered no catalog row".to_string()),
        Some(row) => match row.lease_kind {
            kelir_catalog::schema_catalog::LeaseKind::Lease => match row.expires_at_unix {
                Some(_) => {}
                None => return Err("detached lease row carries no expires_at TTL".to_string()),
            },
            other => {
                return Err(format!(
                    "detached boot kept lease_kind={other:?} expires_at={:?}; no lease TTL row for the runner heartbeat",
                    row.expires_at_unix
                ));
            }
        },
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_15_detached_session_reconnect_relay.yaml#case_id
#[tokio::test]
#[ignore]
async fn lc_15_detached_session_reconnect_relay() -> Result<(), String> {
    let session = KelirBuilder::new(builder_config())
        .detached(true)
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    let id = session.session_id();
    let relay = kelir_core::Session::relay_socket_path(&id).map_err(|e| e.to_string())?;
    if !relay.exists() {
        return Err(format!(
            "detached session never created its relay socket at {}",
            relay.display()
        ));
    }
    std::os::unix::net::UnixStream::connect(&relay)
        .map_err(|e| format!("relay connect failed: {e}"))?;
    match kelir::session_lifecycle::Session::attach(&id).await {
        Ok(_) => {}
        Err(e) => return Err(format!("reconnect attach failed: {e}")),
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/attestation_receipts/ar_05_erofs_trusted_prefix_fast_path.yaml#case_id
#[tokio::test]
#[ignore]
async fn ar_05_erofs_trusted_prefix_fast_path() -> Result<(), String> {
    let session = KelirBuilder::new(builder_config())
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    match session.spawn("/usr/bin/env", &[]).await {
        Ok(result) => assert_eq!(result.exit_code, 0),
        Err(e) => {
            return Err(format!(
                "/usr/bin/env trusted-prefix resolution never completed (pre-computed BLAKE3 table in host RAM, zero disk reads): {e}"
            ));
        }
    }
    Ok(())
}

// see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
#[tokio::test]
async fn sandbox_facade_fails_closed_before_boot() -> Result<(), String> {
    let mut sandbox = kelir::sandbox_runtime::Sandbox::create(
        kelir::sandbox_runtime::SandboxOptions::new(builder_config()),
    )
    .await
    .map_err(|e| e.to_string())?;
    match sandbox.resume("/tmp/snap").await {
        Err(PluginError::Halt(message)) if message.contains("Sandbox::resume before boot") => {}
        other => return Err(format!("resume before boot gave {other:?}")),
    }
    match sandbox.exec("/bin/true", &[]).await {
        Err(PluginError::Halt(message)) if message.contains("Sandbox::exec before boot") => {}
        other => return Err(format!("exec before boot gave {other:?}")),
    }
    match sandbox
        .stop_with_timeout(std::time::Duration::from_secs(1))
        .await
    {
        Err(PluginError::Halt(message))
            if message.contains("Sandbox::stop_with_timeout before boot") => {}
        other => return Err(format!("stop_with_timeout before boot gave {other:?}")),
    }
    match sandbox.terminate().await {
        Err(PluginError::Halt(message)) if message.contains("Sandbox::terminate before boot") => {}
        other => return Err(format!("terminate before boot gave {other:?}")),
    }
    Ok(())
}
