// see: cans/plugins/ledger.md#OpenTelemetry
use kelir_interface::hex_codec::{HexDecodeError, encode_hex};
use kelir_interface::plugin_model::PluginError;

// see: cans/plugins/ledger.md#OpenTelemetry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceParent {
    pub trace_id: [u8; 16],
    pub parent_span_id: [u8; 8],
    pub flags: u8,
}

// see: cans/plugins/ledger.md#OpenTelemetry
fn trace_segment<const N: usize>(text: &str) -> Result<[u8; N], PluginError> {
    kelir_interface::hex_codec::decode_hex_fixed::<N>(text).map_err(|error| match error {
        HexDecodeError::InvalidDigit(_) => {
            PluginError::Halt("traceparent segment carries a non-hex char".into())
        }
        _ => PluginError::Halt(format!("traceparent segment must be {} hex chars", N * 2)),
    })
}

impl TraceParent {
    // see: cans/plugins/ledger.md#OpenTelemetry
    pub fn to_header(&self) -> String {
        format!(
            "00-{}-{}-{}",
            encode_hex(&self.trace_id),
            encode_hex(&self.parent_span_id),
            encode_hex(&[self.flags])
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
        let flag_bytes = trace_segment::<1>(flags)?;
        Ok(Self {
            trace_id: trace_segment(trace_id)?,
            parent_span_id: trace_segment(parent_span_id)?,
            flags: flag_bytes[0],
        })
    }
}
