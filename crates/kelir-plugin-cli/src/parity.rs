// see: cans/15-operations.md#Multi-language-parity-gate
use std::fs;

// see: cans/15-operations.md#Multi-language-parity-gate
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(entries) => entries
                .iter()
                .find(|(entry_key, _)| entry_key == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            JsonValue::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            JsonValue::Array(items) => Some(items),
            _ => None,
        }
    }
}

struct JsonParser<'a> {
    text: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text: text.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.text.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn parse(&mut self) -> Result<JsonValue, String> {
        let value = self.parse_value()?;
        self.skip_ws();
        if self.pos != self.text.len() {
            return Err(format!("parity: trailing characters at byte {}", self.pos));
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(JsonValue::Text(self.parse_string()?)),
            Some(b't') => self.parse_literal("true", JsonValue::Bool(true)),
            Some(b'f') => self.parse_literal("false", JsonValue::Bool(false)),
            Some(b'n') => self.parse_literal("null", JsonValue::Null),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            Some(other) => Err(format!("parity: unexpected byte `{other}` at {}", self.pos)),
            None => Err("parity: unexpected end of input".to_string()),
        }
    }

    fn parse_literal(&mut self, literal: &str, value: JsonValue) -> Result<JsonValue, String> {
        if self.text[self.pos..].starts_with(literal.as_bytes()) {
            self.pos += literal.len();
            Ok(value)
        } else {
            Err(format!("parity: bad literal at byte {}", self.pos))
        }
    }

    fn parse_number(&mut self) -> Result<JsonValue, String> {
        let start = self.pos;
        while matches!(
            self.peek(),
            Some(b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')
        ) {
            self.pos += 1;
        }
        let raw = std::str::from_utf8(&self.text[start..self.pos])
            .map_err(|_| "parity: bad number".to_string())?;
        raw.parse::<f64>()
            .map(JsonValue::Number)
            .map_err(|_| format!("parity: bad number `{raw}`"))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        if self.peek() != Some(b'"') {
            return Err(format!("parity: expected string at byte {}", self.pos));
        }
        self.pos += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            match self.peek() {
                None => return Err("parity: unterminated string".to_string()),
                Some(b'"') => {
                    self.pos += 1;
                    return String::from_utf8(out)
                        .map_err(|_| "parity: string is not valid UTF-8".to_string());
                }
                Some(b'\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some(b'"') => out.push(b'"'),
                        Some(b'\\') => out.push(b'\\'),
                        Some(b'/') => out.push(b'/'),
                        Some(b'n') => out.push(b'\n'),
                        Some(b't') => out.push(b'\t'),
                        Some(b'r') => out.push(b'\r'),
                        Some(b'b') => out.push(8),
                        Some(b'f') => out.push(12),
                        Some(b'u') => {
                            let hex = self
                                .text
                                .get(self.pos + 1..self.pos + 5)
                                .ok_or("parity: bad unicode escape".to_string())?;
                            let hex = std::str::from_utf8(hex)
                                .map_err(|_| "parity: bad unicode escape".to_string())?;
                            let code = u32::from_str_radix(hex, 16)
                                .map_err(|_| "parity: bad unicode escape".to_string())?;
                            let ch = char::from_u32(code)
                                .ok_or("parity: bad unicode escape".to_string())?;
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                            self.pos += 4;
                        }
                        _ => {
                            return Err(format!("parity: bad escape at byte {}", self.pos));
                        }
                    }
                    self.pos += 1;
                }
                Some(byte) => {
                    out.push(byte);
                    self.pos += 1;
                }
            }
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(JsonValue::Array(items));
                }
                _ => {
                    return Err(format!("parity: expected `,` or `]` at byte {}", self.pos));
                }
            }
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.pos += 1;
        let mut entries = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonValue::Object(entries));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(format!("parity: expected `:` at byte {}", self.pos));
            }
            self.pos += 1;
            let value = self.parse_value()?;
            entries.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(JsonValue::Object(entries));
                }
                _ => {
                    return Err(format!("parity: expected `,` or `}}` at byte {}", self.pos));
                }
            }
        }
    }
}

