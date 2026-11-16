// see: cans/plugins/mcp.md#MCP-contract
use kelir_interface::plugin_model::PluginError;
use serde_json::Value;

// see: cans/plugins/mcp.md#MCP-contract
pub const MAX_MCP_FRAME_BYTES: usize = 16_777_216;

// see: cans/plugins/mcp.md#MCP-contract
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonRpcId {
    Number(u64),
    Text(String),
}

// see: cans/plugins/mcp.md#MCP-contract
#[derive(Debug, Clone)]
pub struct JsonRpcRequest {
    pub id: Option<JsonRpcId>,
    pub method: String,
    pub params: Option<Value>,
}

// see: cans/plugins/mcp.md#MCP-contract
#[derive(Debug, Clone)]
pub struct JsonRpcResponse {
    pub id: Option<JsonRpcId>,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

// see: cans/plugins/mcp.md#MCP-contract
fn parse_id(value: &Value) -> Result<JsonRpcId, PluginError> {
    match value {
        Value::Number(number) => number.as_u64().map(JsonRpcId::Number).ok_or_else(|| {
            PluginError::Halt(
                "ESecurityViolation: JSON-RPC id must be an unsigned integer or string".into(),
            )
        }),
        Value::String(text) => Ok(JsonRpcId::Text(text.clone())),
        _ => Err(PluginError::Halt(
            "ESecurityViolation: JSON-RPC id must be an unsigned integer or string".into(),
        )),
    }
}

// see: cans/plugins/mcp.md#MCP-contract
fn parse_object(payload: &[u8]) -> Result<serde_json::Map<String, Value>, PluginError> {
    if payload.len() > MAX_MCP_FRAME_BYTES {
        return Err(PluginError::Halt(
            "ESecurityViolation: MCP frame exceeds the 16 MB maximum".into(),
        ));
    }
    let value: Value = serde_json::from_slice(payload).map_err(|err| {
        PluginError::Halt(format!(
            "ESecurityViolation: MCP frame is not valid JSON: {err}"
        ))
    })?;
    value.as_object().cloned().ok_or_else(|| {
        PluginError::Halt("ESecurityViolation: MCP frame must be a JSON object".into())
    })
}

// see: cans/plugins/mcp.md#MCP-contract
pub fn parse_json_rpc_request(payload: &[u8]) -> Result<JsonRpcRequest, PluginError> {
    let object = parse_object(payload)?;
    match object.get("jsonrpc") {
        Some(Value::String(version)) if version == "2.0" => (),
        _ => {
            return Err(PluginError::Halt(
                "ESecurityViolation: JSON-RPC request must carry jsonrpc \"2.0\"".into(),
            ));
        }
    }
    let method = match object.get("method") {
        Some(Value::String(method)) if !method.is_empty() => method.clone(),
        _ => {
            return Err(PluginError::Halt(
                "ESecurityViolation: JSON-RPC request method must be a non-empty string".into(),
            ));
        }
    };
    let id = match object.get("id") {
        Some(value) => Some(parse_id(value)?),
        None => None,
    };
    let params = object.get("params").cloned();
    if let Some(params) = &params
        && !params.is_object()
        && !params.is_array()
    {
        return Err(PluginError::Halt(
            "ESecurityViolation: JSON-RPC params must be an object or array".into(),
        ));
    }
    Ok(JsonRpcRequest { id, method, params })
}

// see: cans/plugins/mcp.md#MCP-contract
pub fn parse_json_rpc_response(payload: &[u8]) -> Result<JsonRpcResponse, PluginError> {
    let object = parse_object(payload)?;
    match object.get("jsonrpc") {
        Some(Value::String(version)) if version == "2.0" => (),
        _ => {
            return Err(PluginError::Halt(
                "ESecurityViolation: JSON-RPC response must carry jsonrpc \"2.0\"".into(),
            ));
        }
    }
    let id = match object.get("id") {
        Some(Value::Null) | None => None,
        Some(value) => Some(parse_id(value)?),
    };
    let result = object.get("result").cloned();
    let error = object.get("error").cloned();
    if result.is_none() && error.is_none() {
        return Err(PluginError::Halt(
            "ESecurityViolation: JSON-RPC response carries neither result nor error".into(),
        ));
    }
    Ok(JsonRpcResponse { id, result, error })
}

// see: cans/11-protocol.md#Stream-mux-framing
#[derive(Debug, Default)]
pub struct McpFrameAssembler {
    buffer: Vec<u8>,
}

impl McpFrameAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    // see: cans/plugins/mcp.md#MCP-contract
    pub fn push_continuation(&mut self, chunk: &[u8]) -> Result<(), PluginError> {
        if self.buffer.len().saturating_add(chunk.len()) > MAX_MCP_FRAME_BYTES {
            return Err(PluginError::Halt(
                "ESecurityViolation: MCP continuation frame exceeds the 16 MB maximum".into(),
            ));
        }
        self.buffer.extend_from_slice(chunk);
        Ok(())
    }

    // see: cans/plugins/mcp.md#MCP-contract
    pub fn finish(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.buffer)
    }

    // see: cans/plugins/mcp.md#MCP-contract
    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }
}
