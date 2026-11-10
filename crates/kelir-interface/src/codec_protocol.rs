// see: cans/11-protocol.md#Wire-framing
// see: cans/artifacts/code/wire-protocol.rs#MuxFrameHeader
use std::fmt;

use crate::protocol_wire::{Channel, MsgType};

// see: cans/11-protocol.md#Wire-framing
pub const MAX_FRAME_PAYLOAD: usize = 64 * 1024;
// see: cans/11-protocol.md#Control-channel
pub const MAX_CONTROL_PAYLOAD: usize = 4 * 1024;
// see: cans/11-protocol.md#Wire-framing
pub const MAX_STREAM_ID: u32 = 0x00FF_FFFF;

// see: cans/11-protocol.md#Wire-framing
pub const HEADER_LEN: usize = 9;

// see: cans/11-protocol.md#Wire-framing
pub struct MuxFrameHeader {
    pub stream_id: u32,
    pub flags: u8,
    pub len: u32,
}

// see: cans/11-protocol.md#Wire-framing
pub const FLAG_OPEN: u8 = 0x01;
// see: cans/11-protocol.md#Stream-mux-framing
pub const FLAG_DATA: u8 = 0x02;
// see: cans/11-protocol.md#Stream-mux-framing
pub const FLAG_CLOSE: u8 = 0x04;
// see: cans/11-protocol.md#Stream-mux-framing
pub const FLAG_RST: u8 = 0x08;
// see: cans/11-protocol.md#Stream-mux-framing
pub const FLAG_WINDOW_UPDATE: u8 = 0x10;

// see: cans/11-protocol.md#Wire-framing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecError {
    StreamIdOutOfRange { stream_id: u32 },
    FrameTooLarge { len: usize },
    BufferTooSmall { needed: usize, have: usize },
    LengthOverflow,
    UnknownMsgType { msg_type: u16 },
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::StreamIdOutOfRange { stream_id } => {
                write!(f, "stream id {stream_id} exceeds 0x00FFFFFF")
            }
            CodecError::FrameTooLarge { len } => {
                write!(f, "frame payload {len} exceeds the 64KB frame budget")
            }
            CodecError::BufferTooSmall { needed, have } => {
                write!(
                    f,
                    "wire buffer holds {have} bytes; the frame layout needs {needed}"
                )
            }
            CodecError::LengthOverflow => write!(f, "frame length arithmetic overflow"),
            CodecError::UnknownMsgType { msg_type } => {
                write!(f, "msg_type {msg_type:#06x} is not a wired MsgType")
            }
        }
    }
}

impl std::error::Error for CodecError {}

impl MuxFrameHeader {
    // see: cans/11-protocol.md#Wire-framing
    pub fn total_len(&self) -> Result<usize, CodecError> {
        let len = usize::try_from(self.len).map_err(|_| CodecError::LengthOverflow)?;
        HEADER_LEN
            .checked_add(len)
            .ok_or(CodecError::LengthOverflow)
    }
}

// see: cans/11-protocol.md#Wire-framing
fn validate_frame(stream_id: u32, payload_len: usize) -> Result<u32, CodecError> {
    if stream_id > MAX_STREAM_ID {
        return Err(CodecError::StreamIdOutOfRange { stream_id });
    }
    if payload_len > MAX_FRAME_PAYLOAD {
        return Err(CodecError::FrameTooLarge { len: payload_len });
    }
    u32::try_from(payload_len).map_err(|_| CodecError::FrameTooLarge { len: payload_len })
}

// see: cans/11-protocol.md#Wire-framing
fn write_header(stream_id: u32, flags: u8, len: u32, out: &mut Vec<u8>) {
    out.extend_from_slice(&stream_id.to_le_bytes());
    out.push(flags);
    out.extend_from_slice(&len.to_le_bytes());
}

// see: cans/11-protocol.md#Wire-framing
pub fn encode_frame(
    stream_id: u32,
    flags: u8,
    payload: &[u8],
    out: &mut Vec<u8>,
) -> Result<(), CodecError> {
    let len = validate_frame(stream_id, payload.len())?;
    write_header(stream_id, flags, len, out);
    out.extend_from_slice(payload);
    Ok(())
}

// see: cans/11-protocol.md#Wire-framing
pub fn encode_msg_frame(
    stream_id: u32,
    flags: u8,
    msg_type: MsgType,
    payload: &[u8],
    out: &mut Vec<u8>,
) -> Result<(), CodecError> {
    let body_len = payload
        .len()
        .checked_add(2)
        .ok_or(CodecError::FrameTooLarge { len: usize::MAX })?;
    let len = validate_frame(stream_id, body_len)?;
    write_header(stream_id, flags, len, out);
    out.extend_from_slice(&(msg_type as u16).to_le_bytes());
    out.extend_from_slice(payload);
    Ok(())
}

// see: cans/11-protocol.md#Wire-framing
pub fn decode_header(buf: &[u8]) -> Result<MuxFrameHeader, CodecError> {
    if buf.len() < HEADER_LEN {
        return Err(CodecError::BufferTooSmall {
            needed: HEADER_LEN,
            have: buf.len(),
        });
    }
    let stream_id = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let flags = buf[4];
    let len = u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
    if stream_id > MAX_STREAM_ID {
        return Err(CodecError::StreamIdOutOfRange { stream_id });
    }
    let len = usize::try_from(len).map_err(|_| CodecError::LengthOverflow)?;
    if len > MAX_FRAME_PAYLOAD {
        return Err(CodecError::FrameTooLarge { len });
    }
    Ok(MuxFrameHeader {
        stream_id,
        flags,
        len: len as u32,
    })
}

// see: cans/11-protocol.md#Wire-framing
pub fn decode_msg_type(payload: &[u8]) -> Result<(u16, &[u8]), CodecError> {
    if payload.len() < 2 {
        return Err(CodecError::BufferTooSmall {
            needed: 2,
            have: payload.len(),
        });
    }
    let msg_type = u16::from_le_bytes([payload[0], payload[1]]);
    Ok((msg_type, &payload[2..]))
}

// see: cans/11-protocol.md#Wire-framing
// see: cans/02-plugin-model.md#Unified-verdict
#[derive(Debug, Clone)]
pub struct WireFrame {
    pub transport: Channel,
    pub stream_id: u32,
    pub flags: u8,
    pub msg_type: MsgType,
    pub payload: Vec<u8>,
}

impl WireFrame {
    // see: cans/11-protocol.md#Wire-framing
    pub fn encode(&self) -> Result<Vec<u8>, CodecError> {
        let mut out = Vec::with_capacity(HEADER_LEN + 2 + self.payload.len());
        encode_msg_frame(
            self.stream_id,
            self.flags,
            self.msg_type,
            &self.payload,
            &mut out,
        )?;
        Ok(out)
    }

    // see: cans/11-protocol.md#Wire-framing
    pub fn decode(transport: Channel, frame: &[u8]) -> Result<Self, CodecError> {
        let header = decode_header(frame)?;
        let total = header.total_len()?;
        if frame.len() < total {
            return Err(CodecError::BufferTooSmall {
                needed: total,
                have: frame.len(),
            });
        }
        let (msg_type, payload) = decode_msg_type(&frame[HEADER_LEN..total])?;
        let msg_type =
            MsgType::from_u16(msg_type).ok_or(CodecError::UnknownMsgType { msg_type })?;
        Ok(Self {
            transport,
            stream_id: header.stream_id,
            flags: header.flags,
            msg_type,
            payload: payload.to_vec(),
        })
    }
}
