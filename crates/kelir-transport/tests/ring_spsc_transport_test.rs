// see: cans/artifacts/test-plan/cases/virtio_rings/vr_01_spsc_ring_lockfree_transit.yaml#case_id
use std::io::ErrorKind;
use std::io::Read;

use kelir_interface::codec_protocol::{FLAG_DATA, WireFrame};
use kelir_interface::protocol_wire::{Channel, MsgType};
use kelir_transport::Transport;
use kelir_transport::fallback_transport::VsockFallbackTransport;
use kelir_transport::spsc_ring::OwnedSpscRing;

// see: cans/artifacts/test-plan/cases/virtio_rings/vr_01_spsc_ring_lockfree_transit.yaml#case_id
#[test]
fn vr_01_spsc_ring_lockfree_transit() -> Result<(), String> {
    let mut ring = OwnedSpscRing::new(4096);
    let mut transport = ring.transport();
    assert_eq!(transport.used(), 0);
    assert_eq!(transport.free(), 4096);
    let written = transport.write(b"frame-1").map_err(|e| e.to_string())?;
    assert_eq!(written, 7);
    assert_eq!(transport.used(), 7);
    assert_eq!(transport.free(), 4089);
    let mut buf = [0u8; 7];
    let read = transport.read(&mut buf).map_err(|e| e.to_string())?;
    assert_eq!(read, 7);
    assert_eq!(buf, *b"frame-1");
    assert_eq!(transport.used(), 0);
    let mut empty = [0u8; 4];
    let read_empty = transport.read(&mut empty).map_err(|e| e.to_string())?;
    assert_eq!(read_empty, 0);
    let full = [0xabu8; 4096];
    let written_full = transport.write(&full).map_err(|e| e.to_string())?;
    assert_eq!(written_full, 4096);
    assert_eq!(transport.used(), 4096);
    assert_eq!(transport.free(), 0);
    match transport.write(b"x") {
        Err(e) if e.kind() == ErrorKind::WouldBlock => {}
        Err(e) => return Err(format!("full ring write gave wrong error kind: {e}")),
        Ok(_) => return Err("full ring write succeeded".to_string()),
    }
    let mut drain = [0u8; 4096];
    let drained = transport.read(&mut drain).map_err(|e| e.to_string())?;
    assert_eq!(drained, 4096);
    assert_eq!(drain, full);
    assert_eq!(transport.used(), 0);
    let head_block = [0x11u8; 4000];
    let written_head = transport.write(&head_block).map_err(|e| e.to_string())?;
    assert_eq!(written_head, 4000);
    let mut head_read = [0u8; 4000];
    let read_head = transport.read(&mut head_read).map_err(|e| e.to_string())?;
    assert_eq!(read_head, 4000);
    assert_eq!(head_read, head_block);
    let tail_block = [0x22u8; 200];
    let written_tail = transport.write(&tail_block).map_err(|e| e.to_string())?;
    assert_eq!(written_tail, 200);
    let mut tail_read = [0u8; 200];
    let read_tail = transport.read(&mut tail_read).map_err(|e| e.to_string())?;
    assert_eq!(read_tail, 200);
    assert_eq!(tail_read, tail_block);
    assert_eq!(transport.used(), 0);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/virtio_rings/vr_15_owned_spsc_ring_capacity_accounting.yaml#case_id
#[test]
fn vr_15_owned_spsc_ring_capacity_accounting() -> Result<(), String> {
    let mut ring = OwnedSpscRing::new(4096);
    assert_eq!(ring.capacity(), 4096);
    assert_eq!(ring.transport().used(), 0);
    assert_eq!(ring.transport().free(), 4096);
    assert!(!ring.is_shared_backed());
    assert_eq!(ring.guest_phys_addr(), None);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/virtio_rings/vr_12_transport_af_vsock_fallback_parity.yaml#case_id
#[test]
#[ignore]
fn vr_12_transport_af_vsock_fallback_parity() -> Result<(), String> {
    let frame = WireFrame {
        transport: Channel::StreamMux,
        stream_id: 1,
        flags: FLAG_DATA,
        msg_type: MsgType::ChannelPayload,
        payload: b"hello_vsock_parity".to_vec(),
    };
    let encoded = frame.encode().map_err(|e| e.to_string())?;
    let mut ring = OwnedSpscRing::new(65536);
    let mut ring_writer = ring.transport();
    let written_ring = ring_writer.write(&encoded).map_err(|e| e.to_string())?;
    assert_eq!(written_ring, encoded.len());
    let mut ring_read = vec![0u8; encoded.len()];
    let mut ring_reader = ring.transport();
    let read_ring = ring_reader
        .read(&mut ring_read)
        .map_err(|e| e.to_string())?;
    assert_eq!(read_ring, encoded.len());
    assert_eq!(ring_read, encoded);
    let (host_end, mut peer_end) =
        std::os::unix::net::UnixStream::pair().map_err(|e| e.to_string())?;
    let fd = std::os::unix::io::AsRawFd::as_raw_fd(&host_end);
    let mut fallback = VsockFallbackTransport::new(fd);
    match fallback.write(&encoded) {
        Ok(written) => {
            assert_eq!(written, encoded.len());
            let mut fallback_read = vec![0u8; encoded.len()];
            let read_fallback = peer_end
                .read(&mut fallback_read)
                .map_err(|e| e.to_string())?;
            assert_eq!(read_fallback, encoded.len());
            assert_eq!(fallback_read, encoded);
        }
        Err(e) => {
            return Err(format!(
                "vsock fallback transport is not a byte-for-byte framing carrier: {e}"
            ));
        }
    }
    Ok(())
}
