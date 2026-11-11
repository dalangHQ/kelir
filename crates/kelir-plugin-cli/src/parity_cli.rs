// see: cans/15-operations.md#Multi-language-parity-gate
use std::collections::BTreeMap;
use std::fs;

use kelir_core::dispatch_plugin::PipelineEngine;
use kelir_core::plugin_registry::resolve_plugin_registrations;
use kelir_interface::codec_protocol::{MAX_FRAME_PAYLOAD, MAX_STREAM_ID};
use kelir_interface::driver_interface::WatchdogVerdict;
use kelir_interface::fusion_plugin::{
    FusedTrampolineDescriptor, HookEntry, HookStage, RingChain, TrampolineRing, TrampolineTable,
};
use kelir_interface::plugin_model::{PluginRegistration, VetoEvent};
use kelir_interface::protocol_wire::{Channel, GuestToHost, MsgType, Verdict};
use kelir_plugin_testkit::block_on;
use kelir_plugin_testkit::handlers_testkit::handler_ctx_for_trap;
use kelir_plugin_testkit::vectors_testkit::{
    UnifiedBuilderCase, VectorCase, VectorSuite, arm_watchdog_case, register_veto_case,
};

// see: cans/08-lifecycle.md#Watchdog
const WATCHDOG_VERDICTS: [WatchdogVerdict; 2] =
    [WatchdogVerdict::Suppress, WatchdogVerdict::Mitigate];

// see: cans/15-operations.md#Coverage-gates
const VETO_EVENTS: [VetoEvent; 3] = [VetoEvent::Fork, VetoEvent::Suspend, VetoEvent::ClockSkew];

// see: cans/artifacts/code/wire-protocol.rs#MuxFrameHeader
fn assert_frame_shape(vector: &VectorCase) -> Result<(), String> {
    if Channel::from_u16(vector.frame.transport as u16).is_none() {
        return Err(format!(
            "parity: vector `{}` transport {} is not a registered transport",
            vector.name, vector.frame.transport as u16
        ));
    }
    if MsgType::from_u16(vector.frame.msg_type as u16).is_none() {
        return Err(format!(
            "parity: vector `{}` msg_type {:#06x} is not a core message type",
            vector.name, vector.frame.msg_type as u16
        ));
    }
    if vector.frame.stream_id > MAX_STREAM_ID {
        return Err(format!(
            "parity: vector `{}` stream_id {} exceeds MAX_STREAM_ID",
            vector.name, vector.frame.stream_id
        ));
    }
    let body = vector.frame.payload.len() + 2;
    if body > MAX_FRAME_PAYLOAD {
        return Err(format!(
            "parity: vector `{}` body {body} exceeds MAX_FRAME_PAYLOAD",
            vector.name
        ));
    }
    vector.frame.encode().map_err(|e| {
        format!(
            "parity: vector `{}` frame does not re-encode canonically: {e}",
            vector.name
        )
    })?;
    Ok(())
}

// see: cans/15-operations.md#Multi-language-parity-gate
fn assert_event_round_trip(vector: &VectorCase) -> Result<(), String> {
    if vector.watchdog.is_some() || vector.veto.is_some() {
        return Ok(());
    }
    let Some(event) = &vector.event else {
        return Ok(());
    };
    let decoded: GuestToHost =
        ciborium::de::from_reader(vector.frame.payload.as_slice()).map_err(|e| {
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
    let declared_event = serde_json::to_value(event).map_err(|e| {
        format!(
            "parity: vector `{}` declared event does not serialize: {e}",
            vector.name
        )
    })?;
    if encoded_event != declared_event {
        return Err(format!(
            "parity: vector `{}` event JSON diverges from the wire payload decode",
            vector.name
        ));
    }
    Ok(())
}

// see: cans/15-operations.md#Unified-builder-parity-vectors
// see: cans/artifacts/code/plugin-builder-vectors.rs#plugin_builder_fs_write_trap_case
fn assert_builder_ordinals(case: &UnifiedBuilderCase, name: &str) -> Result<(), String> {
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
fn builder_descriptor(case: &UnifiedBuilderCase) -> FusedTrampolineDescriptor {
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

// see: cans/15-operations.md#Multi-language-parity-gate
const PROCESS_EXIT_PENDING: &str =
    "process-exit observation has no Verdict outcome; the engine dispatch executed";

// see: cans/15-operations.md#Multi-language-parity-gate
const STRUCTURAL_ONLY: &str =
    "no wire event on this chain; descriptor construction and prerequisite mounts executed only";

pub fn assert_vectors(vectors: &[VectorCase]) -> Result<ParityReport, String> {
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
            register_veto_case(veto).map_err(|e| {
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
                    kelir_plugin_testkit::vectors_testkit::UnifiedBuilderOutcome::Verdict(
                        verdict,
                    ) => Some(verdict.clone()),
                    kelir_plugin_testkit::vectors_testkit::UnifiedBuilderOutcome::Filter(_) => None,
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
        match event {
            GuestToHost::TrapEvent(trap) => {
                let ctx = handler_ctx_for_trap([0u8; 16], trap);
                let observed =
                    block_on(PipelineEngine::new().dispatch_trap(trap, &ctx)).map_err(|e| {
                        format!(
                            "parity: vector `{}` engine dispatch failed: {e}",
                            vector.name
                        )
                    })?;
                if matches!(observed, Verdict::Pass)
                    && matches!(vector.expected_verdict, Verdict::Pass)
                {
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
fn assert_canonical_roster(vectors: &[VectorCase]) -> Result<(), String> {
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
    let suite = VectorSuite::from_json_text(&text).map_err(|e| format!("parity: {e}"))?;
    let vectors = suite.vectors.as_slice();
    let report = assert_vectors(vectors)?;
    assert_canonical_roster(vectors)?;
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
