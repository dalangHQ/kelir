// see: cans/15-operations.md#Multi-language-parity-gate
use std::collections::BTreeMap;
use std::fs;

use kelir_core::dispatch_plugin::PipelineEngine;
use kelir_core::plugin_registry::resolve_plugin_registrations;
use kelir_interface::codec_protocol::WireFrame;
use kelir_interface::driver_interface::{WatchdogAction, WatchdogEvent, WatchdogVerdict};
use kelir_interface::fusion_plugin::{
    FusedTrampolineDescriptor, HookEntry, HookStage, RingChain, TrampolineRing, TrampolineTable,
};
use kelir_interface::hex_codec::{decode_hex, encode_hex};
use kelir_interface::plugin_model::{PluginRegistration, VetoEvent};
use kelir_interface::protocol_wire::{Channel, FilterAction, GuestToHost, Verdict};
use kelir_plugin_testkit::block_on;
use kelir_plugin_testkit::handlers_testkit::handler_ctx_for_trap;
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
    pub event: Option<Value>,
    pub expected_verdict_kind: String,
    pub watchdog: Option<ParityWatchdog>,
    pub veto: Option<ParityVeto>,
    pub plugin_builder: Option<ParityBuilderCase>,
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

// see: cans/02-plugin-model.md#Mount-scoping
#[derive(Debug, Clone)]
pub struct ParityBuilderScope {
    pub binaries: Option<Vec<String>>,
    pub paths: Option<Vec<String>>,
}

// see: cans/02-plugin-model.md#Unified-verdict
#[derive(Debug, Clone)]
pub enum ParityOutcome {
    Verdict(Verdict),
    Filter(FilterAction),
}

// see: cans/15-operations.md#Unified-builder-parity-vectors
#[derive(Debug, Clone)]
pub struct ParityBuilderCase {
    pub name: String,
    pub requires: Vec<String>,
    pub rings: Vec<(TrampolineRing, u32)>,
    pub host_rings: Vec<(TrampolineRing, u32)>,
    pub scope: Option<ParityBuilderScope>,
    pub events: Vec<Value>,
    pub expected_outcomes: Vec<ParityOutcome>,
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

// see: cans/15-operations.md#Unified-builder-parity-vectors
fn parse_ring_entries(
    value: Option<&Value>,
    index: usize,
    field: &str,
) -> Result<Vec<(TrampolineRing, u32)>, String> {
    let mut entries = Vec::new();
    for entry in value.and_then(Value::as_array).into_iter().flatten() {
        let tag = entry.get("ring").and_then(Value::as_str).ok_or(format!(
            "parity: vector {index} plugin_builder `{field}` entry has no string `ring`"
        ))?;
        let ring = TrampolineRing::from_tag(tag).ok_or(format!(
            "parity: vector {index} plugin_builder `{field}` ring `{tag}` is not a TrampolineRing"
        ))?;
        let ordinal = entry
            .get("ordinal")
            .and_then(Value::as_u64)
            .and_then(|ordinal| u32::try_from(ordinal).ok())
            .ok_or(format!(
                "parity: vector {index} plugin_builder `{field}` entry has no u32 `ordinal`"
            ))?;
        entries.push((ring, ordinal));
    }
    Ok(entries)
}

// see: cans/15-operations.md#Unified-builder-parity-vectors
fn parse_builder_outcome(value: &Value, index: usize) -> Result<ParityOutcome, String> {
    let Value::Object(entries) = value else {
        return Err(format!(
            "parity: vector {index} plugin_builder outcome is not a tagged variant"
        ));
    };
    if entries.len() != 1 {
        return Err(format!(
            "parity: vector {index} plugin_builder outcome must carry exactly one variant"
        ));
    }
    let Some((kind, payload)) = entries.iter().next() else {
        return Err(format!(
            "parity: vector {index} plugin_builder outcome must carry exactly one variant"
        ));
    };
    match kind.as_str() {
        "Verdict" => {
            let verdict: Verdict = serde_json::from_value(payload.clone()).map_err(|e| {
                format!("parity: vector {index} plugin_builder Verdict outcome is malformed: {e}")
            })?;
            Ok(ParityOutcome::Verdict(verdict))
        }
        "Filter" => {
            let action: FilterAction = serde_json::from_value(payload.clone()).map_err(|e| {
                format!("parity: vector {index} plugin_builder Filter outcome is malformed: {e}")
            })?;
            Ok(ParityOutcome::Filter(action))
        }
        other => Err(format!(
            "parity: vector {index} plugin_builder outcome `{other}` is neither Verdict nor Filter"
        )),
    }
}

// see: cans/15-operations.md#Unified-builder-parity-vectors
fn parse_plugin_builder(value: &Value, index: usize) -> Result<Option<ParityBuilderCase>, String> {
    let Value::Object(entries) = value else {
        return Err(format!(
            "parity: vector {index} plugin_builder must be an object"
        ));
    };
    let name = entries
        .get("name")
        .and_then(Value::as_str)
        .ok_or(format!(
            "parity: vector {index} plugin_builder has no string `name`"
        ))?
        .to_string();
    let requires = entries
        .get("requires")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    let rings = parse_ring_entries(entries.get("rings"), index, "rings")?;
    let host_rings = parse_ring_entries(entries.get("host_rings"), index, "host_rings")?;
    let scope = match entries.get("scope") {
        None | Some(Value::Null) => None,
        Some(scope) => Some(ParityBuilderScope {
            binaries: scope
                .get("binaries")
                .and_then(Value::as_array)
                .map(|binaries| {
                    binaries
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                }),
            paths: scope.get("paths").and_then(Value::as_array).map(|paths| {
                paths
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            }),
        }),
    };
    let events = entries
        .get("events")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .cloned()
        .collect();
    let expected_outcomes = entries
        .get("expected_outcomes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|outcome| parse_builder_outcome(outcome, index))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(ParityBuilderCase {
        name,
        requires,
        rings,
        host_rings,
        scope,
        events,
        expected_outcomes,
    }))
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
        let event = item
            .get("event")
            .cloned()
            .filter(|event| !matches!(event, Value::Null));
        let event_kind = match &event {
            None => None,
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
        let plugin_builder = match item.get("plugin_builder") {
            None | Some(Value::Null) => None,
            Some(builder) => parse_plugin_builder(builder, index)?,
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
            event,
            expected_verdict_kind: verdict_kind(expected_verdict, index)?,
            watchdog,
            veto,
            plugin_builder,
        });
    }
    Ok(out)
}

