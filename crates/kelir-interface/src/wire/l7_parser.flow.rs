// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#L7-protocol-inspection
use serde::{Deserialize, Serialize};

// see: cans/07-wire-plane.md#L7-protocol-inspection
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

// see: cans/07-wire-plane.md#L7-protocol-inspection
pub const MAX_L7_BODY_BYTES: usize = 16_777_216;

// see: cans/07-wire-plane.md#L7-protocol-inspection
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub target: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    // see: cans/07-wire-plane.md#L7-protocol-inspection
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

// see: cans/07-wire-plane.md#L7-protocol-inspection
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub reason: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    // see: cans/07-wire-plane.md#L7-protocol-inspection
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

// see: cans/07-wire-plane.md#L7-protocol-inspection
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

// see: cans/07-wire-plane.md#L7-protocol-inspection
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

// see: cans/07-wire-plane.md#L7-protocol-inspection
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

// see: cans/07-wire-plane.md#L7-protocol-inspection
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

// see: cans/07-wire-plane.md#L7-protocol-inspection
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
    pub id: Option<String>,
}

// see: cans/07-wire-plane.md#L7-protocol-inspection
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

// see: cans/07-wire-plane.md#L7-protocol-inspection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcEnvelope {
    pub id: Option<serde_json::Value>,
    pub method: Option<String>,
    pub params: Option<serde_json::Value>,
}

// see: cans/07-wire-plane.md#L7-protocol-inspection
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

// see: cans/07-wire-plane.md#L7-protocol-inspection
pub trait TlsDecryptor: Send + Sync {
    fn decrypt(&self, record: &[u8]) -> Result<Vec<u8>, L7ParseError>;
}

// see: cans/07-wire-plane.md#L7-protocol-inspection
#[derive(Debug, Clone, Default)]
pub struct HostKeyringDecryptor {
    pub key_id: Option<String>,
}

impl TlsDecryptor for HostKeyringDecryptor {
    fn decrypt(&self, _record: &[u8]) -> Result<Vec<u8>, L7ParseError> {
        // TODO (L3) [wire_plane]: fetch the session key from the host keyring and decrypt the TLS record; no keyring read exists yet.; slop: TLS records pass through undecrypted, L7 rules blind
        Err(L7ParseError::TlsUnavailable(
            "host keyring tls decrypt is TODO (L3)".into(),
        ))
    }
}

// see: cans/07-wire-plane.md#L7-protocol-inspection
pub fn synthetic_403_receipt() -> HttpResponse {
    HttpResponse {
        status: 403,
        reason: "Forbidden".to_string(),
        headers: vec![("content-type".to_string(), "application/json".to_string())],
        body: b"{\"error\":\"forbidden\",\"errno\":\"EACCES\"}".to_vec(),
    }
}

// see: cans/07-wire-plane.md#L7-protocol-inspection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L7Message {
    Request,
    Response,
}

// see: cans/07-wire-plane.md#L7-protocol-inspection
pub struct L7Parser {
    pub l7_protocol_inspection: bool,
    buffered: Vec<u8>,
}

impl L7Parser {
    // see: cans/07-wire-plane.md#L7-protocol-inspection
    pub fn new(l7_protocol_inspection: bool) -> Self {
        Self {
            l7_protocol_inspection,
            buffered: Vec::new(),
        }
    }

    // see: cans/07-wire-plane.md#L7-protocol-inspection
    pub fn feed(&mut self, bytes: &[u8]) {
        self.buffered.extend_from_slice(bytes);
    }

    // see: cans/07-wire-plane.md#L7-protocol-inspection
    pub fn buffered_len(&self) -> usize {
        self.buffered.len()
    }

    // see: cans/07-wire-plane.md#L7-protocol-inspection
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

    // see: cans/07-wire-plane.md#L7-protocol-inspection
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
