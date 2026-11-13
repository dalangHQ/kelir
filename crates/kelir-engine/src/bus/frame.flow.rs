// see: cans/11-protocol.md#Wire-framing
use kelir_interface::codec_protocol::{CodecError, HEADER_LEN, decode_header};
use kelir_interface::plugin_model::PluginError;
use kelir_transport::Transport;

// see: cans/11-protocol.md#Wire-framing
pub(super) fn codec_halt(err: CodecError) -> PluginError {
    PluginError::Halt(err.to_string())
}

// see: cans/11-protocol.md#Wire-framing
pub(super) fn take_frame(rx: &mut Vec<u8>) -> Result<Option<(u32, u8, Vec<u8>)>, PluginError> {
    if rx.len() < HEADER_LEN {
        return Ok(None);
    }
    let header = decode_header(&rx[..HEADER_LEN]).map_err(codec_halt)?;
    let total = header.total_len().map_err(codec_halt)?;
    if rx.len() < total {
        return Ok(None);
    }
    let payload = rx[HEADER_LEN..total].to_vec();
    rx.drain(..total);
    Ok(Some((header.stream_id, header.flags, payload)))
}

// see: cans/11-protocol.md#Wire-framing
pub(super) fn write_all_frame(
    transport: &mut kelir_transport::spsc_ring::SpscRingTransport,
    frame: &[u8],
) -> Result<(), PluginError> {
    let mut written = 0usize;
    while written < frame.len() {
        let count = transport.write(&frame[written..])?;
        written += count;
    }
    Ok(())
}