// see: cans/artifacts/code/wire-protocol.rs#MuxFrameHeader
fn assert_frame_shape(vector: &ParityVector) -> Result<(), String> {
    let transport = Channel::from_u16(vector.transport).ok_or(format!(
        "parity: vector `{}` transport {} is not a registered transport",
        vector.name, vector.transport
    ))?;
    kelir_interface::protocol_wire::MsgType::from_u16(vector.msg_type).ok_or(format!(
        "parity: vector `{}` msg_type {:#06x} is not a core message type",
        vector.name, vector.msg_type
    ))?;
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
    let decoded = WireFrame::decode(transport, &frame).map_err(|e| {
        format!(
            "parity: vector `{}` frame does not decode: {e}",
            vector.name
        )
    })?;
    if decoded.stream_id != vector.stream_id
        || decoded.flags != vector.flags
        || decoded.msg_type as u16 != vector.msg_type
        || encode_hex(&decoded.payload) != vector.payload_hex
    {
        return Err(format!(
            "parity: vector `{}` decoded frame fields diverge from the declared frame",
            vector.name
        ));
    }
    let encoded = decoded
        .encode()
        .map_err(|e| format!("parity: vector `{}` re-encode failed: {e}", vector.name))?;
    if encode_hex(&encoded) != vector.frame_hex {
        return Err(format!(
            "parity: vector `{}` frame round-trip diverges: the codec re-encode is not byte-identical",
            vector.name
        ));
    }
    Ok(())
}

// see: cans/15-operations.md#Multi-language-parity-gate
fn assert_event_round_trip(vector: &ParityVector) -> Result<(), String> {
    if vector.watchdog.is_some() || vector.veto.is_some() {
        return Ok(());
    }
    let Some(event_json) = &vector.event else {
        return Ok(());
    };
    let payload = decode_hex(&vector.payload_hex).map_err(|e| {
        format!(
            "parity: vector `{}` payload_cbor_hex is not valid hex: {e}",
            vector.name
        )
    })?;
    let decoded: GuestToHost = ciborium::de::from_reader(payload.as_slice()).map_err(|e| {
        format!(
            "parity: vector `{}` payload does not decode as a GuestToHost event: {e}",
            vector.name
        )
    })?;
    let encoded_event = serde_json::to_value(&decoded).map_err(|e| {
        format!(
            "parity: vector `{}` decoded event does not serialize: {e}",
            vector.name
        )
    })?;
    if &encoded_event != event_json {
        return Err(format!(
            "parity: vector `{}` event JSON diverges from the wire payload decode",
            vector.name
        ));
    }
    Ok(())
}

