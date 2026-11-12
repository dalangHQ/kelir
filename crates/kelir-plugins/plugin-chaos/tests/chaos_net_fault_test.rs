// see: cans/artifacts/test-plan/cases/plugin_engine/pe_16_chaos_rst_injection_drops_matching_frame.yaml#case_id
use kelir_interface::plugin_model::NetInterposer;
use kelir_interface::protocol_wire::FilterAction;
use plugin_chaos::{ChaosConfig, ChaosPlugin, FaultDirection, NetworkFaultRule};

fn tcp_frame(dst_ip: [u8; 4], dst_port: u16) -> Vec<u8> {
    let total_len = u16::try_from(20 + 20 + 8).unwrap_or(0);
    let mut frame = Vec::new();
    frame.extend_from_slice(&[0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
    frame.extend_from_slice(&[0x02, 0x00, 0x00, 0x00, 0x00, 0x02]);
    frame.extend_from_slice(&0x0800u16.to_be_bytes());
    frame.push(0x45);
    frame.push(0);
    frame.extend_from_slice(&total_len.to_be_bytes());
    frame.extend_from_slice(&[0u8; 5]);
    frame.push(6);
    frame.extend_from_slice(&[0u8; 2]);
    frame.extend_from_slice(&[10, 0, 0, 1]);
    frame.extend_from_slice(&dst_ip);
    frame.extend_from_slice(&40000u16.to_be_bytes());
    frame.extend_from_slice(&dst_port.to_be_bytes());
    frame.extend_from_slice(&1000u32.to_be_bytes());
    frame.extend_from_slice(&[0u8; 4]);
    frame.push(0x50);
    frame.push(0x18);
    frame.extend_from_slice(&[0u8; 6]);
    frame.extend_from_slice(b"payload");
    frame
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_16_chaos_rst_injection_drops_matching_frame.yaml#case_id
#[test]
#[ignore]
fn pe_16_chaos_rst_injection_drops_matching_frame() -> Result<(), String> {
    let rule = NetworkFaultRule {
        target: "10.0.0.2".into(),
        direction: FaultDirection::Outbound,
        base_ms: 0,
        jitter_ms: 0,
        drop_rate: 0.0,
        inject_rst: true,
        bandwidth_kbps: None,
    };
    let plugin = ChaosPlugin::new(ChaosConfig {
        network: vec![rule],
        fs: Vec::new(),
    });
    let mut matching = tcp_frame([10, 0, 0, 2], 443);
    match plugin.on_net_outbound(&mut matching) {
        Ok(FilterAction::Drop { errno, reason }) => {
            assert_eq!(
                errno.map(|errno| errno.as_str()),
                Some("ECONNREFUSED"),
                "RST injection must carry ECONNREFUSED, reason: {reason}"
            );
            assert!(
                reason.contains("10.0.0.2"),
                "drop reason must name the rule target, got: {reason}"
            );
        }
        Ok(other) => {
            return Err(format!(
                "matching frame must Drop with ECONNREFUSED, got {other:?}"
            ));
        }
        Err(e) => return Err(format!("chaos net dispatch failed: {e:?}")),
    }
    let mut other = tcp_frame([10, 0, 0, 3], 443);
    match plugin.on_net_outbound(&mut other) {
        Ok(FilterAction::Forward) => {}
        Ok(other_action) => {
            return Err(format!(
                "non-matching frame must Forward untouched, got {other_action:?}"
            ));
        }
        Err(e) => return Err(format!("chaos net dispatch failed: {e:?}")),
    }
    Ok(())
}
