// see: cans/15-operations.md#Multi-language-parity-gate
use std::collections::BTreeMap;
use std::fs;

use kelir_interface::codec_protocol::{MAX_FRAME_PAYLOAD, MAX_STREAM_ID};
use kelir_interface::driver_interface::WatchdogVerdict;
use kelir_interface::plugin_model::VetoEvent;
use kelir_interface::protocol_wire::{Channel, GuestToHost, MsgType};
use kelir_plugin_testkit::vectors_testkit::{
    VectorCase, VectorSuite, arm_watchdog_case, register_veto_case,
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
        // TODO (L3): dispatch each trap/watchdog/veto chain through the engine and compare the observed verdict against expected_verdict typed; until engine dispatch lands, every verdict evaluation is reported not-executable, never a green pass.
        let reason = if vector.veto.is_some() {
            "veto firing evaluation pending engine dispatch (L3)"
        } else if vector.watchdog.is_some() {
            "watchdog verdict evaluation pending engine dispatch (L3)"
        } else if vector
            .event
            .as_ref()
            .is_some_and(|event| matches!(event, GuestToHost::Heartbeat(_)))
        {
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
