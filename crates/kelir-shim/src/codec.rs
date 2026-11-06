#![forbid(unsafe_code)]

// see: cans/11-protocol.md#Wire-framing
// see: cans/artifacts/config/project-structure.txt#kelir-shim

#[allow(dead_code)]
pub const HEADER_LEN: usize = 9;

#[allow(dead_code)]
pub fn encode_frame(
    stream_id: u32,
    flags: u8,
    payload: &[u8],
    out: &mut [u8],
) -> Result<usize, ()> {
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
    Ok((stream_id, flags, len))
}
