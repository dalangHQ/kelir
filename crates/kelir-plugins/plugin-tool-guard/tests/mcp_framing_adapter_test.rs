// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_09_mcp_console_framing_continuation.yaml#case_id
use plugin_tool_guard::service_tool_guard::{McpFrame, McpMethod};

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_09_mcp_console_framing_continuation.yaml#case_id
const FRAME_PREFIX: &str = concat!(
    "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",",
    "\"params\":{\"name\":\"fs.write\",\"arguments\":{\"padding\":\""
);
const FRAME_SUFFIX: &str = "\"}}}\n";
// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_09_mcp_console_framing_continuation.yaml#case_id
const FRAME_LEN: usize = 98_304;
const PADDING_LEN: usize = FRAME_LEN - FRAME_PREFIX.len() - FRAME_SUFFIX.len();

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_09_mcp_console_framing_continuation.yaml#case_id
fn oversized_frame() -> Vec<u8> {
    let mut frame = String::with_capacity(FRAME_LEN);
    frame.push_str(FRAME_PREFIX);
    frame.extend(std::iter::repeat_n('P', PADDING_LEN));
    frame.push_str(FRAME_SUFFIX);
    frame.into_bytes()
}

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_09_mcp_console_framing_continuation.yaml#case_id
#[test]
#[ignore]
fn ad_09_mcp_console_framing_continuation() -> Result<(), String> {
    let frame = oversized_frame();
    if frame.len() != FRAME_LEN {
        return Err(format!(
            "fixture frame is {} bytes, expected {FRAME_LEN}",
            frame.len()
        ));
    }
    let decoded = McpFrame::decode(&frame).map_err(|e| format!("decode halted: {e}"))?;
    if decoded.method != McpMethod::ToolsCall {
        return Err(format!(
            "a {FRAME_LEN}-byte tools/call frame decoded as {:?}; the FLAG_CONTINUATION halves were never reassembled",
            decoded.method
        ));
    }
    if decoded.id != 7 || decoded.tool_name.as_deref() != Some("fs.write") {
        return Err(format!(
            "reassembled frame lost its identity: id {} tool {:?}",
            decoded.id, decoded.tool_name
        ));
    }
    if decoded.arguments.len() != PADDING_LEN {
        return Err(format!(
            "reassembled arguments are {} bytes, expected the full {PADDING_LEN}-byte payload",
            decoded.arguments.len()
        ));
    }
    Ok(())
}
