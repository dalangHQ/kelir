// see: cans/02-plugin-model.md#Mount-scoping
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use kelir_engine::dispatch_plugin::PipelineEngine;
use kelir_interface::plugin_model::{MountScope, NetInterposer, PipelineBinder, PluginError};
use kelir_interface::protocol_wire::FilterAction;

struct CountingNet {
    outbound: AtomicUsize,
    inbound: AtomicUsize,
}

impl CountingNet {
    fn new() -> Self {
        Self {
            outbound: AtomicUsize::new(0),
            inbound: AtomicUsize::new(0),
        }
    }
}

impl NetInterposer for CountingNet {
    fn on_net_outbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        self.outbound.fetch_add(1, Ordering::SeqCst);
        Ok(FilterAction::Forward)
    }

    fn on_net_inbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        self.inbound.fetch_add(1, Ordering::SeqCst);
        Ok(FilterAction::Forward)
    }
}

fn eth_ipv4_tcp(src: [u8; 4], dst: [u8; 4], src_port: u16, dst_port: u16) -> Vec<u8> {
    let mut frame = Vec::with_capacity(42);
    frame.extend_from_slice(&[0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
    frame.extend_from_slice(&[0x02, 0x00, 0x00, 0x00, 0x00, 0x02]);
    frame.extend_from_slice(&[0x08, 0x00]);
    frame.extend_from_slice(&[0x45, 0x00, 0x00, 0x1c, 0x00, 0x00, 0x00, 0x00, 0x40, 0x06]);
    frame.extend_from_slice(&[0x00, 0x00]);
    frame.extend_from_slice(&src);
    frame.extend_from_slice(&dst);
    frame.extend_from_slice(&src_port.to_be_bytes());
    frame.extend_from_slice(&dst_port.to_be_bytes());
    frame.extend_from_slice(&[
        0x50, 0x02, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    frame
}

// see: cans/02-plugin-model.md#Mount-scoping
#[test]
fn outbound_frame_inside_declared_cidr_and_port_scope_reaches_net_hooks() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let net = Arc::new(CountingNet::new());
    engine.bind_net_scoped(
        Arc::clone(&net) as Arc<dyn NetInterposer>,
        MountScope::default()
            .with_net_cidrs(["10.0.0.0/8"])
            .with_net_ports([443u16]),
    );
    let mut frame = eth_ipv4_tcp([10, 0, 0, 1], [10, 0, 0, 2], 1234, 443);
    let action = engine
        .dispatch_net_outbound(&mut frame)
        .map_err(|e| e.to_string())?;
    assert!(matches!(action, FilterAction::Forward));
    assert_eq!(
        net.outbound.load(Ordering::SeqCst),
        1,
        "in-scope outbound frame must reach the net hook"
    );
    Ok(())
}

// see: cans/02-plugin-model.md#Mount-scoping
#[test]
fn outbound_frame_outside_declared_cidr_bypasses_net_hooks() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let net = Arc::new(CountingNet::new());
    engine.bind_net_scoped(
        Arc::clone(&net) as Arc<dyn NetInterposer>,
        MountScope::default()
            .with_net_cidrs(["10.0.0.0/8"])
            .with_net_ports([443u16]),
    );
    let mut frame = eth_ipv4_tcp([192, 168, 1, 5], [192, 168, 50, 2], 1234, 443);
    let action = engine
        .dispatch_net_outbound(&mut frame)
        .map_err(|e| e.to_string())?;
    assert!(matches!(action, FilterAction::Forward));
    assert_eq!(
        net.outbound.load(Ordering::SeqCst),
        0,
        "out-of-CIDR outbound frame must bypass the net hook before FFI"
    );
    Ok(())
}

// see: cans/02-plugin-model.md#Mount-scoping
#[test]
fn outbound_frame_outside_declared_port_bypasses_net_hooks() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let net = Arc::new(CountingNet::new());
    engine.bind_net_scoped(
        Arc::clone(&net) as Arc<dyn NetInterposer>,
        MountScope::default()
            .with_net_cidrs(["10.0.0.0/8"])
            .with_net_ports([443u16]),
    );
    let mut frame = eth_ipv4_tcp([10, 0, 0, 1], [10, 0, 0, 2], 1234, 8080);
    let action = engine
        .dispatch_net_outbound(&mut frame)
        .map_err(|e| e.to_string())?;
    assert!(matches!(action, FilterAction::Forward));
    assert_eq!(
        net.outbound.load(Ordering::SeqCst),
        0,
        "out-of-port outbound frame must bypass the net hook before FFI"
    );
    Ok(())
}

// see: cans/02-plugin-model.md#Mount-scoping
#[test]
fn inbound_frame_inside_declared_cidr_scope_reaches_net_hooks() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let net = Arc::new(CountingNet::new());
    engine.bind_net_scoped(
        Arc::clone(&net) as Arc<dyn NetInterposer>,
        MountScope::default().with_net_cidrs(["10.0.0.0/8"]),
    );
    let mut frame = eth_ipv4_tcp([10, 0, 0, 9], [10, 0, 0, 1], 5353, 1234);
    let action = engine
        .dispatch_net_inbound(&mut frame)
        .map_err(|e| e.to_string())?;
    assert!(matches!(action, FilterAction::Forward));
    assert_eq!(
        net.inbound.load(Ordering::SeqCst),
        1,
        "in-scope inbound frame must reach the net hook"
    );
    Ok(())
}

