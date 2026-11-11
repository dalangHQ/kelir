// see: cans/15-operations.md#Multi-language-parity-gate
use serde_json::Value;

use kelir_interface::codec_protocol::WireFrame;
use kelir_interface::driver_interface::{WatchdogEvent, WatchdogVerdict};
use kelir_interface::fusion_plugin::TrampolineRing;
use kelir_interface::hex_codec::{decode_hex, encode_hex};
use kelir_interface::plugin_model::VetoEvent;
use kelir_interface::protocol_wire::{Channel, FilterAction, GuestToHost, Verdict};

use crate::vectors_testkit::{
    UnifiedBuilderCase, UnifiedBuilderOutcome, UnifiedBuilderScope, VectorCase, VectorError,
    VectorSuite, VetoCase, WatchdogCase,
};

fn parse_error(message: String) -> VectorError {
    VectorError::Parse(message)
}

fn string_field(value: &Value, key: &str, context: &str) -> Result<String, VectorError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| parse_error(format!("vector `{context}` has no string `{key}`")))
}

fn unsigned_field(value: &Value, key: &str, context: &str, max: u64) -> Result<u64, VectorError> {
    let raw = value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| parse_error(format!("vector `{context}` has no `{key}`")))?;
    if raw > max {
        return Err(parse_error(format!(
            "vector `{context}` `{key}` {raw} exceeds the field maximum {max}"
        )));
    }
    Ok(raw)
}

fn optional_table<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value.get(key) {
        None | Some(Value::Null) => None,
        Some(table) => Some(table),
    }
}

// see: cans/15-operations.md#Multi-language-parity-gate
// see: cans/artifacts/code/wire-protocol.rs#MuxFrameHeader
fn frame_from_json(value: &Value, name: &str) -> Result<WireFrame, VectorError> {
    let frame_json = value
        .get("frame")
        .ok_or_else(|| parse_error(format!("vector `{name}` declares no `frame`")))?;
    let transport = unsigned_field(frame_json, "transport", name, u16::MAX as u64)? as u16;
    let channel = Channel::from_u16(transport).ok_or_else(|| {
        parse_error(format!(
            "vector `{name}` transport {transport} is not a registered transport"
        ))
    })?;
    let frame_hex = string_field(frame_json, "frame_hex", name)?;
    let frame_bytes = decode_hex(&frame_hex)
        .map_err(|e| parse_error(format!("vector `{name}` frame_hex is not valid hex: {e}")))?;
    let frame = WireFrame::decode(channel, &frame_bytes)
        .map_err(|e| parse_error(format!("vector `{name}` frame_hex does not decode: {e}")))?;
    let stream_id = unsigned_field(frame_json, "stream_id", name, u32::MAX as u64)? as u32;
    if frame.stream_id != stream_id {
        return Err(parse_error(format!(
            "vector `{name}` frame header stream_id {} differs from declared {stream_id}",
            frame.stream_id
        )));
    }
    let flags = unsigned_field(frame_json, "flags", name, u8::MAX as u64)? as u8;
    if frame.flags != flags {
        return Err(parse_error(format!(
            "vector `{name}` frame header flags {} differs from declared {flags}",
            frame.flags
        )));
    }
    let msg_type = unsigned_field(frame_json, "msg_type", name, u16::MAX as u64)? as u16;
    if frame.msg_type as u16 != msg_type {
        return Err(parse_error(format!(
            "vector `{name}` frame msg_type {:#06x} differs from declared {msg_type:#06x}",
            frame.msg_type as u16
        )));
    }
    let payload_hex = string_field(frame_json, "payload_cbor_hex", name)?;
    if encode_hex(&frame.payload) != payload_hex {
        return Err(parse_error(format!(
            "vector `{name}` payload_cbor_hex does not match the frame body"
        )));
    }
    Ok(frame)
}