// see: cans/15-operations.md#Multi-language-parity-gate
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityVector {
    pub name: String,
    pub transport: u16,
    pub stream_id: u32,
    pub flags: u8,
    pub msg_type: u16,
    pub payload_hex: String,
    pub frame_hex: String,
    pub expected_verdict_kind: String,
}

impl JsonValue {
    fn as_u64(&self) -> Option<u64> {
        match self {
            JsonValue::Number(n) if n.fract() == 0.0 && *n >= 0.0 && *n <= 1.8446744e19 => {
                Some(*n as u64)
            }
            _ => None,
        }
    }
}

fn decode_hex(text: &str) -> Result<Vec<u8>, String> {
    let bytes = text.as_bytes();
    if bytes.len() % 2 != 0 {
        return Err(format!("parity: hex string has odd length: {text}"));
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks(2) {
        let high = (pair[0] as char).to_digit(16);
        let low = (pair[1] as char).to_digit(16);
        match (high, low) {
            (Some(h), Some(l)) => out.push(((h << 4) | l) as u8),
            _ => return Err(format!("parity: invalid hex string: {text}")),
        }
    }
    Ok(out)
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

// see: cans/02-plugin-model.md#Unified-verdict
const VERDICT_VARIANTS: &[&str] = &["Pass", "Patch", "Drop", "Suspend", "Abort"];

fn verdict_kind(value: &JsonValue, index: usize) -> Result<String, String> {
    match value {
        JsonValue::Text(kind) if VERDICT_VARIANTS.contains(&kind.as_str()) => Ok(kind.clone()),
        JsonValue::Object(entries) => {
            if entries.len() != 1 {
                return Err(format!(
                    "parity: vector {index} expected_verdict must carry exactly one Verdict variant"
                ));
            }
            let kind = entries[0].0.clone();
            if VERDICT_VARIANTS.contains(&kind.as_str()) {
                Ok(kind)
            } else {
                Err(format!(
                    "parity: vector {index} expected_verdict `{kind}` is outside the Verdict lattice"
                ))
            }
        }
        _ => Err(format!(
            "parity: vector {index} expected_verdict is not a Verdict"
        )),
    }
}

pub fn parse_vectors(text: &str) -> Result<Vec<ParityVector>, String> {
    let mut parser = JsonParser::new(text);
    let root = parser.parse()?;
    let vectors = root
        .get("vectors")
        .and_then(JsonValue::as_array)
        .ok_or("parity: vectors.json must be an object with a `vectors` array".to_string())?;
    let mut out = Vec::new();
    for (index, item) in vectors.iter().enumerate() {
        let name = item
            .get("name")
            .and_then(JsonValue::as_text)
            .ok_or(format!("parity: vector {index} has no string `name`"))?;
        let frame = item
            .get("frame")
            .ok_or(format!("parity: vector {index} declares no `frame`"))?;
        let transport = frame
            .get("transport")
            .and_then(JsonValue::as_u64)
            .and_then(|v| u16::try_from(v).ok())
            .ok_or(format!(
                "parity: vector {index} frame has no u16 `transport`"
            ))?;
        let stream_id = frame
            .get("stream_id")
            .and_then(JsonValue::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .ok_or(format!(
                "parity: vector {index} frame has no u32 `stream_id`"
            ))?;
        let flags = frame
            .get("flags")
            .and_then(JsonValue::as_u64)
            .and_then(|v| u8::try_from(v).ok())
            .ok_or(format!("parity: vector {index} frame has no u8 `flags`"))?;
        let msg_type = frame
            .get("msg_type")
            .and_then(JsonValue::as_u64)
            .and_then(|v| u16::try_from(v).ok())
            .ok_or(format!(
                "parity: vector {index} frame has no u16 `msg_type`"
            ))?;
        let payload_hex = frame
            .get("payload_cbor_hex")
            .and_then(JsonValue::as_text)
            .ok_or(format!(
                "parity: vector {index} frame has no `payload_cbor_hex`"
            ))?
            .to_string();
        let frame_hex = frame
            .get("frame_hex")
            .and_then(JsonValue::as_text)
            .ok_or(format!("parity: vector {index} frame has no `frame_hex`"))?
            .to_string();
        let expected_verdict = item.get("expected_verdict").ok_or(format!(
            "parity: vector {index} declares no `expected_verdict`"
        ))?;
        out.push(ParityVector {
            name: name.to_string(),
            transport,
            stream_id,
            flags,
            msg_type,
            payload_hex,
            frame_hex,
            expected_verdict_kind: verdict_kind(expected_verdict, index)?,
        });
    }
    Ok(out)
}

// see: cans/artifacts/code/wire-protocol.rs
fn assert_frame_shape(vector: &ParityVector) -> Result<(), String> {
    if vector.transport != kelir_interface::codec::TRANSPORT_CONTROL
        && vector.transport != kelir_interface::codec::TRANSPORT_STREAM_MUX
    {
        return Err(format!(
            "parity: vector `{}` transport {} is not a registered transport",
            vector.name, vector.transport
        ));
    }
    if kelir_interface::protocol::MsgType::from_u16(vector.msg_type).is_none() {
        return Err(format!(
            "parity: vector `{}` msg_type {:#06x} is not a core message type",
            vector.name, vector.msg_type
        ));
    }
    if vector.stream_id > kelir_interface::codec::MAX_STREAM_ID {
        return Err(format!(
            "parity: vector `{}` stream_id {} exceeds MAX_STREAM_ID",
            vector.name, vector.stream_id
        ));
    }
    let frame = decode_hex(&vector.frame_hex)?;
    if frame.len() < 11 {
        return Err(format!(
            "parity: vector `{}` frame is {} bytes, shorter than a 9-byte header plus msg_type",
            vector.name,
            frame.len()
        ));
    }
    let stream_id = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]);
    if stream_id != vector.stream_id {
        return Err(format!(
            "parity: vector `{}` frame header stream_id {stream_id} differs from declared {}",
            vector.name, vector.stream_id
        ));
    }
    if frame[4] != vector.flags {
        return Err(format!(
            "parity: vector `{}` frame header flags {} differs from declared {}",
            vector.name, frame[4], vector.flags
        ));
    }
    let len = u32::from_le_bytes([frame[5], frame[6], frame[7], frame[8]]);
    let body = frame.len() - 9;
    if len as usize != body {
        return Err(format!(
            "parity: vector `{}` frame header len {len} differs from {} body bytes",
            vector.name, body
        ));
    }
    if body > kelir_interface::codec::MAX_FRAME_PAYLOAD {
        return Err(format!(
            "parity: vector `{}` body {body} exceeds MAX_FRAME_PAYLOAD",
            vector.name
        ));
    }
    let msg_type = u16::from_le_bytes([frame[9], frame[10]]);
    if msg_type != vector.msg_type {
        return Err(format!(
            "parity: vector `{}` frame msg_type {msg_type:#06x} differs from declared {:#06x}",
            vector.name, vector.msg_type
        ));
    }
    let payload = &frame[11..];
    if encode_hex(payload) != vector.payload_hex {
        return Err(format!(
            "parity: vector `{}` payload_cbor_hex does not match the frame body",
            vector.name
        ));
    }
    Ok(())
}

pub fn assert_vectors(vectors: &[ParityVector]) -> Result<usize, String> {
    if vectors.is_empty() {
        return Err("parity: vectors.json declares 0 vectors; nothing to assert".to_string());
    }
    for vector in vectors {
        if vector.name.is_empty() {
            return Err("parity: vector with empty name".to_string());
        }
        assert_frame_shape(vector)?;
    }
    Ok(vectors.len())
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut path = "vectors.json".to_string();
    match args.first().map(String::as_str) {
        None => {}
        Some("run") => {
            if let Some(custom) = args.get(1) {
                path = custom.clone();
            }
        }
        Some(other) => {
            return Err(format!("parity: unknown action `{other}` (expected `run [path]`)").into());
        }
    }
    let text = fs::read_to_string(&path).map_err(|e| {
        format!(
            "parity: vectors file missing at {path}: {e} (generate it with `kelir-plugin-cli vectors`)"
        )
    })?;
    let vectors = parse_vectors(&text)?;
    let count = assert_vectors(&vectors)?;
    println!("parity: {count} vector(s) asserted from {path}");
    Ok(())
}
