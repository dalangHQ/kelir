// see: cans/artifacts/test-plan/cases/protocol_mux/pm_04_plugin_stream_kind_dynamic_registration.yaml#case_id
use std::sync::{Arc, Mutex};

use kelir_engine::bus::{BusDispatcher, OwnedBusDispatcher};
use kelir_interface::codec_protocol::{
    FLAG_CLOSE, FLAG_DATA, FLAG_OPEN, MAX_FRAME_PAYLOAD, decode_msg_type, encode_msg_frame,
};
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::{MsgType, StreamKind};
use kelir_transport::Transport;
use kelir_transport::spsc_ring::OwnedSpscRing;

type SinkCall = (u32, u16, u8, Vec<u8>);

struct RecordingSink {
    calls: Mutex<Vec<SinkCall>>,
}

impl RecordingSink {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }
}

impl kelir_engine::bus::StreamFrameSink for RecordingSink {
    fn on_stream_frame(
        &self,
        stream_id: u32,
        kind: u16,
        flags: u8,
        payload: &[u8],
    ) -> Result<(), PluginError> {
        let mut calls = self
            .calls
            .lock()
            .map_err(|_| PluginError::Halt("sink lock poisoned".into()))?;
        calls.push((stream_id, kind, flags, payload.to_vec()));
        Ok(())
    }
}

fn mux_frame(stream_id: u32, flags: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(9 + payload.len());
    frame.extend_from_slice(&stream_id.to_le_bytes());
    frame.push(flags);
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(payload);
    frame
}

// see: cans/artifacts/test-plan/cases/protocol_mux/pm_04_plugin_stream_kind_dynamic_registration.yaml#case_id
#[test]
fn pm_04_plugin_stream_kind_dynamic_registration() -> Result<(), String> {
    let mut control_ring = OwnedSpscRing::new(131072);
    let mut mux_ring = OwnedSpscRing::new(262144);
    let mut mux_writer = mux_ring.transport();
    let mut dispatcher = BusDispatcher::new(control_ring.transport(), mux_ring.transport());
    let sink = Arc::new(RecordingSink::new());
    dispatcher
        .bind_stream_sink(
            StreamKind::ProfileSync as u16,
            Arc::clone(&sink) as Arc<dyn kelir_engine::bus::StreamFrameSink>,
        )
        .map_err(|e| e.to_string())?;
    let open = mux_frame(7, FLAG_OPEN, &[0x10, 0x00]);
    let written = mux_writer.write(&open).map_err(|e| e.to_string())?;
    assert_eq!(written, open.len());
    let ingress = dispatcher.poll_ingress().map_err(|e| e.to_string())?;
    match ingress {
        Some((stream_id, flags, payload)) => {
            assert_eq!(stream_id, 7);
            assert_eq!(flags, FLAG_OPEN);
            assert_eq!(payload, vec![0x10u8, 0x00]);
        }
        None => return Err("OPEN frame never arrived".to_string()),
    }
    assert_eq!(
        dispatcher.active_stream_kind(7),
        Some(StreamKind::ProfileSync as u16)
    );
    dispatcher
        .route_stream_frame(7, FLAG_DATA, b"hello")
        .map_err(|e| e.to_string())?;
    let calls = sink
        .calls
        .lock()
        .map_err(|_| "sink lock poisoned".to_string())?;
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, 7);
    assert_eq!(calls[0].1, StreamKind::ProfileSync as u16);
    assert_eq!(calls[0].2, FLAG_DATA);
    assert_eq!(calls[0].3, b"hello");
    Ok(())
}