// see: cans/08-lifecycle.md#Watchdog
fn watchdog_from_json(value: &Value, name: &str) -> Result<WatchdogCase, VectorError> {
    let event_json = value
        .get("event")
        .ok_or_else(|| parse_error(format!("vector `{name}` watchdog declares no `event`")))?;
    let event: WatchdogEvent = serde_json::from_value(event_json.clone()).map_err(|e| {
        parse_error(format!(
            "vector `{name}` watchdog event is not a WatchdogEvent: {e}"
        ))
    })?;
    let verdict_json = value.get("expected_verdict").ok_or_else(|| {
        parse_error(format!(
            "vector `{name}` watchdog declares no string `expected_verdict`"
        ))
    })?;
    let expected_verdict: WatchdogVerdict =
        serde_json::from_value(verdict_json.clone()).map_err(|e| {
            parse_error(format!(
                "vector `{name}` watchdog expected_verdict is not a WatchdogVerdict: {e}"
            ))
        })?;
    Ok(WatchdogCase {
        event,
        expected_verdict,
    })
}

// see: cans/08-lifecycle.md#Lifecycle-vetoes
fn veto_from_json(value: &Value, name: &str) -> Result<VetoCase, VectorError> {
    let event_json = value
        .get("event")
        .ok_or_else(|| parse_error(format!("vector `{name}` veto declares no string `event`")))?;
    let event: VetoEvent = serde_json::from_value(event_json.clone()).map_err(|e| {
        parse_error(format!(
            "vector `{name}` veto event is not a VetoEvent: {e}"
        ))
    })?;
    let callback_allows = match value.get("callback_allows") {
        None | Some(Value::Null) => true,
        Some(flag) => flag.as_bool().ok_or_else(|| {
            parse_error(format!(
                "vector `{name}` veto `callback_allows` is not a bool"
            ))
        })?,
    };
    Ok(VetoCase {
        event,
        callback_allows,
    })
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
fn outcome_from_json(value: &Value, name: &str) -> Result<UnifiedBuilderOutcome, VectorError> {
    let Some(entries) = value.as_object() else {
        return Err(parse_error(format!(
            "vector `{name}` plugin_builder outcome must carry exactly one of Verdict or Filter"
        )));
    };
    if entries.len() != 1 {
        return Err(parse_error(format!(
            "vector `{name}` plugin_builder outcome must carry exactly one of Verdict or Filter"
        )));
    }
    let Some((kind, payload)) = entries.iter().next() else {
        return Err(parse_error(format!(
            "vector `{name}` plugin_builder outcome must carry exactly one of Verdict or Filter"
        )));
    };
    match kind.as_str() {
        "Verdict" => {
            let verdict: Verdict = serde_json::from_value(payload.clone()).map_err(|e| {
                parse_error(format!(
                    "vector `{name}` plugin_builder Verdict outcome is not a Verdict: {e}"
                ))
            })?;
            Ok(UnifiedBuilderOutcome::Verdict(verdict))
        }
        "Filter" => {
            let action: FilterAction = serde_json::from_value(payload.clone()).map_err(|e| {
                parse_error(format!(
                    "vector `{name}` plugin_builder Filter outcome is not a FilterAction: {e}"
                ))
            })?;
            Ok(UnifiedBuilderOutcome::Filter(action))
        }
        other => Err(parse_error(format!(
            "vector `{name}` plugin_builder outcome `{other}` is not Verdict or Filter"
        ))),
    }
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
fn builder_from_json(value: &Value, name: &str) -> Result<UnifiedBuilderCase, VectorError> {
    let builder_name = string_field(value, "name", name)?;
    let requires: Vec<String> = match value.get("requires") {
        None => Vec::new(),
        Some(list) => serde_json::from_value(list.clone()).map_err(|e| {
            parse_error(format!(
                "vector `{name}` plugin_builder requires is not a string array: {e}"
            ))
        })?,
    };
    let mut rings = Vec::new();
    if let Some(list) = value.get("rings").and_then(Value::as_array) {
        for entry in list {
            let tag = entry.get("ring").and_then(Value::as_str).ok_or_else(|| {
                parse_error(format!(
                    "vector `{name}` plugin_builder ring declares no string `ring`"
                ))
            })?;
            let ring = TrampolineRing::from_tag(tag).ok_or_else(|| {
                parse_error(format!(
                    "vector `{name}` plugin_builder ring tag `{tag}` is not a TrampolineRing"
                ))
            })?;
            let ordinal = unsigned_field(entry, "ordinal", name, u32::MAX as u64)? as u32;
            rings.push((ring, ordinal));
        }
    }
    let mut host_rings = Vec::new();
    if let Some(list) = value.get("host_rings").and_then(Value::as_array) {
        for entry in list {
            let tag = entry.get("ring").and_then(Value::as_str).ok_or_else(|| {
                parse_error(format!(
                    "vector `{name}` plugin_builder host ring declares no string `ring`"
                ))
            })?;
            let ring = TrampolineRing::from_tag(tag).ok_or_else(|| {
                parse_error(format!(
                    "vector `{name}` plugin_builder host ring tag `{tag}` is not a TrampolineRing"
                ))
            })?;
            let ordinal = unsigned_field(entry, "ordinal", name, u32::MAX as u64)? as u32;
            host_rings.push((ring, ordinal));
        }
    }
    let scope = match value.get("scope") {
        None | Some(Value::Null) => None,
        Some(table) => {
            let binaries = match table.get("binaries") {
                None | Some(Value::Null) => None,
                Some(list) => Some(serde_json::from_value(list.clone()).map_err(|e| {
                    parse_error(format!(
                        "vector `{name}` plugin_builder scope binaries is not a string array: {e}"
                    ))
                })?),
            };
            let paths = match table.get("paths") {
                None | Some(Value::Null) => None,
                Some(list) => Some(serde_json::from_value(list.clone()).map_err(|e| {
                    parse_error(format!(
                        "vector `{name}` plugin_builder scope paths is not a string array: {e}"
                    ))
                })?),
            };
            Some(UnifiedBuilderScope { binaries, paths })
        }
    };
    let mut events = Vec::new();
    if let Some(list) = value.get("events").and_then(Value::as_array) {
        events.extend(list.iter().cloned());
    }
    let mut expected_outcomes = Vec::new();
    if let Some(list) = value.get("expected_outcomes").and_then(Value::as_array) {
        for entry in list {
            expected_outcomes.push(outcome_from_json(entry, name)?);
        }
    }
    Ok(UnifiedBuilderCase {
        name: builder_name,
        requires,
        rings,
        host_rings,
        scope,
        events,
        expected_outcomes,
    })
}

// see: cans/15-operations.md#Multi-language-parity-gate
impl VectorCase {
    pub fn from_json(value: &Value) -> Result<Self, VectorError> {
        let name = value
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| parse_error("vector entry has no string `name`".to_string()))?
            .to_string();
        let frame = frame_from_json(value, &name)?;
        let event = match optional_table(value, "event") {
            None => None,
            Some(event_json) => {
                let event: GuestToHost =
                    serde_json::from_value(event_json.clone()).map_err(|e| {
                        parse_error(format!("vector `{name}` event is not a GuestToHost: {e}"))
                    })?;
                Some(event)
            }
        };
        let expected_verdict_json = value.get("expected_verdict").ok_or_else(|| {
            parse_error(format!("vector `{name}` declares no `expected_verdict`"))
        })?;
        let expected_verdict: Verdict = serde_json::from_value(expected_verdict_json.clone())
            .map_err(|e| {
                parse_error(format!(
                    "vector `{name}` expected_verdict is not a Verdict: {e}"
                ))
            })?;
        let watchdog = match optional_table(value, "watchdog") {
            None => None,
            Some(watchdog_json) => Some(watchdog_from_json(watchdog_json, &name)?),
        };
        let veto = match optional_table(value, "veto") {
            None => None,
            Some(veto_json) => Some(veto_from_json(veto_json, &name)?),
        };
        let plugin_builder = match optional_table(value, "plugin_builder") {
            None => None,
            Some(builder_json) => Some(builder_from_json(builder_json, &name)?),
        };
        Ok(VectorCase {
            name,
            frame,
            event,
            expected_verdict,
            watchdog,
            veto,
            plugin_builder,
        })
    }
}

// see: cans/15-operations.md#Multi-language-parity-gate
impl VectorSuite {
    pub fn from_json_text(text: &str) -> Result<Self, VectorError> {
        let root: Value = serde_json::from_str(text)
            .map_err(|e| parse_error(format!("vectors.json is not valid JSON: {e}")))?;
        let vectors = root
            .get("vectors")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                parse_error("vectors.json must be an object with a `vectors` array".to_string())
            })?;
        let mut out = Vec::with_capacity(vectors.len());
        for (index, item) in vectors.iter().enumerate() {
            let case = VectorCase::from_json(item)
                .map_err(|e| parse_error(format!("vectors[{index}]: {e}")))?;
            out.push(case);
        }
        Ok(VectorSuite { vectors: out })
    }
}
