// see: cans/02-plugin-model.md#Plugin-testkit
// see: cans/15-operations.md#Multi-language-parity-gate
// see: cans/artifacts/code/wire-protocol.rs#MuxFrameHeader
// see: cans/artifacts/code/messages.rs#GuestToHost
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use kelir_interface::codec_protocol::{CodecError, FLAG_DATA};
use kelir_interface::driver_interface::{WatchdogAction, WatchdogEvent, WatchdogVerdict};
use kelir_interface::fusion_plugin::TrampolineRing;
use kelir_interface::hex_codec::encode_hex;
use kelir_interface::plugin_model::VetoEvent;
use kelir_interface::protocol_wire::{
    Channel, ExecveMutation, FilterAction, GuestToHost, Heartbeat, MsgType, PatchPayload,
    SuspendUntil, TrapEvent, Verdict,
};

use crate::handlers_testkit::LifecycleCtxFixture;

// see: cans/11-protocol.md#Wire-framing
pub use kelir_interface::codec_protocol::WireFrame;

// see: cans/11-protocol.md#Wire-framing
#[derive(Debug)]
pub enum VectorError {
    FrameTooLarge { len: usize },
    StreamIdOutOfRange { stream_id: u32 },
    Encode(String),
    Execution(String),
    Parse(String),
}

impl fmt::Display for VectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VectorError::FrameTooLarge { len } => {
                write!(f, "vector frame payload {len} exceeds MAX_FRAME_PAYLOAD")
            }
            VectorError::StreamIdOutOfRange { stream_id } => {
                write!(
                    f,
                    "vector frame stream_id {stream_id} exceeds MAX_STREAM_ID"
                )
            }
            VectorError::Encode(message) => write!(f, "vector encode failure: {message}"),
            VectorError::Execution(message) => write!(f, "vector execution failure: {message}"),
            VectorError::Parse(message) => write!(f, "vector parse failure: {message}"),
        }
    }
}

impl Error for VectorError {}

// see: cans/11-protocol.md#Wire-framing
impl From<CodecError> for VectorError {
    fn from(err: CodecError) -> Self {
        match err {
            CodecError::FrameTooLarge { len } => VectorError::FrameTooLarge { len },
            CodecError::StreamIdOutOfRange { stream_id } => {
                VectorError::StreamIdOutOfRange { stream_id }
            }
            other => VectorError::Encode(other.to_string()),
        }
    }
}

// see: cans/15-operations.md#Multi-language-parity-gate
// see: cans/08-lifecycle.md#Watchdog
#[derive(Debug, Clone)]
pub struct WatchdogCase {
    pub event: WatchdogEvent,
    pub expected_verdict: WatchdogVerdict,
}

