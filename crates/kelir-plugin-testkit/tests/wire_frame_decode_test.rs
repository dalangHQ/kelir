// see: cans/artifacts/test-plan/cases/protocol_mux/pm_11_wire_framing_checked_math_validation.yaml#case_id
use kelir_interface::codec_protocol::FLAG_DATA;
use kelir_interface::protocol_wire::{Channel, MsgType};
use kelir_plugin_testkit::vectors_testkit::WireFrame;

fn frame() -> WireFrame {
    WireFrame {
        transport: Channel::Control,
        stream_id: 7,
        flags: FLAG_DATA,
        msg_type: MsgType::Trap,
        payload: b"trap-payload".to_vec(),
    }
}

#[test]
fn wire_frame_decode_round_trips_encode() -> Result<(), String> {
    let frame = frame();
    let bytes = frame.encode().map_err(|e| e.to_string())?;
    let decoded = WireFrame::decode(Channel::Control, &bytes).map_err(|e| e.to_string())?;
    assert_eq!(decoded.stream_id, frame.stream_id);
    assert_eq!(decoded.flags, frame.flags);
    assert_eq!(decoded.msg_type, frame.msg_type);
    assert_eq!(decoded.payload, frame.payload);
    Ok(())
}

#[test]
fn wire_frame_decode_rejects_short_buffer() {
    assert!(WireFrame::decode(Channel::Control, &[0u8; 4]).is_err());
}

#[test]
fn wire_frame_decode_rejects_truncated_body() -> Result<(), String> {
    let mut bytes = frame().encode().map_err(|e| e.to_string())?;
    bytes.truncate(12);
    assert!(WireFrame::decode(Channel::Control, &bytes).is_err());
    Ok(())
}

#[test]
fn wire_frame_decode_rejects_unknown_msg_type() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.push(FLAG_DATA);
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&0xFFFFu16.to_le_bytes());
    assert!(WireFrame::decode(Channel::Control, &bytes).is_err());
}