// see: cans/artifacts/test-plan/cases/protocol_mux/pm_01_control_channel_0_lifecycle_priority.yaml#case_id
#[test]
#[ignore]
fn pm_01_control_channel_0_lifecycle_priority() -> Result<(), String> {
    let mut control_ring = OwnedSpscRing::new(131072);
    let mut control_writer = control_ring.transport();
    let mut mux_ring = OwnedSpscRing::new(65536);
    let mut mux_writer = mux_ring.transport();
    let mut dispatcher = BusDispatcher::new(control_ring.transport(), mux_ring.transport());
    let mut backlog = vec![0u8; 65527];
    for (index, byte) in backlog.iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
    let saturated_mux = mux_frame(7, FLAG_DATA, &backlog);
    let written_mux = mux_writer
        .write(&saturated_mux)
        .map_err(|e| e.to_string())?;
    assert_eq!(written_mux, 65536);
    let mut signal_body = Vec::new();
    signal_body.extend_from_slice(&15u32.to_le_bytes());
    signal_body.extend_from_slice(&100u64.to_le_bytes());
    let mut signal_frame = Vec::new();
    encode_msg_frame(
        0,
        FLAG_DATA,
        MsgType::Signal,
        &signal_body,
        &mut signal_frame,
    )
    .map_err(|e| e.to_string())?;
    control_writer
        .write(&signal_frame)
        .map_err(|e| e.to_string())?;
    match dispatcher.poll_ingress().map_err(|e| e.to_string())? {
        Some((stream_id, flags, payload)) => {
            if stream_id != 0 {
                return Err(format!(
                    "channel-0 signal starved behind the saturated stream mux: first ingress delivered mux stream {stream_id} (flags {flags:#04x}) instead of the control signal"
                ));
            }
            let (msg_type, body) = decode_msg_type(&payload).map_err(|e| e.to_string())?;
            assert_eq!(msg_type, MsgType::Signal as u16);
            assert_eq!(flags, FLAG_DATA);
            assert_eq!(body, signal_body.as_slice());
        }
        None => {
            return Err(
                "ingress poll found no frame while both channels carry pending data".to_string(),
            );
        }
    }
    assert_eq!(mux_writer.used(), 65536);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/protocol_mux/pm_02_stream_mux_open_data_close_flags.yaml#case_id
#[test]
#[ignore]
fn pm_02_stream_mux_open_data_close_flags() -> Result<(), String> {
    let mut control_ring = OwnedSpscRing::new(4096);
    let mut mux_ring = OwnedSpscRing::new(4096);
    let mut mux_writer = mux_ring.transport();
    let mut dispatcher = BusDispatcher::new(control_ring.transport(), mux_ring.transport());
    let hostile = mux_frame(7, FLAG_OPEN | FLAG_CLOSE, &[0x10, 0x00]);
    mux_writer.write(&hostile).map_err(|e| e.to_string())?;
    match dispatcher.poll_ingress() {
        Err(PluginError::Halt(message)) => assert!(
            message.contains("EProtocol") || message.contains("flags"),
            "mutually exclusive flags rejected with the wrong diagnostic: {message}"
        ),
        Err(e) => {
            return Err(format!(
                "mutually exclusive flags gave the wrong error: {e}"
            ));
        }
        Ok(Some((stream_id, flags, _))) => {
            return Err(format!(
                "mutually exclusive OPEN|CLOSE frame (flags 0x05) was accepted and registered stream {stream_id} with flags {flags:#04x}"
            ));
        }
        Ok(None) => {
            return Err(
                "mutually exclusive OPEN|CLOSE frame dropped silently without a protocol error"
                    .to_string(),
            );
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/protocol_mux/pm_11_wire_framing_checked_math_validation.yaml#case_id
#[test]
fn pm_11_wire_framing_checked_math_validation() -> Result<(), String> {
    assert_eq!(MAX_FRAME_PAYLOAD, 65536);
    let mut bus = OwnedBusDispatcher::new(131072, 262144);
    assert_eq!(bus.control_capacity(), 131072);
    assert_eq!(bus.mux_capacity(), 262144);
    bus.dispatcher()
        .send_control_msg(MsgType::Signal, b"ping")
        .map_err(|e| e.to_string())?;
    let oversized = vec![0u8; 65535];
    match bus
        .dispatcher()
        .send_control_msg(MsgType::Signal, &oversized)
    {
        Err(PluginError::Halt(message)) => {
            assert!(message.contains("64KB frame budget"));
        }
        Err(e) => return Err(format!("oversized frame gave wrong error: {e}")),
        Ok(()) => return Err("oversized frame accepted".to_string()),
    }
    Ok(())
}

// see: cans/11-protocol.md#Kind-registration
#[test]
fn pm_04_plugin_kind_open_admitted_when_sink_registered() -> Result<(), String> {
    let mut control_ring = OwnedSpscRing::new(131072);
    let mut mux_ring = OwnedSpscRing::new(262144);
    let mut mux_writer = mux_ring.transport();
    let mut dispatcher = BusDispatcher::new(control_ring.transport(), mux_ring.transport());
    let sink = Arc::new(RecordingSink::new());
    dispatcher
        .bind_stream_sink(
            0x0019,
            Arc::clone(&sink) as Arc<dyn kelir_engine::bus::StreamFrameSink>,
        )
        .map_err(|e| e.to_string())?;
    let open = mux_frame(9, FLAG_OPEN, &[0x19, 0x00]);
    let written = mux_writer.write(&open).map_err(|e| e.to_string())?;
    assert_eq!(written, open.len());
    let ingress = dispatcher.poll_ingress().map_err(|e| e.to_string())?;
    match ingress {
        Some((stream_id, flags, payload)) => {
            assert_eq!(stream_id, 9);
            assert_eq!(flags, FLAG_OPEN);
            assert_eq!(payload, vec![0x19u8, 0x00]);
        }
        None => return Err("OPEN frame never arrived".to_string()),
    }
    assert_eq!(dispatcher.active_stream_kind(9), Some(0x0019));
    dispatcher
        .route_stream_frame(9, FLAG_DATA, b"hello")
        .map_err(|e| e.to_string())?;
    let calls = sink
        .calls
        .lock()
        .map_err(|_| "sink lock poisoned".to_string())?;
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, 9);
    assert_eq!(calls[0].1, 0x0019);
    assert_eq!(calls[0].2, FLAG_DATA);
    assert_eq!(calls[0].3, b"hello");
    Ok(())
}
