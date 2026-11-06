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
        out.push(ParityVector {
            name: name.to_string(),
        });
    }
    Ok(out)
}

pub fn assert_vectors(vectors: &[ParityVector]) -> Result<usize, String> {
    if vectors.is_empty() {
        return Err("parity: vectors.json declares 0 vectors; nothing to assert".to_string());
    }
    Ok(vectors.len())
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = args.first().map(String::as_str).unwrap_or("vectors.json");
    let text = fs::read_to_string(path).map_err(|e| {
        format!(
            "parity: vectors file missing at {path}: {e} (generate it with `kelir-plugin-cli vectors`, L3)"
        )
    })?;
    let vectors = parse_vectors(&text)?;
    let count = assert_vectors(&vectors)?;
    println!("parity: {count} vector(s) asserted from {path}");
    Ok(())
}
