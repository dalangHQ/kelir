// see: cans/15-operations.md#Multi-language-parity-gate
use std::collections::BTreeMap;
use std::fs;

use kelir_interface::driver_interface::{WatchdogAction, WatchdogEvent, WatchdogVerdict};
use kelir_interface::hex_codec::{decode_hex, encode_hex};
use kelir_interface::plugin_model::VetoEvent;
use kelir_interface::protocol_wire::Channel;
use kelir_plugin_testkit::vectors_testkit::{VetoCase, arm_watchdog_case, register_veto_case};
use serde_json::Value;

// see: cans/15-operations.md#Multi-language-parity-gate
#[derive(Debug, Clone)]
pub struct ParityVector {
    pub name: String,
    pub transport: u16,
    pub stream_id: u32,
    pub flags: u8,
    pub msg_type: u16,
    pub payload_hex: String,
    pub frame_hex: String,
    pub event_kind: Option<String>,
    pub expected_verdict_kind: String,
    pub watchdog: Option<ParityWatchdog>,
    pub veto: Option<ParityVeto>,
}

// see: cans/08-lifecycle.md#Watchdog
#[derive(Debug, Clone)]
pub struct ParityWatchdog {
    pub event: WatchdogEvent,
    pub expected_verdict: WatchdogVerdict,
}

// see: cans/08-lifecycle.md#Lifecycle-vetoes
#[derive(Debug, Clone)]
pub struct ParityVeto {
    pub event: VetoEvent,
    pub callback_allows: Option<bool>,
}

// see: cans/02-plugin-model.md#Unified-verdict
const VERDICT_VARIANTS: &[&str] = &["Pass", "Patch", "Drop", "Suspend", "Abort"];

// see: cans/15-operations.md#Coverage-gates
const WATCHDOG_VERDICTS: [WatchdogVerdict; 2] =
    [WatchdogVerdict::Suppress, WatchdogVerdict::Mitigate];

// see: cans/15-operations.md#Coverage-gates
const VETO_EVENTS: [VetoEvent; 3] = [VetoEvent::Fork, VetoEvent::Suspend, VetoEvent::ClockSkew];

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

fn parse_watchdog_action(value: &Value, index: usize) -> Result<WatchdogAction, String> {
    match value {
        Value::String(kind) if kind == "Reboot" => Ok(WatchdogAction::Reboot),
        Value::Object(entries) => {
            if entries.len() != 1 || !entries.contains_key("Terminate") {
                return Err(format!(
                    "parity: vector {index} watchdog action is not Reboot or Terminate"
                ));
            }
            let emit_diagnostic = entries
                .get("Terminate")
                .and_then(|action| action.get("emit_diagnostic"))
                .and_then(Value::as_bool)
                .ok_or(format!(
                    "parity: vector {index} watchdog Terminate action has no bool `emit_diagnostic`"
                ))?;
            Ok(WatchdogAction::Terminate { emit_diagnostic })
        }
        _ => Err(format!(
            "parity: vector {index} watchdog action is not Reboot or Terminate"
        )),
    }
}

// see: cans/08-lifecycle.md#Watchdog
fn parse_watchdog_event(value: &Value, index: usize) -> Result<WatchdogEvent, String> {
    let timeout_ms = value
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .ok_or(format!(
            "parity: vector {index} watchdog event has no u64 `timeout_ms`"
        ))?;
    let action = parse_watchdog_action(
        value.get("action").ok_or(format!(
            "parity: vector {index} watchdog event has no `action`"
        ))?,
        index,
    )?;
    let fired_at_ns = value
        .get("fired_at_ns")
        .and_then(Value::as_u64)
        .ok_or(format!(
            "parity: vector {index} watchdog event has no u64 `fired_at_ns`"
        ))?;
    let last_heartbeat_ms = value
        .get("last_heartbeat_ms")
        .and_then(Value::as_u64)
        .ok_or(format!(
            "parity: vector {index} watchdog event has no u64 `last_heartbeat_ms`"
        ))?;
    let last_active_pid = match value.get("last_active_pid") {
        None | Some(Value::Null) => None,
        Some(pid) => Some(
            u32::try_from(pid.as_u64().ok_or(format!(
                "parity: vector {index} watchdog `last_active_pid` is not a u32"
            ))?)
            .map_err(|_| {
                format!("parity: vector {index} watchdog `last_active_pid` is not a u32")
            })?,
        ),
    };
    Ok(WatchdogEvent {
        timeout_ms,
        action,
        fired_at_ns,
        last_heartbeat_ms,
        last_active_pid,
    })
}

