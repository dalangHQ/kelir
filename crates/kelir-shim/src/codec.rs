// see: cans/11-protocol.md#Wire-framing
// see: cans/artifacts/config/project-structure.txt#kelir-shim
// see: cans/artifacts/code/wire-protocol.rs
// NOTE: these constants are local copies; crates/kelir-interface/src/codec.rs is the canonical home (lands at convergence).

#[allow(dead_code)]
pub const HEADER_LEN: usize = 9;

#[allow(dead_code)]
pub const MAX_FRAME_PAYLOAD: usize = 64 * 1024;

#[allow(dead_code)]
pub const MAX_CONTROL_PAYLOAD: usize = 4 * 1024;

#[allow(dead_code)]
pub const MAX_STREAM_ID: u32 = 0x00FF_FFFF;

#[allow(dead_code)]
pub const FLAG_OPEN: u8 = 0x01;

#[allow(dead_code)]
pub const FLAG_DATA: u8 = 0x02;

#[allow(dead_code)]
pub const FLAG_CLOSE: u8 = 0x04;

#[allow(dead_code)]
pub const FLAG_RST: u8 = 0x08;

#[allow(dead_code)]
pub const FLAG_WINDOW_UPDATE: u8 = 0x10;

// Hostile-input rule (wire-protocol.rs): checked_* arithmetic only; wire-derived numbers validated before any slicing; oversize payloads fail immediately.
#[allow(dead_code)]
pub fn encode_frame(
    stream_id: u32,
    flags: u8,
    payload: &[u8],
    out: &mut [u8],
) -> Result<usize, ()> {
    if stream_id > MAX_STREAM_ID {
        return Err(());
    }
    if payload.len() > MAX_FRAME_PAYLOAD {
        return Err(());
    }
    let payload_len: u32 = payload.len().try_into().map_err(|_| ())?;
    let total_len = HEADER_LEN.checked_add(payload.len()).ok_or(())?;
    if out.len() < total_len {
        return Err(());
    }
    out[0..4].copy_from_slice(&stream_id.to_le_bytes());
    out[4] = flags;
    out[5..9].copy_from_slice(&payload_len.to_le_bytes());
    out[HEADER_LEN..total_len].copy_from_slice(payload);
    Ok(total_len)
}

#[allow(dead_code)]
pub fn decode_header(buf: &[u8]) -> Result<(u32, u8, u32), ()> {
    if buf.len() < HEADER_LEN {
        return Err(());
    }
    let stream_id = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let flags = buf[4];
    let len = u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
    if stream_id > MAX_STREAM_ID {
        return Err(());
    }
    if usize::try_from(len).map_err(|_| ())? > MAX_FRAME_PAYLOAD {
        return Err(());
    }
    Ok((stream_id, flags, len))
}

// Control frames (transport 0x0000) carry at most MAX_CONTROL_PAYLOAD bytes; called by the control-channel split when it lands (L3).
#[allow(dead_code)]
pub fn check_control_payload(payload: &[u8]) -> Result<(), ()> {
    if payload.len() > MAX_CONTROL_PAYLOAD {
        return Err(());
    }
    Ok(())
}
