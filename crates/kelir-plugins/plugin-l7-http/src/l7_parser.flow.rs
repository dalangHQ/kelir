// see: cans/plugins/l7-http.md#Plugin-l7-http
// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
use serde::{Deserialize, Serialize};

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L7ParseError {
    Incomplete,
    MalformedHeader(String),
    MalformedRequestLine(String),
    MalformedStatusLine(String),
    BodyTooLarge { len: usize },
    TlsUnavailable(String),
}

impl std::fmt::Display for L7ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Incomplete => f.write_str("l7 stream ends mid-message"),
            Self::MalformedHeader(line) => write!(f, "malformed l7 header line `{line}`"),
            Self::MalformedRequestLine(line) => write!(f, "malformed l7 request line `{line}`"),
            Self::MalformedStatusLine(line) => write!(f, "malformed l7 status line `{line}`"),
            Self::BodyTooLarge { len } => write!(f, "l7 body of {len} bytes exceeds the 16MB cap"),
            Self::TlsUnavailable(detail) => write!(f, "tls decrypt unavailable: {detail}"),
        }
    }
}

impl std::error::Error for L7ParseError {}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
pub const MAX_L7_BODY_BYTES: usize = 16_777_216;

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub target: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub reason: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
pub fn split_headers(bytes: &[u8]) -> Result<(Vec<(String, String)>, usize), L7ParseError> {
    let mut end = None;
    let mut cursor = 0;
    while cursor + 1 < bytes.len() {
        if bytes[cursor] == b'\r' && bytes[cursor + 1] == b'\n' {
            if cursor + 3 < bytes.len() && bytes[cursor + 2] == b'\r' && bytes[cursor + 3] == b'\n'
            {
                end = Some(cursor + 4);
                break;
            }
            cursor += 2;
        } else if bytes[cursor] == b'\n' && cursor + 1 < bytes.len() && bytes[cursor + 1] == b'\n' {
            end = Some(cursor + 2);
            break;
        } else {
            cursor += 1;
        }
    }
    let Some(body_offset) = end else {
        return Err(L7ParseError::Incomplete);
    };
    let head = std::str::from_utf8(&bytes[..body_offset])
        .map_err(|_| L7ParseError::MalformedHeader("header block is not valid utf-8".into()))?;
    let mut headers = Vec::new();
    for line in head.lines().skip(1) {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(L7ParseError::MalformedHeader(line.to_string()));
        };
        headers.push((name.trim().to_string(), value.trim().to_string()));
    }
    Ok((headers, body_offset))
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
pub fn content_length(headers: &[(String, String)]) -> Result<usize, L7ParseError> {
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("content-length") {
            return value
                .parse::<usize>()
                .map_err(|_| L7ParseError::MalformedHeader(format!("content-length: {value}")));
        }
    }
    Ok(0)
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
pub fn parse_http_request(bytes: &[u8]) -> Result<(HttpRequest, usize), L7ParseError> {
    let (headers, body_offset) = split_headers(bytes)?;
    let head = std::str::from_utf8(&bytes[..body_offset])
        .map_err(|_| L7ParseError::MalformedRequestLine("request head is not utf-8".into()))?;
    let request_line = head.lines().next().unwrap_or_default();
    let mut parts = request_line.split_ascii_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();
    let version = parts.next().unwrap_or_default().to_string();
    if method.is_empty() || target.is_empty() || !version.starts_with("HTTP/") {
        return Err(L7ParseError::MalformedRequestLine(request_line.to_string()));
    }
    let len = content_length(&headers)?;
    if len > MAX_L7_BODY_BYTES {
        return Err(L7ParseError::BodyTooLarge { len });
    }
    if bytes.len() < body_offset + len {
        return Err(L7ParseError::Incomplete);
    }
    Ok((
        HttpRequest {
            method,
            target,
            version,
            headers,
            body: bytes[body_offset..body_offset + len].to_vec(),
        },
        body_offset + len,
    ))
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
pub fn parse_http_response(bytes: &[u8]) -> Result<(HttpResponse, usize), L7ParseError> {
    let (headers, body_offset) = split_headers(bytes)?;
    let head = std::str::from_utf8(&bytes[..body_offset])
        .map_err(|_| L7ParseError::MalformedStatusLine("response head is not utf-8".into()))?;
    let status_line = head.lines().next().unwrap_or_default();
    let mut parts = status_line.split_ascii_whitespace();
    let version = parts.next().unwrap_or_default();
    let status = parts
        .next()
        .unwrap_or_default()
        .parse::<u16>()
        .map_err(|_| L7ParseError::MalformedStatusLine(status_line.to_string()))?;
    if !version.starts_with("HTTP/") {
        return Err(L7ParseError::MalformedStatusLine(status_line.to_string()));
    }
    let reason = parts.collect::<Vec<&str>>().join(" ");
    let len = content_length(&headers)?;
    if len > MAX_L7_BODY_BYTES {
        return Err(L7ParseError::BodyTooLarge { len });
    }
    if bytes.len() < body_offset + len {
        return Err(L7ParseError::Incomplete);
    }
    Ok((
        HttpResponse {
            status,
            reason,
            headers,
            body: bytes[body_offset..body_offset + len].to_vec(),
        },
        body_offset + len,
    ))
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
    pub id: Option<String>,
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
pub fn split_sse_frames(body: &[u8]) -> Vec<SseEvent> {
    let text = String::from_utf8_lossy(body);
    let mut events = Vec::new();
    let mut current = SseEvent::default();
    let mut open = false;
    for line in text.lines() {
        if line.is_empty() {
            if open {
                events.push(std::mem::take(&mut current));
                open = false;
            }
            continue;
        }
        open = true;
        if let Some(data) = line.strip_prefix("data:") {
            current.data.push_str(data.trim_start());
            current.data.push('\n');
        } else if let Some(event) = line.strip_prefix("event:") {
            current.event = Some(event.trim().to_string());
        } else if let Some(id) = line.strip_prefix("id:") {
            current.id = Some(id.trim().to_string());
        }
    }
    if open {
        events.push(current);
    }
    for event in &mut events {
        if event.data.ends_with('\n') {
            event.data.pop();
        }
    }
    events
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcEnvelope {
    pub id: Option<serde_json::Value>,
    pub method: Option<String>,
    pub params: Option<serde_json::Value>,
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
pub fn parse_json_rpc(body: &[u8]) -> Result<JsonRpcEnvelope, L7ParseError> {
    if body.len() > MAX_L7_BODY_BYTES {
        return Err(L7ParseError::BodyTooLarge { len: body.len() });
    }
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| L7ParseError::MalformedHeader("json-rpc body is not valid json".into()))?;
    let Some(object) = value.as_object() else {
        return Err(L7ParseError::MalformedHeader(
            "json-rpc body is not an object".into(),
        ));
    };
    if object.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
        return Err(L7ParseError::MalformedHeader(
            "json-rpc body carries no jsonrpc 2.0 marker".into(),
        ));
    }
    Ok(JsonRpcEnvelope {
        id: object.get("id").cloned(),
        method: object
            .get("method")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        params: object.get("params").cloned(),
    })
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
pub use crate::l7_tls::{HostKeyringDecryptor, TlsDecryptor};

// see: cans/plugins/l7-http.md#Error-and-return-semantics-(l7-http)
pub fn synthetic_403_receipt(reason: &str, pid: u32, binary: &str) -> HttpResponse {
    synthetic_403_receipt_with_guidance(reason, pid, binary, None)
}

// see: blindess.patch.md#B: deny → redirect + guidance steering so the agent
// does not burn tokens in blind retry loops against an opaque 403.
pub fn synthetic_403_receipt_with_guidance(
    reason: &str,
    pid: u32,
    binary: &str,
    guidance: Option<&str>,
) -> HttpResponse {
    let body = match guidance {
        Some(hint) => serde_json::json!({
            "error": "E_POLICY_DENIED",
            "reason": reason,
            "pid": pid,
            "binary": binary,
            "guidance": hint,
        }),
        None => serde_json::json!({
            "error": "E_POLICY_DENIED",
            "reason": reason,
            "pid": pid,
            "binary": binary,
        }),
    };
    HttpResponse {
        status: 403,
        reason: "Forbidden".to_string(),
        headers: vec![("content-type".to_string(), "application/json".to_string())],
        body: serde_json::to_vec(&body).unwrap_or_else(|_| Vec::new()),
    }
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L7Message {
    Request,
    Response,
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
pub struct L7Parser {
    pub l7_protocol_inspection: bool,
    buffered: Vec<u8>,
}

impl L7Parser {
    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    pub fn new(l7_protocol_inspection: bool) -> Self {
        Self {
            l7_protocol_inspection,
            buffered: Vec::new(),
        }
    }

    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    pub fn feed(&mut self, bytes: &[u8]) {
        self.buffered.extend_from_slice(bytes);
    }

    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    pub fn buffered_len(&self) -> usize {
        self.buffered.len()
    }

    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    pub fn next_request(&mut self) -> Result<Option<HttpRequest>, L7ParseError> {
        if self.buffered.is_empty() {
            return Ok(None);
        }
        match parse_http_request(&self.buffered) {
            Ok((request, consumed)) => {
                self.buffered.drain(..consumed);
                Ok(Some(request))
            }
            Err(L7ParseError::Incomplete) => Ok(None),
            Err(error) => Err(error),
        }
    }

    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    pub fn next_response(&mut self) -> Result<Option<HttpResponse>, L7ParseError> {
        if self.buffered.is_empty() {
            return Ok(None);
        }
        match parse_http_response(&self.buffered) {
            Ok((response, consumed)) => {
                self.buffered.drain(..consumed);
                Ok(Some(response))
            }
            Err(L7ParseError::Incomplete) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

impl Default for L7Parser {
    fn default() -> Self {
        Self::new(true)
    }
}

// see: blindess.patch.md#B: declared-services `..` path-traversal rejection.
/// Pure check: true when any `/`-separated segment of `target` equals `..`,
/// which the proxy must reject with 403 before dialing the upstream.
pub fn path_traversal_rejected(target: &str) -> bool {
    let path = target.split('?').next().unwrap_or("");
    path.split('/').any(|segment| segment == "..")
}

// see: blindess.patch.md#B: normalized token usage across LLM dialects.
/// Pure OpenAI/Anthropic token-usage normalizer: maps provider-specific
/// usage JSON onto the shared `{input_tokens, output_tokens}` shape.
pub fn normalize_token_usage(dialect: &str, usage: &serde_json::Value) -> (u64, u64) {
    let object = usage.as_object();
    let pick = |names: &[&str]| -> u64 {
        names
            .iter()
            .find_map(|name| object.and_then(|o| o.get(*name)).and_then(|v| v.as_u64()))
            .unwrap_or(0)
    };
    match dialect {
        // Anthropic: usage {input_tokens, output_tokens}
        "anthropic" => (
            pick(&["input_tokens"]),
            pick(&["output_tokens"]),
        ),
        // OpenAI: usage {prompt_tokens, completion_tokens} (+ total_tokens)
        "openai" => (
            pick(&["prompt_tokens", "input_tokens"]),
            pick(&["completion_tokens", "output_tokens"]),
        ),
        // Unknown dialect: fall back to the shared field names.
        _ => (pick(&["input_tokens"]), pick(&["output_tokens"])),
    }
}

// see: blindess.patch.md#B: real-time SSE tool_use chunk inspection.
/// Pure scan of one SSE chunk string for an Anthropic/OpenAI-shaped
/// `"type":"tool_use"` JSON block (whitespace between tokens tolerated),
/// so streaming tool use is caught before guest read.
pub fn sse_chunk_contains_tool_use(chunk: &str) -> bool {
    if !chunk.contains("tool_use") {
        return false;
    }
    // Whitespace-tolerant scan for the `"type": "tool_use"` pair.
    let mut cursor = 0usize;
    let bytes = chunk.as_bytes();
    while let Some(found) = find_subslice(bytes, b"\"type\"", cursor) {
        let mut scan = found + b"\"type\"".len();
        while scan < bytes.len() && (bytes[scan] as char).is_ascii_whitespace() {
            scan += 1;
        }
        if bytes.get(scan) == Some(&b':') {
            scan += 1;
            while scan < bytes.len() && (bytes[scan] as char).is_ascii_whitespace() {
                scan += 1;
            }
            if chunk[scan..].starts_with("\"tool_use\"") {
                return true;
            }
        }
        cursor = found + 1;
    }
    false
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
fn find_subslice(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let start = from.min(haystack.len().saturating_sub(1));
    haystack[start..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| start + offset)
}
