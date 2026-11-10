#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HexDecodeError {
    #[error("hex string has odd length")]
    OddLength,
    #[error("hex string length differs from the expected {expected_chars} chars")]
    LengthMismatch { expected_chars: usize },
    #[error("hex string carries non-hex byte {0}")]
    InvalidDigit(u8),
}

pub fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[usize::from(*byte >> 4)] as char);
        out.push(HEX[usize::from(*byte & 0x0f)] as char);
    }
    out
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn decode_hex(text: &str) -> Result<Vec<u8>, HexDecodeError> {
    let bytes = text.as_bytes();
    if bytes.len() & 1 != 0 {
        return Err(HexDecodeError::OddLength);
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks(2) {
        let high = hex_digit(pair[0]).ok_or(HexDecodeError::InvalidDigit(pair[0]))?;
        let low = hex_digit(pair[1]).ok_or(HexDecodeError::InvalidDigit(pair[1]))?;
        out.push((high << 4) | low);
    }
    Ok(out)
}

pub fn decode_hex_fixed<const N: usize>(text: &str) -> Result<[u8; N], HexDecodeError> {
    let bytes = text.as_bytes();
    if bytes.len() != N * 2 {
        return Err(HexDecodeError::LengthMismatch {
            expected_chars: N * 2,
        });
    }
    let mut out = [0u8; N];
    for (index, slot) in out.iter_mut().enumerate() {
        let high =
            hex_digit(bytes[index * 2]).ok_or(HexDecodeError::InvalidDigit(bytes[index * 2]))?;
        let low = hex_digit(bytes[index * 2 + 1])
            .ok_or(HexDecodeError::InvalidDigit(bytes[index * 2 + 1]))?;
        *slot = (high << 4) | low;
    }
    Ok(out)
}
