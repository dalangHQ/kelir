// see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
use kelir_interface::handles_lifecycle::VmHandle;
use kelir_interface::plugin_model::KelirPlugin;
use plugin_wire_ingress::{L4ForwardFrame, MAX_PARKED_BYTES, ParkOutcome, WireIngressPlugin};

fn l4_frame_bytes(target_port: u16, connection_id: u64, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&target_port.to_le_bytes());
    bytes.extend_from_slice(&connection_id.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(payload.len()).unwrap_or(0).to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

// see: cans/plugins/wire-ingress.md#Ingress-stream-framing-on-stream-kind-L4-FORWARD-(mux-0x0001):
#[test]
fn wire_ingress_l4_forward_stream_handler_fires_on_stream_kind() -> Result<(), String> {
    use kelir_interface::protocol_wire::{FilterAction, StreamKind};
    let Some(kind) = StreamKind::from_u16(0x0005) else {
        return Err("L4_FORWARD stream kind 0x0005 is unmapped".into());
    };
    if plugin_wire_ingress::L4_FORWARD_STREAM_KIND != kind {
        return Err("wire-ingress must handle stream kind L4_FORWARD 0x0005".into());
    }
    let plugin = WireIngressPlugin::new();
    let mut truncated = l4_frame_bytes(8443, 7, b"GET /");
    truncated.truncate(9);
    match plugin.on_l4_forward_frame(&mut truncated) {
        Ok(FilterAction::Drop { reason, .. }) if reason.contains("L4_FORWARD") => (),
        other => {
            return Err(format!(
                "malformed L4_FORWARD frame must drop closed, got {other:?}"
            ));
        }
    }
    let mut payload = l4_frame_bytes(8443, 9, b"GET / HTTP/1.1\r\n\r\n");
    match plugin.on_l4_forward_frame(&mut payload) {
        Ok(FilterAction::Forward) => (),
        other => {
            return Err(format!(
                "well-formed L4_FORWARD frame outside parking must forward, got {other:?}"
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/wire_plane/wp_17_wire_ingress_park_connection_suspends_session.yaml#case_id
#[tokio::test]
async fn wp_17_wire_ingress_park_connection_suspends_session() -> Result<(), String> {
    assert_eq!(L4ForwardFrame::decode(&[0u8; 5]), None);
    let short = l4_frame_bytes(8443, 7, b"GET /");
    let mut truncated = short.clone();
    truncated[10] = 99;
    assert_eq!(L4ForwardFrame::decode(&truncated), None);
    let payload = b"GET / HTTP/1.1\r\n\r\n";
    let bytes = l4_frame_bytes(8443, 7, payload);
    let decoded = match L4ForwardFrame::decode(&bytes) {
        Some(decoded) => decoded,
        None => return Err("valid L4_FORWARD frame failed to decode".into()),
    };
    assert_eq!(decoded.0.target_port, 8443);
    assert_eq!(decoded.0.connection_id, 7);
    assert_eq!(decoded.0.payload_len, 18);
    assert_eq!(decoded.1, payload);
    let plugin = WireIngressPlugin::new();
    let router = plugin.context();
    assert_eq!(
        router
            .park_connection(decoded.0, decoded.1)
            .map_err(|err| format!("{err:?}"))?,
        ParkOutcome::Parked
    );
    assert_eq!(
        router
            .park_connection(decoded.0, b"more-bytes")
            .map_err(|err| format!("{err:?}"))?,
        ParkOutcome::Parked
    );
    let oversize = vec![0u8; MAX_PARKED_BYTES + 1];
    let big_frame = L4ForwardFrame {
        target_port: 8443,
        connection_id: 8,
        payload_len: u32::try_from(oversize.len()).unwrap_or(0),
    };
    assert_eq!(
        router
            .park_connection(big_frame, &oversize)
            .map_err(|err| format!("{err:?}"))?,
        ParkOutcome::Overflow
    );
    assert_eq!(
        router
            .reset_parked_connections()
            .map_err(|err| format!("{err:?}"))?,
        vec![7]
    );
    assert_eq!(
        router
            .reset_parked_connections()
            .map_err(|err| format!("{err:?}"))?,
        Vec::<u64>::new()
    );
    assert!(!router.is_parking().map_err(|err| format!("{err:?}"))?);
    let unbound = VmHandle::default();
    assert!(!unbound.is_bound());
    let drained = router
        .resume_parked(&unbound, "suspend.img")
        .await
        .map_err(|err| format!("{err:?}"))?;
    assert_eq!(drained, Vec::<plugin_wire_ingress::ParkedDrain>::new());
    Ok(())
}