// see: cans/15-operations.md#Multi-language-parity-gate
// see: cans/08-lifecycle.md#Lifecycle-vetoes
#[derive(Debug, Clone)]
pub struct VetoCase {
    pub event: VetoEvent,
    pub callback_allows: bool,
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
#[derive(Debug, Clone)]
pub enum UnifiedBuilderOutcome {
    Verdict(Verdict),
    Filter(FilterAction),
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
#[derive(Debug, Clone)]
pub struct UnifiedBuilderCase {
    pub name: String,
    pub requires: Vec<String>,
    pub rings: Vec<(TrampolineRing, u32)>,
    pub events: Vec<serde_json::Value>,
    pub expected_outcomes: Vec<UnifiedBuilderOutcome>,
}

impl UnifiedBuilderCase {
    pub fn to_json(&self) -> Result<serde_json::Value, VectorError> {
        let mut rings: Vec<serde_json::Value> = Vec::with_capacity(self.rings.len());
        for (ring, ordinal) in &self.rings {
            rings.push(serde_json::json!({"ring": ring.as_tag(), "ordinal": ordinal}));
        }
        let mut outcomes: Vec<serde_json::Value> = Vec::with_capacity(self.expected_outcomes.len());
        for outcome in &self.expected_outcomes {
            let value = match outcome {
                UnifiedBuilderOutcome::Verdict(verdict) => {
                    serde_json::json!({"Verdict": serde_json::to_value(verdict).map_err(|e| VectorError::Encode(e.to_string()))?})
                }
                UnifiedBuilderOutcome::Filter(action) => {
                    serde_json::json!({"Filter": serde_json::to_value(action).map_err(|e| VectorError::Encode(e.to_string()))?})
                }
            };
            outcomes.push(value);
        }
        Ok(serde_json::json!({
            "name": self.name,
            "requires": self.requires,
            "rings": rings,
            "events": self.events,
            "expected_outcomes": outcomes,
        }))
    }
}

// see: cans/15-operations.md#Multi-language-parity-gate
#[derive(Debug)]
pub struct VectorCase {
    pub name: String,
    pub frame: WireFrame,
    pub event: Option<GuestToHost>,
    pub expected_verdict: Verdict,
    pub watchdog: Option<WatchdogCase>,
    pub veto: Option<VetoCase>,
    pub plugin_builder: Option<UnifiedBuilderCase>,
}

impl VectorCase {
    pub fn to_json(&self) -> Result<serde_json::Value, VectorError> {
        let frame = self.frame.encode()?;
        let mut value = serde_json::json!({
            "name": self.name,
            "frame": {
                "transport": self.frame.transport as u16,
                "stream_id": self.frame.stream_id,
                "flags": self.frame.flags,
                "msg_type": self.frame.msg_type as u16,
                "payload_cbor_hex": encode_hex(&self.frame.payload),
                "frame_hex": encode_hex(&frame),
            },
            "event": serde_json::to_value(&self.event).map_err(|e| VectorError::Encode(e.to_string()))?,
            "expected_verdict": serde_json::to_value(&self.expected_verdict).map_err(|e| VectorError::Encode(e.to_string()))?,
        });
        if let Some(watchdog) = &self.watchdog {
            let entry = serde_json::json!({
                "event": serde_json::to_value(&watchdog.event).map_err(|e| VectorError::Encode(e.to_string()))?,
                "expected_verdict": serde_json::to_value(watchdog.expected_verdict).map_err(|e| VectorError::Encode(e.to_string()))?,
            });
            if let Some(object) = value.as_object_mut() {
                object.insert("watchdog".to_string(), entry);
            }
        }
        if let Some(veto) = &self.veto {
            let entry = serde_json::json!({
                "event": serde_json::to_value(veto.event).map_err(|e| VectorError::Encode(e.to_string()))?,
                "callback_allows": veto.callback_allows,
            });
            if let Some(object) = value.as_object_mut() {
                object.insert("veto".to_string(), entry);
            }
        }
        if let Some(builder) = &self.plugin_builder {
            let entry = builder.to_json()?;
            if let Some(object) = value.as_object_mut() {
                object.insert("plugin_builder".to_string(), entry);
            }
        }
        Ok(value)
    }
}

// see: cans/15-operations.md#Multi-language-parity-gate
#[derive(Debug, Default)]
pub struct VectorSuite {
    pub vectors: Vec<VectorCase>,
}

impl VectorSuite {
    pub fn to_canonical_json(&self) -> Result<String, VectorError> {
        let mut vectors = Vec::with_capacity(self.vectors.len());
        for case in &self.vectors {
            vectors.push(case.to_json()?);
        }
        let root = serde_json::json!({ "vectors": vectors });
        serde_json::to_string(&root).map_err(|e| VectorError::Encode(e.to_string()))
    }

