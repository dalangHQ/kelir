// see: cans/11-protocol.md#Wire-framing
// see: cans/artifacts/config/project-structure.txt#kelir-shim
// see: cans/artifacts/code/wire-protocol.rs#MuxFrameHeader

// see: cans/11-protocol.md#Wire-framing
pub use kelir_interface::codec_protocol::{
    FLAG_CLOSE, FLAG_DATA, FLAG_OPEN, FLAG_RST, FLAG_WINDOW_UPDATE, HEADER_LEN,
    MAX_CONTROL_PAYLOAD, MAX_FRAME_PAYLOAD, MAX_STREAM_ID, decode_header, decode_msg_type,
    encode_msg_frame,
};

// see: cans/11-protocol.md#Binary-exec-channel
#[allow(dead_code)]
pub fn decode_exec_sub_lane(payload: &[u8]) -> Result<(u8, &[u8]), ()> {
    let Some((&lane, rest)) = payload.split_first() else {
        return Err(());
    };
    Ok((lane, rest))
}

// see: cans/11-protocol.md#Binary-exec-channel
#[allow(dead_code)]
pub fn decode_signal(body: &[u8]) -> Result<(u32, u64), ()> {
    if body.len() != 12 {
        return Err(());
    }
    let signal = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
    let exec_id = u64::from_le_bytes([
        body[4], body[5], body[6], body[7], body[8], body[9], body[10], body[11],
    ]);
    Ok((signal, exec_id))
}

// see: cans/11-protocol.md#Control-channel
#[allow(dead_code)]
pub fn check_control_payload(payload: &[u8]) -> Result<(), ()> {
    if payload.len() > MAX_CONTROL_PAYLOAD {
        return Err(());
    }
    Ok(())
}
