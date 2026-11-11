// see: cans/artifacts/test-plan/cases/wire_plane/wp_01_default_deny_ingress_drop.yaml#case_id
use std::net::TcpListener;

use kelir_core::Session;
use kelir_core::planes::wire::{
    FrameDirection, LinkLocalRoute, WirePlane, classify_frame_directional, link_local_route,
};
use kelir_interface::driver_interface::SnapshotDescriptor;
use kelir_interface::driver_interface::default_vm_config;
use kelir_interface::protocol_wire::FilterAction;
use kelir_interface::protocol_wire::StreamKind;

fn ipv4_frame(dst: [u8; 4], proto: u8, dst_port: u16) -> Vec<u8> {
    let mut frame = Vec::with_capacity(42);
    frame.extend_from_slice(&[0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
    frame.extend_from_slice(&[0x02, 0x00, 0x00, 0x00, 0x00, 0x02]);
    frame.extend_from_slice(&[0x08, 0x00]);
    frame.extend_from_slice(&[0x45, 0x00, 0x00, 0x1c, 0x00, 0x00, 0x00, 0x00, 0x40, proto]);
    frame.extend_from_slice(&[0x00, 0x00]);
    frame.extend_from_slice(&[10, 0, 0, 1]);
    frame.extend_from_slice(&dst);
    frame.extend_from_slice(&1234u16.to_be_bytes());
    frame.extend_from_slice(&dst_port.to_be_bytes());
    frame.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    frame
}

// see: cans/artifacts/test-plan/cases/wire_plane/wp_01_default_deny_ingress_drop.yaml#case_id
#[test]
fn wp_01_default_deny_ingress_drop() -> Result<(), String> {
    let mut frame = ipv4_frame([10, 0, 0, 2], 1, 0);
    assert_eq!(
        classify_frame_directional(&frame, FrameDirection::Inbound),
        kelir_core::planes::wire::FrameClass::NonTcpUdp
    );
    let mut wire = WirePlane::new();
    let action = wire
        .process_inbound_frame(&mut frame)
        .map_err(|e| e.to_string())?;
    match action {
        FilterAction::Drop { reason, .. } => {
            assert!(reason.contains("non-TCP/UDP"));
        }
        other => return Err(format!("ICMP frame was not dropped at ingress: {other:?}")),
    }
    assert_eq!(wire.metrics().snapshot().dropped_frames, 1);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/wire_plane/wp_05_udp_nat_full_cone_mapping.yaml#case_id
#[test]
fn wp_05_udp_nat_full_cone_mapping() -> Result<(), String> {
    let wire = WirePlane::new();
    wire.udp_nat_alloc(51820, 1000).map_err(|e| e.to_string())?;
    wire.udp_nat_alloc(51820, 1001).map_err(|e| e.to_string())?;
    assert_eq!(wire.udp_nat_slab_used().map_err(|e| e.to_string())?, 1);
    wire.udp_nat_alloc(51821, 1001).map_err(|e| e.to_string())?;
    assert_eq!(wire.udp_nat_slab_used().map_err(|e| e.to_string())?, 2);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/wire_plane/wp_06_udp_socket_reaper_slab_limit.yaml#case_id
#[test]
fn wp_06_udp_socket_reaper_slab_limit() -> Result<(), String> {
    let wire = WirePlane::new();
    for port in 0..2000u16 {
        wire.udp_nat_alloc(port, 1000).map_err(|e| e.to_string())?;
    }
    assert_eq!(wire.udp_nat_slab_used().map_err(|e| e.to_string())?, 1024);
    let reaped = wire
        .reap_stale_udp_sockets(1031)
        .map_err(|e| e.to_string())?;
    assert_eq!(reaped, 1024);
    assert_eq!(wire.udp_nat_slab_used().map_err(|e| e.to_string())?, 0);
    let kept = wire
        .reap_stale_udp_sockets(1000)
        .map_err(|e| e.to_string())?;
    assert_eq!(kept, 0);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/wire_plane/wp_07_link_local_quarantine_drop.yaml#case_id
#[test]
fn wp_07_link_local_quarantine_drop() -> Result<(), String> {
    let mut frame = ipv4_frame([169, 254, 1, 1], 6, 80);
    assert_eq!(
        link_local_route(&frame, FrameDirection::Outbound),
        Some(LinkLocalRoute::Drop)
    );
    let mut wire = WirePlane::new();
    let action = wire
        .process_outbound_frame(&mut frame)
        .map_err(|e| e.to_string())?;
    match action {
        FilterAction::Drop { reason, .. } => {
            assert!(reason.contains("link-local"));
        }
        other => return Err(format!("link-local frame was not dropped: {other:?}")),
    }
    assert_eq!(wire.metrics().snapshot().dropped_frames, 1);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/wire_plane/wp_08_imds_cloud_metadata_quarantine.yaml#case_id
#[test]
fn wp_08_imds_cloud_metadata_quarantine() -> Result<(), String> {
    let frame = ipv4_frame([169, 254, 169, 254], 6, 80);
    match link_local_route(&frame, FrameDirection::Outbound) {
        Some(LinkLocalRoute::VaultImds { vsock_port }) => assert_eq!(vsock_port, 80),
        other => return Err(format!("IMDS frame routed wrongly: {other:?}")),
    }
    let wire = WirePlane::new();
    assert!(!wire.imds_sink_bound());
    match wire.route_imds_to_vault(&frame) {
        Err(kelir_interface::plugin_model::PluginError::Halt(message)) => {
            assert!(message.contains("no vault vsock sink bound"));
        }
        Err(e) => return Err(format!("IMDS without sink gave wrong error: {e}")),
        Ok(_) => return Err("IMDS without sink forwarded".to_string()),
    }
    let azure = ipv4_frame([168, 63, 129, 16], 6, 80);
    assert_eq!(link_local_route(&azure, FrameDirection::Outbound), None);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/wire_plane/wp_14_port_forward_ephemeral_allocation.yaml#case_id
#[test]
fn wp_14_port_forward_ephemeral_allocation() -> Result<(), String> {
    let wire = WirePlane::new();
    let allocated = wire
        .forward_port(8080, Some(0))
        .map_err(|e| e.to_string())?;
    assert!(
        allocated > 1024,
        "ephemeral host port must be kernel-allocated above 1024"
    );
    assert_eq!(
        wire.forwarded_guest_port(allocated)
            .map_err(|e| e.to_string())?,
        Some(8080)
    );
    let second = wire
        .forward_port(9090, Some(0))
        .map_err(|e| e.to_string())?;
    assert!(
        second > 1024,
        "ephemeral host port must be kernel-allocated above 1024"
    );
    assert_ne!(
        second, allocated,
        "kernel must not hand the same ephemeral port twice"
    );
    assert_eq!(
        wire.forwarded_guest_port(second)
            .map_err(|e| e.to_string())?,
        Some(9090)
    );
    Ok(())
}

// see: cans/artifacts/test-plan/cases/wire_plane/wp_15_port_forward_collision_abort.yaml#case_id
#[test]
fn wp_15_port_forward_collision_abort() -> Result<(), String> {
    let wire = WirePlane::new();
    let first = wire
        .forward_port(8080, Some(9999))
        .map_err(|e| e.to_string())?;
    assert_eq!(first, 9999);
    match wire.forward_port(9090, Some(9999)) {
        Err(kelir_interface::plugin_model::PluginError::Halt(message)) => {
            assert!(message.contains("already forwarded"));
            assert!(message.contains("9999"));
        }
        Err(e) => return Err(format!("port collision gave wrong error: {e}")),
        Ok(_) => return Err("port collision silently fell back".to_string()),
    }
    assert_eq!(
        wire.forwarded_guest_port(9999).map_err(|e| e.to_string())?,
        Some(8080)
    );
    Ok(())
}

fn rollback_vm_config() -> kelir_interface::driver_interface::VmConfig {
    default_vm_config(
        None,
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    )
}

// see: cans/artifacts/test-plan/cases/wire_plane/wp_03_rollback_socket_termination_econnreset.yaml#case_id
#[tokio::test]
#[ignore]
async fn wp_03_rollback_socket_termination_econnreset() -> Result<(), String> {
    let mut session = Session::boot(rollback_vm_config()).map_err(|e| e.to_string())?;
    let snapshot = SnapshotDescriptor {
        id: "wp_03".to_string(),
        dirty_bytes: 0,
        created_at_ns: 0,
        parent_digest: None,
        guest_clock: None,
    };
    session
        .rollback_to(&snapshot)
        .await
        .map_err(|error| {
            format!(
                "rollback must tear down every host-side socket and emit RST so guest sends fail ECONNRESET: {error:?}"
            )
        })?;
    Ok(())
}

// see: cans/artifacts/test-plan/cases/wire_plane/wp_12_l4_diversion_passthrough_tls.yaml#case_id
#[test]
#[ignore]
fn wp_12_l4_diversion_passthrough_tls() -> Result<(), String> {
    let wire = WirePlane::new();
    match wire.divert_l4(8443, "127.0.0.1:9443".to_string()) {
        Ok(stream) => {
            assert_eq!(stream.kind, StreamKind::L4Forward);
            assert_eq!(stream.guest_port, 8443);
            assert_eq!(stream.host_target, "127.0.0.1:9443");
        }
        Err(error) => {
            return Err(format!(
                "divert_l4 must open the raw duplex L4-only stream (0x0005, no TLS termination) instead of erroring: {error:?}"
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/wire_plane/wp_16_port_forward_so_linger_zero_release.yaml#case_id
#[test]
fn wp_16_port_forward_so_linger_zero_release() -> Result<(), String> {
    let wire = WirePlane::new();
    let published = wire
        .forward_port(8080, Some(12345))
        .map_err(|e| e.to_string())?;
    assert_eq!(published, 12345);
    match TcpListener::bind(("127.0.0.1", 12345)) {
        Ok(listener) => {
            drop(listener);
            return Err(
                "forward_port published 12345 without holding a live host listener; SO_LINGER=0 teardown is unobservable"
                    .into(),
            );
        }
        Err(error) => match error.kind() {
            std::io::ErrorKind::AddrInUse => {}
            _ => {
                return Err(format!(
                    "rebind while the forward is active gave wrong error: {error}"
                ));
            }
        },
    }
    wire.quiesce().map_err(|e| e.to_string())?;
    match TcpListener::bind(("127.0.0.1", 12345)) {
        Ok(listener) => drop(listener),
        Err(error) => {
            return Err(format!(
                "port 12345 must be instantly re-bindable after SO_LINGER=0 teardown: {error}"
            ));
        }
    }
    Ok(())
}