// see: cans/15-operations.md#Unified-builder-parity-vectors
// see: cans/artifacts/code/plugin-builder-vectors.rs#plugin_builder_fs_write_trap_case
fn assert_builder_ordinals(case: &ParityBuilderCase, name: &str) -> Result<(), String> {
    let host_len = case.host_rings.len();
    for (index, (_, ordinal)) in case.host_rings.iter().enumerate() {
        if *ordinal != index as u32 {
            return Err(format!(
                "parity: vector `{name}` host hook {index} declares ordinal {ordinal}; host hooks occupy chain positions 0..{host_len}"
            ));
        }
    }
    for (index, (_, ordinal)) in case.rings.iter().enumerate() {
        let expected = (host_len + index) as u32;
        if *ordinal != expected {
            return Err(format!(
                "parity: vector `{name}` plugin hook {index} declares ordinal {ordinal}; the chain continues at {expected} after {host_len} host hook(s)"
            ));
        }
    }
    Ok(())
}

// see: cans/15-operations.md#Unified-builder-parity-vectors
fn builder_descriptor(case: &ParityBuilderCase) -> FusedTrampolineDescriptor {
    let mut chains: Vec<RingChain> = Vec::new();
    let mut push_hook =
        |ring: TrampolineRing, ordinal: u32, plugin_name: Option<String>| match chains
            .iter_mut()
            .find(|chain| chain.ring == ring)
        {
            Some(chain) => chain.hooks.push(HookEntry {
                ordinal,
                stage: HookStage::Single,
                plugin_name,
                filter: None,
                lifecycle_tag: None,
                custom_action_name: None,
                vsock_kind: None,
            }),
            None => chains.push(RingChain {
                ring,
                hooks: vec![HookEntry {
                    ordinal,
                    stage: HookStage::Single,
                    plugin_name,
                    filter: None,
                    lifecycle_tag: None,
                    custom_action_name: None,
                    vsock_kind: None,
                }],
            }),
        };
    for (ring, ordinal) in &case.host_rings {
        push_hook(*ring, *ordinal, None);
    }
    for (ring, ordinal) in &case.rings {
        let plugin_name = Some(case.name.clone());
        push_hook(*ring, *ordinal, plugin_name);
    }
    FusedTrampolineDescriptor { rings: chains }
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

// see: cans/15-operations.md#Multi-language-parity-gate
const POLICY_PENDING: &str = "trap verdict evaluation needs the SDK-side policy handler (L3 codegen); the L2 engine dispatch returned Pass for the handler-less chain";

// see: cans/15-operations.md#Multi-language-parity-gate
const WATCHDOG_PENDING: &str =
    "watchdog verdict evaluation needs the SDK-side policy handler (L3 codegen)";

// see: cans/15-operations.md#Multi-language-parity-gate
const VETO_PENDING: &str =
    "veto firing evaluation needs the lifecycle consult (L3); registration and callback executed";

// see: cans/15-operations.md#Multi-language-parity-gate
const HEARTBEAT_PENDING: &str =
    "heartbeat observation evaluation needs the telemetry dispatch loop (L3)";

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
        assert_event_round_trip(vector)?;
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
        if let Some(builder) = &vector.plugin_builder {
            assert_builder_ordinals(builder, &vector.name)?;
            if builder.scope.is_some() != !builder.host_rings.is_empty() {
                return Err(format!(
                    "parity: vector `{}` merge pin is inconsistent: a scope requires host hooks and host hooks require a scope",
                    vector.name
                ));
            }
            let mounts = builder
                .requires
                .iter()
                .map(|dep| PluginRegistration {
                    name: dep.clone(),
                    enabled: true,
                })
                .collect::<Vec<_>>();
            resolve_plugin_registrations(&mounts).map_err(|e| {
                format!(
                    "parity: vector `{}` prerequisite mount failed: {e}",
                    vector.name
                )
            })?;
            let descriptor = builder_descriptor(builder);
            TrampolineTable::from_descriptor(&descriptor).map_err(|e| {
                format!(
                    "parity: vector `{}` fused descriptor rejected: {e}",
                    vector.name
                )
            })?;
            let engine = PipelineEngine::new().with_trampoline(descriptor);
            let verdicts = builder
                .expected_outcomes
                .iter()
                .filter_map(|outcome| match outcome {
                    ParityOutcome::Verdict(verdict) => Some(verdict.clone()),
                    ParityOutcome::Filter(_) => None,
                })
                .collect::<Vec<_>>();
            let mut trap_events = 0;
            for event_json in &builder.events {
                let event: GuestToHost =
                    serde_json::from_value(event_json.clone()).map_err(|e| {
                        format!(
                            "parity: vector `{}` event is not a wire event: {e}",
                            vector.name
                        )
                    })?;
                match event {
                    GuestToHost::TrapEvent(trap) => {
                        let ctx = handler_ctx_for_trap([0u8; 16], &trap);
                        let observed =
                            block_on(engine.dispatch_trap(&trap, &ctx)).map_err(|e| {
                                format!(
                                    "parity: vector `{}` engine dispatch failed: {e}",
                                    vector.name
                                )
                            })?;
                        let expected = verdicts.get(trap_events);
                        trap_events += 1;
                        if expected.is_some_and(|expected| *expected == observed) {
                            report.verdicts_executed += 1;
                        } else {
                            report
                                .verdicts_not_executable
                                .push((vector.name.clone(), POLICY_PENDING));
                        }
                    }
                    GuestToHost::ProcessExit(process_exit) => {
                        let ctx = kelir_plugin_testkit::handlers_testkit::handler_ctx(
                            [0u8; 16],
                            process_exit.pid,
                        );
                        block_on(engine.dispatch_process_exit(&process_exit, &ctx)).map_err(
                            |e| {
                                format!(
                                    "parity: vector `{}` engine dispatch failed: {e}",
                                    vector.name
                                )
                            },
                        )?;
                        report
                            .verdicts_not_executable
                            .push((vector.name.clone(), PROCESS_EXIT_PENDING));
                    }
                    _ => {
                        report.verdicts_not_executable.push((
                            vector.name.clone(),
                            "wire event has no L2 engine dispatch; ring wiring executed only",
                        ));
                    }
                }
            }
            if builder.events.is_empty() {
                report
                    .verdicts_not_executable
                    .push((vector.name.clone(), STRUCTURAL_ONLY));
            }
            continue;
        }
        let Some(event) = &vector.event else {
            let reason = if vector.veto.is_some() {
                VETO_PENDING
            } else if vector.watchdog.is_some() {
                WATCHDOG_PENDING
            } else {
                HEARTBEAT_PENDING
            };
            report
                .verdicts_not_executable
                .push((vector.name.clone(), reason));
            continue;
        };
        let event: GuestToHost = serde_json::from_value(event.clone()).map_err(|e| {
            format!(
                "parity: vector `{}` event does not decode: {e}",
                vector.name
            )
        })?;
        match event {
            GuestToHost::TrapEvent(trap) => {
                let ctx = handler_ctx_for_trap([0u8; 16], &trap);
                let observed =
                    block_on(PipelineEngine::new().dispatch_trap(&trap, &ctx)).map_err(|e| {
                        format!(
                            "parity: vector `{}` engine dispatch failed: {e}",
                            vector.name
                        )
                    })?;
                if matches!(observed, Verdict::Pass) && vector.expected_verdict_kind == "Pass" {
                    report.verdicts_executed += 1;
                } else {
                    report
                        .verdicts_not_executable
                        .push((vector.name.clone(), POLICY_PENDING));
                }
            }
            _ => {
                let reason = if vector.veto.is_some() {
                    VETO_PENDING
                } else if vector.watchdog.is_some() {
                    WATCHDOG_PENDING
                } else {
                    HEARTBEAT_PENDING
                };
                report
                    .verdicts_not_executable
                    .push((vector.name.clone(), reason));
            }
        }
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
const PROCESS_EXIT_PENDING: &str =
    "process-exit observation has no Verdict outcome; the engine dispatch executed";

// see: cans/15-operations.md#Multi-language-parity-gate
const STRUCTURAL_ONLY: &str =
    "no wire event on this chain; descriptor construction and prerequisite mounts executed only";

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