    // see: cans/02-plugin-model.md#Plugin-testkit
    pub fn fixtures() -> Result<VectorSuite, VectorError> {
        Ok(VectorSuite {
            vectors: {
                let mut vectors = vec![
                    trap_case(
                        "trap-git-commit-pass",
                        trap_event(
                            1,
                            1234,
                            100,
                            "/usr/bin/git",
                            vec!["git".to_string(), "commit".to_string()],
                            HashMap::from([
                                ("KELIR_PARAMS".to_string(), "{}".to_string()),
                                ("PATH".to_string(), "/usr/bin:/bin".to_string()),
                            ]),
                        ),
                        Verdict::Pass,
                    )?,
                    trap_case(
                        "trap-git-push-deny-epolicy",
                        trap_event(
                            2,
                            1234,
                            100,
                            "/usr/bin/git",
                            vec!["git".to_string(), "push".to_string(), "--force".to_string()],
                            HashMap::new(),
                        ),
                        Verdict::Drop {
                            errno: Some("EPERM".to_string()),
                            reason: "EPOLICY: force push denied".to_string(),
                        },
                    )?,
                    exec_rewrite_case()?,
                    trap_case(
                        "trap-mutating-tool-suspend-approval",
                        trap_event(
                            4,
                            2401,
                            100,
                            "/workspace/scripts/deploy.py",
                            vec!["deploy.py".to_string(), "--migrate".to_string()],
                            HashMap::new(),
                        ),
                        Verdict::Suspend {
                            until: SuspendUntil::Approval,
                        },
                    )?,
                    trap_case(
                        "trap-clock-skew-deadline-suspend",
                        trap_event(
                            5,
                            2402,
                            1234,
                            "/usr/bin/date",
                            vec!["date".to_string()],
                            HashMap::new(),
                        ),
                        Verdict::Suspend {
                            until: SuspendUntil::Deadline(60_000),
                        },
                    )?,
                    trap_case(
                        "trap-unverified-caller-abort-ewxviolation",
                        trap_event(
                            6,
                            2403,
                            100,
                            "/workspace/scratch/loader",
                            vec!["loader".to_string()],
                            HashMap::new(),
                        ),
                        Verdict::Abort {
                            reason: "EWXVIOLATION: unverified caller binary".to_string(),
                        },
                    )?,
                    heartbeat_case()?,
                    watchdog_case(
                        "watchdog-reboot-suppress",
                        WatchdogEvent {
                            timeout_ms: 5_000,
                            action: WatchdogAction::Reboot,
                            fired_at_ns: 60_000_000_000,
                            last_heartbeat_ms: 55_000,
                            last_active_pid: None,
                        },
                        WatchdogVerdict::Suppress,
                    )?,
                    watchdog_case(
                        "watchdog-terminate-mitigate",
                        WatchdogEvent {
                            timeout_ms: 5_000,
                            action: WatchdogAction::Terminate {
                                emit_diagnostic: true,
                            },
                            fired_at_ns: 60_000_000_000,
                            last_heartbeat_ms: 55_000,
                            last_active_pid: Some(1234),
                        },
                        WatchdogVerdict::Mitigate,
                    )?,
                    veto_case("veto-fork", VetoEvent::Fork, false)?,
                    veto_case("veto-suspend", VetoEvent::Suspend, false)?,
                    veto_case("veto-clock-skew", VetoEvent::ClockSkew, false)?,
                    plugin_builder_trap_case()?,
                    plugin_builder_fs_write_trap_case()?,
                ];
                vectors.extend(plugin_builder_verb_cases()?);
                vectors.extend(plugin_builder_nested_cases()?);
                vectors.extend(plugin_builder_merge_cases()?);
                vectors
            },
        })
    }
}

// see: cans/15-operations.md#Multi-language-parity-gate
// TODO (L3): canonicalize CBOR map ordering (envp HashMap iteration) so `kelir-plugin-cli vectors` regenerates byte-stable frame_hex literals.
fn cbor<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, VectorError> {
    let mut out = Vec::new();
    ciborium::ser::into_writer(value, &mut out).map_err(|e| VectorError::Encode(e.to_string()))?;
    Ok(out)
}

fn trap_event(
    seq: u64,
    pid: u32,
    ppid: u32,
    binary_path: &str,
    argv: Vec<String>,
    envp: HashMap<String, String>,
) -> TrapEvent {
    let mut event = TrapEvent::new(seq, pid, ppid, binary_path);
    event.task_id = seq;
    event.binary_dev = 2049;
    event.binary_ino = 400_000 + seq * 7;
    event.binary_blake3 = [0x42; 32];
    event.argv = argv;
    event.envp = envp;
    event.cwd = "/workspace".to_string();
    event
}

fn trap_case(
    name: &str,
    event: TrapEvent,
    expected_verdict: Verdict,
) -> Result<VectorCase, VectorError> {
    let event = GuestToHost::TrapEvent(event);
    Ok(VectorCase {
        name: name.to_string(),
        frame: WireFrame {
            transport: Channel::Control,
            stream_id: 0,
            flags: FLAG_DATA,
            msg_type: MsgType::Trap,
            payload: cbor(&event)?,
        },
        event: Some(event),
        expected_verdict,
        watchdog: None,
        veto: None,
        plugin_builder: None,
    })
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
fn plugin_builder_trap_case() -> Result<VectorCase, VectorError> {
    let event = trap_event(
        7,
        1234,
        100,
        "/usr/bin/git",
        vec!["git".to_string(), "commit".to_string()],
        HashMap::new(),
    );
    let event_json = serde_json::to_value(GuestToHost::TrapEvent(event.clone()))
        .map_err(|e| VectorError::Encode(e.to_string()))?;
    let mut case = trap_case("plugin-builder-trap-only", event, Verdict::Pass)?;
    case.plugin_builder = Some(UnifiedBuilderCase {
        name: "plugin-builder-trap-only".to_string(),
        requires: Vec::new(),
        rings: vec![(TrampolineRing::Trap, 0)],
        events: vec![event_json],
        expected_outcomes: vec![UnifiedBuilderOutcome::Verdict(Verdict::Pass)],
    });
    Ok(case)
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
fn plugin_builder_fs_write_trap_case() -> Result<VectorCase, VectorError> {
    let event = trap_event(
        8,
        1300,
        100,
        "/bin/sh",
        vec!["sh".to_string(), "-c".to_string(), "echo hi".to_string()],
        HashMap::new(),
    );
    let event_json = serde_json::to_value(GuestToHost::TrapEvent(event.clone()))
        .map_err(|e| VectorError::Encode(e.to_string()))?;
    let mut case = trap_case("plugin-builder-fs-write-trap", event, Verdict::Pass)?;
    case.plugin_builder = Some(UnifiedBuilderCase {
        name: "plugin-builder-fs-write-trap".to_string(),
        requires: vec!["ledger".to_string()],
        rings: vec![(TrampolineRing::FsWrite, 0), (TrampolineRing::Trap, 1)],
        events: vec![
            serde_json::json!({
                "kind": "fs-write",
                "path": "/workspace/ledger/wal",
                "pid": 1300,
            }),
            event_json,
        ],
        expected_outcomes: vec![
            UnifiedBuilderOutcome::Filter(FilterAction::Forward),
            UnifiedBuilderOutcome::Verdict(Verdict::Pass),
        ],
    });
    Ok(case)
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
fn plugin_builder_case(
    name: &str,
    seq: u64,
    requires: Vec<String>,
    rings: Vec<(TrampolineRing, u32)>,
    events: Vec<serde_json::Value>,
    expected_outcomes: Vec<UnifiedBuilderOutcome>,
) -> Result<VectorCase, VectorError> {
    let event = trap_event(
        seq,
        1234,
        100,
        "/usr/bin/git",
        vec!["git".to_string(), "commit".to_string()],
        HashMap::new(),
    );
    let mut case = trap_case(name, event, Verdict::Pass)?;
    case.plugin_builder = Some(UnifiedBuilderCase {
        name: name.to_string(),
        requires,
        rings,
        events,
        expected_outcomes,
    });
    Ok(case)
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
fn plugin_builder_trap_json(seq: u64) -> Result<serde_json::Value, VectorError> {
    let event = trap_event(
        seq,
        1234,
        100,
        "/usr/bin/git",
        vec!["git".to_string(), "commit".to_string()],
        HashMap::new(),
    );
    serde_json::to_value(GuestToHost::TrapEvent(event))
        .map_err(|e| VectorError::Encode(e.to_string()))
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
fn plugin_builder_verb_event(verb: &str, pid: u32) -> serde_json::Value {
    match verb {
        "process-exit" => serde_json::json!({"kind": verb, "pid": pid, "exit_code": 0}),
        "fs-lookup" | "fs-read" | "fs-write" | "fs-lock" => {
            serde_json::json!({"kind": verb, "path": "/workspace/ledger/wal", "pid": pid})
        }
        "net-outbound" | "net-inbound" => serde_json::json!({"kind": verb, "stream_id": 0}),
        "console-input" | "console-output" => serde_json::json!({"kind": verb, "port": 1}),
        "exec-stdout" | "exec-stderr" => {
            serde_json::json!({"kind": verb, "pid": pid, "exec_id": 7})
        }
        "host-lsm-exec" => {
            serde_json::json!({"kind": verb, "path": "/usr/bin/git", "pid": pid})
        }
        "host-pressure" => serde_json::json!({"kind": verb, "level": 2}),
        "host-keyring-access" => serde_json::json!({"kind": verb, "key": "api-token"}),
        "vm-init" | "vm-ready" | "vm-shutdown" | "pre-commit" | "post-commit" | "host-preempt" => {
            serde_json::json!({"kind": verb})
        }
        "memory-pressure" => serde_json::json!({"kind": verb, "used": 512, "total": 1024}),
        "heartbeat" => serde_json::json!({"kind": verb, "uptime_ms": 42000}),
        "watchdog" => serde_json::json!({"kind": verb, "timeout_ms": 5000, "action": "Reboot"}),
        _ => serde_json::json!({"kind": verb}),
    }
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
fn plugin_builder_verb_cases() -> Result<Vec<VectorCase>, VectorError> {
    let verbs: &[(&str, TrampolineRing, Option<UnifiedBuilderOutcome>)] = &[
        ("process-exit", TrampolineRing::ProcessExit, None),
        (
            "fs-lookup",
            TrampolineRing::FsLookup,
            Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
        ),
        ("fs-read", TrampolineRing::FsRead, None),
        (
            "fs-lock",
            TrampolineRing::FsLock,
            Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
        ),
        (
            "net-outbound",
            TrampolineRing::NetOutbound,
            Some(UnifiedBuilderOutcome::Filter(FilterAction::Forward)),
        ),
        (
            "net-inbound",
            TrampolineRing::NetInbound,
            Some(UnifiedBuilderOutcome::Filter(FilterAction::Forward)),
        ),
        ("console-input", TrampolineRing::ConsoleInput, None),
        ("console-output", TrampolineRing::ConsoleOutput, None),
        ("exec-stdout", TrampolineRing::ExecStdout, None),
        ("exec-stderr", TrampolineRing::ExecStderr, None),
        (
            "host-lsm-exec",
            TrampolineRing::HostLsmExec,
            Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
        ),
        (
            "host-pressure",
            TrampolineRing::HostPressure,
            Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
        ),
        (
            "host-keyring-access",
            TrampolineRing::HostKeyringAccess,
            Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
        ),
        ("vm-init", TrampolineRing::Lifecycle, None),
        ("vm-ready", TrampolineRing::Lifecycle, None),
        ("vm-shutdown", TrampolineRing::Lifecycle, None),
        ("pre-commit", TrampolineRing::Lifecycle, None),
        ("post-commit", TrampolineRing::Lifecycle, None),
        ("host-preempt", TrampolineRing::HostPreempt, None),
        ("memory-pressure", TrampolineRing::MemoryPressure, None),
        ("heartbeat", TrampolineRing::Heartbeat, None),
        ("watchdog", TrampolineRing::Watchdog, None),
    ];
    let mut cases = Vec::with_capacity(verbs.len() + 6);
    let mut seq = 9;
    for (verb, ring, outcome) in verbs {
        let outcomes = match outcome {
            Some(outcome) => vec![outcome.clone()],
            None => Vec::new(),
        };
        cases.push(plugin_builder_case(
            &format!("plugin-builder-{verb}-only"),
            seq,
            Vec::new(),
            vec![(*ring, 0)],
            vec![plugin_builder_verb_event(verb, 1300)],
            outcomes,
        )?);
        seq += 1;
    }
    type StructuralVerb = (&'static str, Vec<String>, Vec<(TrampolineRing, u32)>);
    let structural: &[StructuralVerb] = &[
        ("context", Vec::new(), Vec::new()),
        ("require", vec!["ledger".to_string()], Vec::new()),
        ("scope", Vec::new(), Vec::new()),
        ("fork-with", Vec::new(), Vec::new()),
        (
            "vsock-service",
            Vec::new(),
            vec![(TrampolineRing::VsockService, 0)],
        ),
        ("build", Vec::new(), Vec::new()),
    ];
    for (verb, requires, rings) in structural {
        cases.push(plugin_builder_case(
            &format!("plugin-builder-{verb}-only"),
            seq,
            requires.clone(),
            rings.clone(),
            Vec::new(),
            Vec::new(),
        )?);
        seq += 1;
    }
    Ok(cases)
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/03-sdk-architecture.md#Unified-builder-surface
fn plugin_builder_scope_cases(
    prefix: &str,
    seq_start: u64,
) -> Result<Vec<VectorCase>, VectorError> {
    type ScopeVerb = (&'static str, TrampolineRing, Option<UnifiedBuilderOutcome>);
    let scopes: &[(&str, &[ScopeVerb])] = &[
        (
            "trap",
            &[
                (
                    "trap",
                    TrampolineRing::Trap,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
                ("process-exit", TrampolineRing::ProcessExit, None),
            ],
        ),
        (
            "fs",
            &[
                (
                    "fs-lookup",
                    TrampolineRing::FsLookup,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
                ("fs-read", TrampolineRing::FsRead, None),
                (
                    "fs-write",
                    TrampolineRing::FsWrite,
                    Some(UnifiedBuilderOutcome::Filter(FilterAction::Forward)),
                ),
                (
                    "fs-lock",
                    TrampolineRing::FsLock,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
            ],
        ),
        (
            "net",
            &[
                (
                    "net-outbound",
                    TrampolineRing::NetOutbound,
                    Some(UnifiedBuilderOutcome::Filter(FilterAction::Forward)),
                ),
                (
                    "net-inbound",
                    TrampolineRing::NetInbound,
                    Some(UnifiedBuilderOutcome::Filter(FilterAction::Forward)),
                ),
            ],
        ),
        (
            "console",
            &[
                ("console-input", TrampolineRing::ConsoleInput, None),
                ("console-output", TrampolineRing::ConsoleOutput, None),
            ],
        ),
        (
            "exec",
            &[
                ("exec-stdout", TrampolineRing::ExecStdout, None),
                ("exec-stderr", TrampolineRing::ExecStderr, None),
            ],
        ),
        (
            "host",
            &[
                (
                    "host-lsm-exec",
                    TrampolineRing::HostLsmExec,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
                (
                    "host-pressure",
                    TrampolineRing::HostPressure,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
                (
                    "host-keyring-access",
                    TrampolineRing::HostKeyringAccess,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
                ("host-preempt", TrampolineRing::HostPreempt, None),
            ],
        ),
        (
            "lifecycle",
            &[
                ("vm-init", TrampolineRing::Lifecycle, None),
                ("vm-ready", TrampolineRing::Lifecycle, None),
                ("vm-shutdown", TrampolineRing::Lifecycle, None),
                ("pre-commit", TrampolineRing::Lifecycle, None),
                ("post-commit", TrampolineRing::Lifecycle, None),
            ],
        ),
        (
            "telemetry",
            &[
                ("memory-pressure", TrampolineRing::MemoryPressure, None),
                ("heartbeat", TrampolineRing::Heartbeat, None),
                ("watchdog", TrampolineRing::Watchdog, None),
            ],
        ),
    ];
    let mut cases = Vec::with_capacity(scopes.len());
    for (seq, (scope, verbs)) in (seq_start..).zip(scopes.iter()) {
        let mut rings = Vec::with_capacity(verbs.len());
        let mut events = Vec::with_capacity(verbs.len());
        let mut outcomes = Vec::new();
        for (ordinal, (verb, ring, outcome)) in verbs.iter().enumerate() {
            rings.push((*ring, ordinal as u32));
            if *verb == "trap" {
                events.push(plugin_builder_trap_json(seq)?);
            } else {
                events.push(plugin_builder_verb_event(verb, 1300));
            }
            if let Some(outcome) = outcome {
                outcomes.push(outcome.clone());
            }
        }
        cases.push(plugin_builder_case(
            &format!("{prefix}-{scope}"),
            seq,
            Vec::new(),
            rings,
            events,
            outcomes,
        )?);
    }
    Ok(cases)
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
fn plugin_builder_nested_cases() -> Result<Vec<VectorCase>, VectorError> {
    plugin_builder_scope_cases("plugin-builder-nested", 37)
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
fn plugin_builder_merge_cases() -> Result<Vec<VectorCase>, VectorError> {
    plugin_builder_scope_cases("plugin-merge", 45)
}

// see: cans/artifacts/examples/policy-matrix.rs#WatchdogSilencerPlugin
// see: cans/artifacts/examples/policy-matrix.rs#PolicyMatrixPlugin
fn watchdog_case(
    name: &str,
    event: WatchdogEvent,
    expected_verdict: WatchdogVerdict,
) -> Result<VectorCase, VectorError> {
    let heartbeat = GuestToHost::Heartbeat(Heartbeat {
        uptime_ms: event.last_heartbeat_ms,
    });
    Ok(VectorCase {
        name: name.to_string(),
        frame: WireFrame {
            transport: Channel::Control,
            stream_id: 0,
            flags: FLAG_DATA,
            msg_type: MsgType::SystemTelemetry,
            payload: cbor(&event)?,
        },
        event: Some(heartbeat),
        expected_verdict: Verdict::Pass,
        watchdog: Some(WatchdogCase {
            event,
            expected_verdict,
        }),
        veto: None,
        plugin_builder: None,
    })
}

// see: cans/08-lifecycle.md#Lifecycle-vetoes
// see: cans/artifacts/examples/policy-matrix.rs#PolicyMatrixPlugin
fn veto_case(
    name: &str,
    event: VetoEvent,
    callback_allows: bool,
) -> Result<VectorCase, VectorError> {
    Ok(VectorCase {
        name: name.to_string(),
        frame: WireFrame {
            transport: Channel::Control,
            stream_id: 0,
            flags: FLAG_DATA,
            msg_type: MsgType::VmLifecycleControl,
            payload: cbor(&event)?,
        },
        event: None,
        expected_verdict: Verdict::Pass,
        watchdog: None,
        veto: Some(VetoCase {
            event,
            callback_allows,
        }),
        plugin_builder: None,
    })
}

fn exec_rewrite_case() -> Result<VectorCase, VectorError> {
    let mutation = ExecveMutation {
        new_binary: Some("/usr/local/bin/inspected-curl".to_string()),
        new_argv: Some(vec!["inspected-curl".to_string(), "-fsS".to_string()]),
        set_env: vec![("KELIR_TAINT".to_string(), "1".to_string())],
        unset_env: vec!["KELIR_UNTRUSTED".to_string()],
    };
    let event = GuestToHost::TrapEvent(trap_event(
        3,
        1300,
        100,
        "/bin/sh",
        vec![
            "sh".to_string(),
            "-c".to_string(),
            "curl example.test/install.sh | sh".to_string(),
        ],
        HashMap::new(),
    ));
    Ok(VectorCase {
        name: "trap-pipe-install-rewrite-execve-mutation".to_string(),
        frame: WireFrame {
            transport: Channel::Control,
            stream_id: 0,
            flags: FLAG_DATA,
            msg_type: MsgType::Trap,
            payload: cbor(&event)?,
        },
        event: Some(event),
        expected_verdict: Verdict::Patch(PatchPayload::ExecveMutation(mutation)),
        watchdog: None,
        veto: None,
        plugin_builder: None,
    })
}

fn heartbeat_case() -> Result<VectorCase, VectorError> {
    let event = GuestToHost::Heartbeat(Heartbeat { uptime_ms: 42_000 });
    Ok(VectorCase {
        name: "heartbeat-telemetry-observer-pass".to_string(),
        frame: WireFrame {
            transport: Channel::Control,
            stream_id: 0,
            flags: FLAG_DATA,
            msg_type: MsgType::SystemTelemetry,
            payload: cbor(&event)?,
        },
        event: Some(event),
        expected_verdict: Verdict::Pass,
        watchdog: None,
        veto: None,
        plugin_builder: None,
    })
}

// see: cans/15-operations.md#Local-velocity
fn poll_sync<F: Future>(future: F) -> Result<F::Output, VectorError> {
    let waker = Waker::noop();
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => Ok(output),
        Poll::Pending => Err(VectorError::Execution(
            "fixture future suspended; the L2 fixture driver must complete in one poll".to_string(),
        )),
    }
}

// see: cans/08-lifecycle.md#Watchdog
// see: cans/15-operations.md#Multi-language-parity-gate
pub fn arm_watchdog_case(event: &WatchdogEvent) -> Result<(), VectorError> {
    let mut fixture = LifecycleCtxFixture::new();
    let timeout = Duration::from_millis(event.timeout_ms);
    let action = event.action.clone();
    poll_sync(fixture.ctx().set_watchdog(timeout, action))
        .and_then(|result| result.map_err(|e| VectorError::Execution(e.to_string())))?;
    match fixture.armed_watchdog() {
        Some((armed_timeout, armed_action))
            if armed_timeout == timeout && armed_action == event.action =>
        {
            Ok(())
        }
        other => Err(VectorError::Execution(format!(
            "armed watchdog {other:?} does not match the vector timeout {timeout:?} and action {:?}",
            event.action
        ))),
    }
}

// see: cans/08-lifecycle.md#Lifecycle-vetoes
// see: cans/15-operations.md#Multi-language-parity-gate
pub fn register_veto_case(veto: &VetoCase) -> Result<(), VectorError> {
    let mut fixture = LifecycleCtxFixture::new();
    let callback_allows = veto.callback_allows;
    fixture
        .ctx()
        .register_veto(veto.event, Box::new(move || callback_allows));
    let registered = fixture
        .vetoes()
        .iter()
        .any(|(event, callback)| *event == veto.event && callback() == veto.callback_allows);
    if registered {
        Ok(())
    } else {
        Err(VectorError::Execution(format!(
            "veto registration {:?} did not survive LifecycleCtx::register_veto",
            veto.event
        )))
    }
}
