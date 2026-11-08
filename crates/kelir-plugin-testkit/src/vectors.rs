// see: cans/02-plugin-model.md#Plugin-testkit
// see: cans/15-operations.md#Multi-language-parity-gate
// see: cans/artifacts/code/wire-protocol.rs#MuxFrameHeader
// see: cans/artifacts/code/messages.rs#GuestToHost
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use kelir_interface::codec::{FLAG_DATA, MAX_FRAME_PAYLOAD, MAX_STREAM_ID};
use kelir_interface::driver::{WatchdogAction, WatchdogEvent, WatchdogVerdict};
use kelir_interface::plugin::VetoEvent;
use kelir_interface::protocol::{
    Channel, ExecveMutation, GuestToHost, Heartbeat, MsgType, PatchPayload, SuspendUntil,
    TrapEvent, Verdict,
};

// see: cans/11-protocol.md#Wire-framing
#[derive(Debug)]
pub enum VectorError {
    FrameTooLarge { len: usize },
    StreamIdOutOfRange { stream_id: u32 },
    Encode(String),
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
        }
    }
}

impl Error for VectorError {}

// see: cans/11-protocol.md#Wire-framing
#[derive(Debug, Clone)]
pub struct WireFrame {
    pub transport: Channel,
    pub stream_id: u32,
    pub flags: u8,
    pub msg_type: MsgType,
    pub payload: Vec<u8>,
}

impl WireFrame {
    // see: cans/artifacts/code/wire-protocol.rs#MuxFrameHeader
    pub fn encode(&self) -> Result<Vec<u8>, VectorError> {
        if self.stream_id > MAX_STREAM_ID {
            return Err(VectorError::StreamIdOutOfRange {
                stream_id: self.stream_id,
            });
        }
        let body_len = self
            .payload
            .len()
            .checked_add(2)
            .ok_or(VectorError::FrameTooLarge { len: usize::MAX })?;
        if body_len > MAX_FRAME_PAYLOAD {
            return Err(VectorError::FrameTooLarge { len: body_len });
        }
        let len =
            u32::try_from(body_len).map_err(|_| VectorError::FrameTooLarge { len: body_len })?;
        let mut frame = Vec::with_capacity(
            body_len
                .checked_add(9)
                .ok_or(VectorError::FrameTooLarge { len: usize::MAX })?,
        );
        frame.extend_from_slice(&self.stream_id.to_le_bytes());
        frame.push(self.flags);
        frame.extend_from_slice(&len.to_le_bytes());
        frame.extend_from_slice(&(self.msg_type as u16).to_le_bytes());
        frame.extend_from_slice(&self.payload);
        Ok(frame)
    }
}

// see: cans/15-operations.md#Multi-language-parity-gate
#[derive(Debug, Clone)]
pub struct WatchdogCase {
    pub event: WatchdogEvent,
    pub expected_verdict: WatchdogVerdict,
}

// see: cans/15-operations.md#Multi-language-parity-gate
#[derive(Debug, Clone)]
pub struct VetoCase {
    pub event: VetoEvent,
}

// see: cans/15-operations.md#Multi-language-parity-gate
#[derive(Debug)]
pub struct VectorCase {
    pub name: String,
    pub frame: WireFrame,
    pub event: GuestToHost,
    pub expected_verdict: Verdict,
    pub watchdog: Option<WatchdogCase>,
    pub veto: Option<VetoCase>,
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
                "payload_cbor_hex": hex(&self.frame.payload),
                "frame_hex": hex(&frame),
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
            });
            if let Some(object) = value.as_object_mut() {
                object.insert("veto".to_string(), entry);
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
            vectors: vec![
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
                        last_active_pid: Some(1234),
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
                        last_active_pid: None,
                    },
                    WatchdogVerdict::Mitigate,
                )?,
                veto_case("veto-fork", VetoEvent::Fork)?,
                veto_case("veto-suspend", VetoEvent::Suspend)?,
                veto_case("veto-clock-skew", VetoEvent::ClockSkew)?,
            ],
        })
    }
}

fn cbor<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, VectorError> {
    let mut out = Vec::new();
    ciborium::ser::into_writer(value, &mut out).map_err(|e| VectorError::Encode(e.to_string()))?;
    Ok(out)
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn trap_event(
    seq: u64,
    pid: u32,
    ppid: u32,
    binary_path: &str,
    argv: Vec<String>,
    envp: HashMap<String, String>,
) -> TrapEvent {
    TrapEvent {
        seq,
        pid,
        ppid,
        task_id: seq,
        binary_path: binary_path.to_string(),
        binary_dev: 2049,
        binary_ino: 400_000 + seq * 7,
        binary_blake3: "42".repeat(32),
        argv,
        envp,
        cwd: "/workspace".to_string(),
        syscall: None,
    }
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
        event,
        expected_verdict,
        watchdog: None,
        veto: None,
    })
}

fn watchdog_case(
    name: &str,
    event: WatchdogEvent,
    expected_verdict: WatchdogVerdict,
) -> Result<VectorCase, VectorError> {
    Ok(VectorCase {
        name: name.to_string(),
        frame: WireFrame {
            transport: Channel::Control,
            stream_id: 0,
            flags: FLAG_DATA,
            msg_type: MsgType::SystemTelemetry,
            payload: cbor(&event)?,
        },
        event: GuestToHost::Heartbeat(Heartbeat {
            uptime_ms: event.last_heartbeat_ms,
        }),
        expected_verdict: Verdict::Pass,
        watchdog: Some(WatchdogCase {
            event,
            expected_verdict,
        }),
        veto: None,
    })
}

fn veto_case(name: &str, event: VetoEvent) -> Result<VectorCase, VectorError> {
    Ok(VectorCase {
        name: name.to_string(),
        frame: WireFrame {
            transport: Channel::Control,
            stream_id: 0,
            flags: FLAG_DATA,
            msg_type: MsgType::VmLifecycleControl,
            payload: cbor(&event)?,
        },
        event: GuestToHost::Heartbeat(Heartbeat { uptime_ms: 0 }),
        expected_verdict: Verdict::Pass,
        watchdog: None,
        veto: Some(VetoCase { event }),
    })
}

fn exec_rewrite_case() -> Result<VectorCase, VectorError> {
    let mutation = ExecveMutation {
        new_binary: Some("/usr/local/bin/inspected-curl".to_string()),
        new_argv: Some(vec!["inspected-curl".to_string(), "-fsS".to_string()]),
        set_env: vec![("KELIR_TAINT".to_string(), "1".to_string())],
        unset_env: vec!["KELIR_UNTRUSTED".to_string()],
    };
    let mutation_cbor = cbor(&mutation)?;
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
        event,
        expected_verdict: Verdict::Patch(PatchPayload::ExecveMutation(mutation_cbor)),
        watchdog: None,
        veto: None,
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
        event,
        expected_verdict: Verdict::Pass,
        watchdog: None,
        veto: None,
    })
}
