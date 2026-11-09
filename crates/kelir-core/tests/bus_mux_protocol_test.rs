// see: cans/artifacts/test-plan/cases/protocol_mux/pm_04_plugin_stream_kind_dynamic_registration.yaml#case_id
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kelir_core::bus::{BusDispatcher, FLAG_DATA, FLAG_OPEN, OwnedBusDispatcher};
use kelir_core::dispatch_plugin::PipelineEngine;
use kelir_interface::codec_protocol::MAX_FRAME_PAYLOAD;
use kelir_interface::driver_interface::Stream;
use kelir_interface::plugin_model::{HandlerCtx, PipelineBinder, PluginError};
use kelir_interface::protocol_wire::{MsgType, StreamKind};
use kelir_interface::vsock_mesh::{VsockPeer, VsockServiceHandler, VsockServiceSpec};
use kelir_transport::Transport;
use kelir_transport::spsc_ring::OwnedSpscRing;

type SinkCall = (u32, StreamKind, u8, Vec<u8>);

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

impl kelir_core::bus::StreamFrameSink for RecordingSink {
    fn on_stream_frame(
        &self,
        stream_id: u32,
        kind: StreamKind,
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

struct NoopVsockHandler;

#[async_trait]
impl VsockServiceHandler for NoopVsockHandler {
    async fn on_connect(
        &self,
        _peer: &VsockPeer,
        _stream: Box<dyn Stream>,
        _ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
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
            StreamKind::ProfileSync,
            Arc::clone(&sink) as Arc<dyn kelir_core::bus::StreamFrameSink>,
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
        Some(StreamKind::ProfileSync)
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
    assert_eq!(calls[0].1, StreamKind::ProfileSync);
    assert_eq!(calls[0].2, FLAG_DATA);
    assert_eq!(calls[0].3, b"hello");
    Ok(())
}

// see: cans/artifacts/test-plan/cases/protocol_mux/pm_05_stream_kind_conflict_ekindconflict.yaml#case_id
#[test]
fn pm_05_stream_kind_conflict_ekindconflict() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    engine.bind_vsock_service(
        VsockServiceSpec {
            port: 5252,
            service: "first".to_string(),
        },
        Arc::new(NoopVsockHandler),
    );
    engine.bind_vsock_service(
        VsockServiceSpec {
            port: 5252,
            service: "second".to_string(),
        },
        Arc::new(NoopVsockHandler),
    );
    match engine.seal() {
        Err(PluginError::Halt(message)) => {
            assert!(message.contains("vsock port already bound"));
            assert!(message.contains("5252"));
        }
        Err(e) => return Err(format!("duplicate bind gave wrong error: {e}")),
        Ok(()) => return Err("duplicate bind sealed successfully".to_string()),
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
