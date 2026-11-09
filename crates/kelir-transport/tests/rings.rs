// see: cans/artifacts/test-plan/cases/virtio_rings/vr_01_spsc_ring_lockfree_transit.yaml#case_id
use std::io::ErrorKind;

use kelir_transport::Transport;
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
