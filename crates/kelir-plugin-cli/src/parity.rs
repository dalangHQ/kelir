// see: cans/15-operations.md#Multi-language-parity-gate
use std::fs;

use kelir_interface::protocol::Channel;
use serde_json::Value;

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
    pub watchdog_verdict: Option<String>,
    pub veto_event: Option<String>,
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

// see: cans/15-operations.md#Coverage-gates
const WATCHDOG_VERDICTS: &[&str] = &["Suppress", "Mitigate"];

// see: cans/15-operations.md#Coverage-gates
const VETO_EVENTS: &[&str] = &["Fork", "Suspend", "ClockSkew"];

fn verdict_kind(value: &Value, index: usize) -> Result<String, String> {
    match value {
        Value::String(kind) if VERDICT_VARIANTS.contains(&kind.as_str()) => Ok(kind.clone()),
        Value::Object(entries) => {
            if entries.len() != 1 {
                return Err(format!(
                    "parity: vector {index} expected_verdict must carry exactly one Verdict variant"
                ));
            }
            let Some((kind, _)) = entries.iter().next() else {
                return Err(format!(
                    "parity: vector {index} expected_verdict must carry exactly one Verdict variant"
                ));
            };
            if VERDICT_VARIANTS.contains(&kind.as_str()) {
                Ok(kind.clone())
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
    let root: Value = serde_json::from_str(text)
        .map_err(|e| format!("parity: vectors.json is not valid JSON: {e}"))?;
    let vectors = root
        .get("vectors")
        .and_then(Value::as_array)
        .ok_or("parity: vectors.json must be an object with a `vectors` array".to_string())?;
    let mut out = Vec::new();
    for (index, item) in vectors.iter().enumerate() {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .ok_or(format!("parity: vector {index} has no string `name`"))?
            .to_string();
        let frame = item
            .get("frame")
            .ok_or(format!("parity: vector {index} declares no `frame`"))?;
        let transport = frame
            .get("transport")
            .and_then(Value::as_u64)
            .and_then(|v| u16::try_from(v).ok())
            .ok_or(format!(
                "parity: vector {index} frame has no u16 `transport`"
            ))?;
        let stream_id = frame
            .get("stream_id")
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .ok_or(format!(
                "parity: vector {index} frame has no u32 `stream_id`"
            ))?;
        let flags = frame
            .get("flags")
            .and_then(Value::as_u64)
            .and_then(|v| u8::try_from(v).ok())
            .ok_or(format!("parity: vector {index} frame has no u8 `flags`"))?;
        let msg_type = frame
            .get("msg_type")
            .and_then(Value::as_u64)
            .and_then(|v| u16::try_from(v).ok())
            .ok_or(format!(
                "parity: vector {index} frame has no u16 `msg_type`"
            ))?;
        let payload_hex = frame
            .get("payload_cbor_hex")
            .and_then(Value::as_str)
            .ok_or(format!(
                "parity: vector {index} frame has no `payload_cbor_hex`"
            ))?
            .to_string();
        let frame_hex = frame
            .get("frame_hex")
            .and_then(Value::as_str)
            .ok_or(format!("parity: vector {index} frame has no `frame_hex`"))?
            .to_string();
        let expected_verdict = item.get("expected_verdict").ok_or(format!(
            "parity: vector {index} declares no `expected_verdict`"
        ))?;
        let watchdog_verdict = match item.get("watchdog") {
            None => None,
            Some(watchdog) => {
                let verdict = watchdog
                    .get("expected_verdict")
                    .and_then(Value::as_str)
                    .ok_or(format!(
                        "parity: vector {index} watchdog declares no string `expected_verdict`"
                    ))?;
                if !WATCHDOG_VERDICTS.contains(&verdict) {
                    return Err(format!(
                        "parity: vector {index} watchdog verdict `{verdict}` is not Suppress or Mitigate"
                    ));
                }
                Some(verdict.to_string())
            }
        };
        let veto_event = match item.get("veto") {
            None => None,
            Some(veto) => {
                let event = veto.get("event").and_then(Value::as_str).ok_or(format!(
                    "parity: vector {index} veto declares no string `event`"
                ))?;
                if !VETO_EVENTS.contains(&event) {
                    return Err(format!(
                        "parity: vector {index} veto event `{event}` is not a VetoEvent"
                    ));
                }
                Some(event.to_string())
            }
        };
        out.push(ParityVector {
            name,
            transport,
            stream_id,
            flags,
            msg_type,
            payload_hex,
            frame_hex,
            expected_verdict_kind: verdict_kind(expected_verdict, index)?,
            watchdog_verdict,
            veto_event,
        });
    }
    Ok(out)
}

// see: cans/artifacts/code/wire-protocol.rs#MuxFrameHeader
fn assert_frame_shape(vector: &ParityVector) -> Result<(), String> {
    if Channel::from_u16(vector.transport).is_none() {
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
    for verdict in WATCHDOG_VERDICTS {
        if !vectors
            .iter()
            .any(|vector| vector.watchdog_verdict.as_deref() == Some(verdict))
        {
            return Err(format!(
                "parity: watchdog coverage missing WatchdogVerdict::{verdict} in vectors.json chains"
            ));
        }
    }
    for event in VETO_EVENTS {
        if !vectors
            .iter()
            .any(|vector| vector.veto_event.as_deref() == Some(event))
        {
            return Err(format!(
                "parity: veto coverage missing VetoEvent::{event} in vectors.json chains"
            ));
        }
    }
    Ok(vectors.len())
}

// see: cans/15-operations.md#Multi-language-parity-gate
fn assert_canonical_roster(vectors: &[ParityVector]) -> Result<(), String> {
    let canonical = kelir_plugin_testkit::fixtures::canonical_vector_names()
        .map_err(|e| format!("parity: cannot derive the canonical roster from the testkit: {e}"))?;
    let declared: Vec<&str> = vectors.iter().map(|vector| vector.name.as_str()).collect();
    let expected: Vec<&str> = canonical.iter().map(String::as_str).collect();
    if declared != expected {
        return Err(format!(
            "parity: vectors.json roster drifted from the kelir-plugin-testkit fixtures (regenerate with `kelir-plugin-cli vectors`): declared {declared:?}, expected {expected:?}"
        ));
    }
    Ok(())
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
    assert_canonical_roster(&vectors)?;
    println!("parity: {count} vector(s) asserted from {path}");
    Ok(())
}