// see: cans/08-lifecycle.md#Watchdog
fn parse_watchdog_verdict(verdict: &str, index: usize) -> Result<WatchdogVerdict, String> {
    match verdict {
        "Suppress" => Ok(WatchdogVerdict::Suppress),
        "Mitigate" => Ok(WatchdogVerdict::Mitigate),
        other => Err(format!(
            "parity: vector {index} watchdog verdict `{other}` is not Suppress or Mitigate"
        )),
    }
}

// see: cans/08-lifecycle.md#Lifecycle-vetoes
fn parse_veto_event(event: &str, index: usize) -> Result<VetoEvent, String> {
    match event {
        "Fork" => Ok(VetoEvent::Fork),
        "Suspend" => Ok(VetoEvent::Suspend),
        "ClockSkew" => Ok(VetoEvent::ClockSkew),
        other => Err(format!(
            "parity: vector {index} veto event `{other}` is not a VetoEvent"
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
        let event_kind = match item.get("event") {
            None | Some(Value::Null) => None,
            Some(Value::Object(entries)) if entries.len() == 1 => {
                entries.iter().next().map(|(kind, _)| kind.clone())
            }
            Some(_) => {
                return Err(format!(
                    "parity: vector {index} event must be a single-variant GuestToHost or null"
                ));
            }
        };
        let expected_verdict = item.get("expected_verdict").ok_or(format!(
            "parity: vector {index} declares no `expected_verdict`"
        ))?;
        let watchdog = match item.get("watchdog") {
            None | Some(Value::Null) => None,
            Some(watchdog) => {
                let event = parse_watchdog_event(
                    watchdog.get("event").ok_or(format!(
                        "parity: vector {index} watchdog declares no `event`"
                    ))?,
                    index,
                )?;
                let verdict = watchdog
                    .get("expected_verdict")
                    .and_then(Value::as_str)
                    .ok_or(format!(
                        "parity: vector {index} watchdog declares no string `expected_verdict`"
                    ))?;
                Some(ParityWatchdog {
                    event,
                    expected_verdict: parse_watchdog_verdict(verdict, index)?,
                })
            }
        };
        let veto = match item.get("veto") {
            None | Some(Value::Null) => None,
            Some(veto) => {
                let event = veto.get("event").and_then(Value::as_str).ok_or(format!(
                    "parity: vector {index} veto declares no string `event`"
                ))?;
                let callback_allows = match veto.get("callback_allows") {
                    None | Some(Value::Null) => None,
                    Some(value) => Some(value.as_bool().ok_or(format!(
                        "parity: vector {index} veto `callback_allows` is not a bool"
                    ))?),
                };
                Some(ParityVeto {
                    event: parse_veto_event(event, index)?,
                    callback_allows,
                })
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
            event_kind,
            expected_verdict_kind: verdict_kind(expected_verdict, index)?,
            watchdog,
            veto,
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
    if kelir_interface::protocol_wire::MsgType::from_u16(vector.msg_type).is_none() {
        return Err(format!(
            "parity: vector `{}` msg_type {:#06x} is not a core message type",
            vector.name, vector.msg_type
        ));
    }
    if vector.stream_id > kelir_interface::codec_protocol::MAX_STREAM_ID {
        return Err(format!(
            "parity: vector `{}` stream_id {} exceeds MAX_STREAM_ID",
            vector.name, vector.stream_id
        ));
    }
    let frame = decode_hex(&vector.frame_hex).map_err(|e| {
        format!(
            "parity: vector `{}` frame_hex is not valid hex: {e}",
            vector.name
        )
    })?;
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
    if body > kelir_interface::codec_protocol::MAX_FRAME_PAYLOAD {
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

// see: cans/15-operations.md#Coverage-gates
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityReport {
    pub total: usize,
    pub frame_validated: usize,
    pub watchdog_armed: usize,
    pub veto_registered: usize,
    pub verdicts_executed: usize,
    pub verdicts_not_executable: Vec<(String, &'static str)>,
}

pub fn assert_vectors(vectors: &[ParityVector]) -> Result<ParityReport, String> {
    if vectors.is_empty() {
        return Err("parity: vectors.json declares 0 vectors; nothing to assert".to_string());
    }
    let mut report = ParityReport {
        total: vectors.len(),
        frame_validated: 0,
        watchdog_armed: 0,
        veto_registered: 0,
        verdicts_executed: 0,
        verdicts_not_executable: Vec::new(),
    };
    for vector in vectors {
        if vector.name.is_empty() {
            return Err("parity: vector with empty name".to_string());
        }
        assert_frame_shape(vector)?;
        report.frame_validated += 1;
        if let Some(watchdog) = &vector.watchdog {
            arm_watchdog_case(&watchdog.event).map_err(|e| {
                format!(
                    "parity: vector `{}` watchdog arming failed: {e}",
                    vector.name
                )
            })?;
            report.watchdog_armed += 1;
        }
        if let Some(veto) = &vector.veto {
            let case = VetoCase {
                event: veto.event,
                callback_allows: veto.callback_allows.unwrap_or(true),
            };
            register_veto_case(&case).map_err(|e| {
                format!(
                    "parity: vector `{}` veto registration failed: {e}",
                    vector.name
                )
            })?;
            report.veto_registered += 1;
        }
        // TODO (L3): dispatch each trap/watchdog/veto chain through the engine and compare the observed verdict against expected_verdict typed; until engine dispatch lands, every verdict evaluation is reported not-executable, never a green pass.
        let reason = if vector.veto.is_some() {
            "veto firing evaluation pending engine dispatch (L3)"
        } else if vector.watchdog.is_some() {
            "watchdog verdict evaluation pending engine dispatch (L3)"
        } else if vector.event_kind.as_deref() == Some("Heartbeat") {
            "heartbeat observation evaluation pending engine dispatch (L3)"
        } else {
            "trap verdict evaluation pending engine dispatch (L3)"
        };
        report
            .verdicts_not_executable
            .push((vector.name.to_string(), reason));
    }
    for verdict in WATCHDOG_VERDICTS {
        if !vectors.iter().any(|vector| {
            vector
                .watchdog
                .as_ref()
                .is_some_and(|watchdog| watchdog.expected_verdict == verdict)
        }) {
            return Err(format!(
                "parity: watchdog coverage missing WatchdogVerdict::{verdict:?} in vectors.json chains"
            ));
        }
    }
    for event in VETO_EVENTS {
        if !vectors
            .iter()
            .any(|vector| vector.veto.as_ref().is_some_and(|veto| veto.event == event))
        {
            return Err(format!(
                "parity: veto coverage missing VetoEvent::{event:?} in vectors.json chains"
            ));
        }
    }
    Ok(report)
}

// see: cans/15-operations.md#Multi-language-parity-gate
fn assert_canonical_roster(vectors: &[ParityVector]) -> Result<(), String> {
    let canonical = kelir_plugin_testkit::fixtures_testkit::canonical_vector_names()
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
    let report = assert_vectors(&vectors)?;
    assert_canonical_roster(&vectors)?;
    println!(
        "parity: {} vector(s) from {path}: frame-validated {}; watchdog-armed {}; veto-registered {}",
        report.total, report.frame_validated, report.watchdog_armed, report.veto_registered
    );
    let mut reasons: BTreeMap<&'static str, usize> = BTreeMap::new();
    for (_, reason) in &report.verdicts_not_executable {
        *reasons.entry(reason).or_default() += 1;
    }
    let summary = reasons
        .iter()
        .map(|(reason, count)| format!("{reason} x{count}"))
        .collect::<Vec<_>>()
        .join("; ");
    println!(
        "parity: verdict evaluations executed {}/{}: not-executable — {summary}",
        report.verdicts_executed, report.total
    );
    Ok(())
}
