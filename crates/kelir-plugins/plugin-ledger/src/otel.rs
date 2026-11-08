// see: cans/plugins/ledger.md#OpenTelemetry
use kelir_interface::plugin::PluginError;

// see: cans/plugins/ledger.md#OpenTelemetry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceParent {
    pub trace_id: [u8; 16],
    pub parent_span_id: [u8; 8],
    pub flags: u8,
}

// see: cans/plugins/ledger.md#OpenTelemetry
const HEX: &[u8; 16] = b"0123456789abcdef";

// see: cans/plugins/ledger.md#OpenTelemetry
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[usize::from(*byte >> 4)] as char);
        out.push(HEX[usize::from(*byte & 0x0f)] as char);
    }
    out
}

// see: cans/plugins/ledger.md#OpenTelemetry
fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

// see: cans/plugins/ledger.md#OpenTelemetry
fn decode_hex<const N: usize>(text: &str) -> Result<[u8; N], PluginError> {
    let bytes = text.as_bytes();
    if bytes.len() != N * 2 {
        return Err(PluginError::Halt(format!(
            "traceparent segment must be {} hex chars",
            N * 2
        )));
    }
    let mut out = [0u8; N];
    for (index, slot) in out.iter_mut().enumerate() {
        let high = hex_digit(bytes[index * 2]).ok_or_else(|| {
            PluginError::Halt("traceparent segment carries a non-hex char".into())
        })?;
        let low = hex_digit(bytes[index * 2 + 1]).ok_or_else(|| {
            PluginError::Halt("traceparent segment carries a non-hex char".into())
        })?;
        *slot = (high << 4) | low;
    }
    Ok(out)
}

impl TraceParent {
    // see: cans/plugins/ledger.md#OpenTelemetry
    pub fn to_header(&self) -> String {
        format!(
            "00-{}-{}-{}",
            hex(&self.trace_id),
            hex(&self.parent_span_id),
            hex(&[self.flags])
        )
    }

    // see: cans/plugins/ledger.md#OpenTelemetry
    pub fn parse_header(value: &str) -> Result<Self, PluginError> {
        // TODO (L3): enforce the full W3C trace-context rules (version, all-zero trace/span ids, extension fields).
        let mut parts = value.split('-');
        let version = parts
            .next()
            .ok_or_else(|| PluginError::Halt("traceparent header is empty".into()))?;
        let trace_id = parts
            .next()
            .ok_or_else(|| PluginError::Halt("traceparent header carries no trace id".into()))?;
        let parent_span_id = parts
            .next()
            .ok_or_else(|| PluginError::Halt("traceparent header carries no span id".into()))?;
        let flags = parts
            .next()
            .ok_or_else(|| PluginError::Halt("traceparent header carries no flags".into()))?;
        if version.is_empty() || parts.next().is_some() {
            return Err(PluginError::Halt(
                "traceparent header must carry exactly four fields".into(),
            ));
        }
        let flag_bytes = decode_hex::<1>(flags)?;
        Ok(Self {
            trace_id: decode_hex(trace_id)?,
            parent_span_id: decode_hex(parent_span_id)?,
            flags: flag_bytes[0],
        })
    }
}