// see: cans/02-plugin-model.md#Mount-scoping
#[test]
fn inbound_frame_outside_declared_cidr_bypasses_net_hooks() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let net = Arc::new(CountingNet::new());
    engine.bind_net_scoped(
        Arc::clone(&net) as Arc<dyn NetInterposer>,
        MountScope::default().with_net_cidrs(["10.0.0.0/8"]),
    );
    let mut frame = eth_ipv4_tcp([172, 16, 0, 9], [10, 0, 0, 1], 5353, 1234);
    let action = engine
        .dispatch_net_inbound(&mut frame)
        .map_err(|e| e.to_string())?;
    assert!(matches!(action, FilterAction::Forward));
    assert_eq!(
        net.inbound.load(Ordering::SeqCst),
        0,
        "out-of-CIDR inbound frame must bypass the net hook before FFI"
    );
    Ok(())
}

// see: cans/02-plugin-model.md#Mount-scoping
#[test]
fn undeclared_net_scope_still_sees_unparseable_frames() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let net = Arc::new(CountingNet::new());
    engine.bind_net_scoped(
        Arc::clone(&net) as Arc<dyn NetInterposer>,
        MountScope::default(),
    );
    let mut frame = vec![0x01u8, 0x02, 0x03];
    let action = engine
        .dispatch_net_outbound(&mut frame)
        .map_err(|e| e.to_string())?;
    assert!(matches!(action, FilterAction::Forward));
    assert_eq!(
        net.outbound.load(Ordering::SeqCst),
        1,
        "scopes without declared net rules must keep every frame flowing to the hook"
    );
    Ok(())
}

// see: cans/02-plugin-model.md#Mount-scoping
#[test]
fn declared_net_scope_bypasses_non_ip_frames() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let net = Arc::new(CountingNet::new());
    engine.bind_net_scoped(
        Arc::clone(&net) as Arc<dyn NetInterposer>,
        MountScope::default().with_net_cidrs(["0.0.0.0/0"]),
    );
    let mut arp = Vec::new();
    arp.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    arp.extend_from_slice(&[0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
    arp.extend_from_slice(&[0x08, 0x06]);
    arp.extend_from_slice(&[0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x01]);
    let action = engine
        .dispatch_net_outbound(&mut arp)
        .map_err(|e| e.to_string())?;
    assert!(matches!(action, FilterAction::Forward));
    assert_eq!(
        net.outbound.load(Ordering::SeqCst),
        0,
        "non-IP frame cannot match a declared CIDR rule and must bypass the net hook"
    );
    Ok(())
}

// see: cans/02-plugin-model.md#Mount-scoping
#[test]
fn mixed_scopes_route_frames_per_interceptor() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let tenant = Arc::new(CountingNet::new());
    let corporate = Arc::new(CountingNet::new());
    engine.bind_net_scoped(
        Arc::clone(&tenant) as Arc<dyn NetInterposer>,
        MountScope::default().with_net_cidrs(["10.0.0.0/8"]),
    );
    engine.bind_net_scoped(
        Arc::clone(&corporate) as Arc<dyn NetInterposer>,
        MountScope::default().with_net_cidrs(["192.168.0.0/16"]),
    );
    let mut frame = eth_ipv4_tcp([10, 0, 0, 1], [10, 0, 0, 2], 1234, 443);
    let action = engine
        .dispatch_net_outbound(&mut frame)
        .map_err(|e| e.to_string())?;
    assert!(matches!(action, FilterAction::Forward));
    assert_eq!(
        tenant.outbound.load(Ordering::SeqCst),
        1,
        "tenant-scoped hook must see the 10/8 frame"
    );
    assert_eq!(
        corporate.outbound.load(Ordering::SeqCst),
        0,
        "corporate-scoped hook must bypass the 10/8 frame"
    );
    Ok(())
}
