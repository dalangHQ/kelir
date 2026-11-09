// see: cans/plugins/db-proxy.md#Invariants-and-policy-(db-proxy)
use std::net::{Ipv4Addr, SocketAddrV4};

use kelir_interface::plugin::NetInterposer;
use kelir_interface::protocol::FilterAction;
use plugin_db_proxy::DbProxyPlugin;
use plugin_db_proxy::pgwire::{PgFrameDescriptor, PgFrameKind};
use plugin_db_proxy::reassembler::FlowKey;

fn pgwire_query_frame(sql: &str) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.push(b'Q');
    let length = u32::try_from(4 + sql.len() + 1).unwrap_or(0);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(sql.as_bytes());
    frame.push(0);
    frame
}

fn tcp_frame(dst_port: u16, seq: u32, payload: &[u8]) -> Vec<u8> {
    let total_len = u16::try_from(20 + 20 + payload.len()).unwrap_or(0);
    let mut frame = Vec::new();
    frame.extend_from_slice(&[0u8; 12]);
    frame.extend_from_slice(&0x0800u16.to_be_bytes());
    frame.push(0x45);
    frame.push(0);
    frame.extend_from_slice(&total_len.to_be_bytes());
    frame.extend_from_slice(&[0u8; 5]);
    frame.push(6);
    frame.extend_from_slice(&[0u8; 2]);
    frame.extend_from_slice(&Ipv4Addr::new(10, 0, 0, 1).octets());
    frame.extend_from_slice(&Ipv4Addr::new(10, 0, 0, 2).octets());
    frame.extend_from_slice(&40000u16.to_be_bytes());
    frame.extend_from_slice(&dst_port.to_be_bytes());
    frame.extend_from_slice(&seq.to_be_bytes());
    frame.extend_from_slice(&[0u8; 4]);
    frame.push(0x50);
    frame.push(0x18);
    frame.extend_from_slice(&[0u8; 6]);
    frame.extend_from_slice(payload);
    frame
}

// see: cans/artifacts/test-plan/cases/relational_transaction/rt_01_pgwire_port_5432_interposition.yaml#case_id
#[test]
fn rt_01_pgwire_port_5432_interposition() -> Result<(), String> {
    assert_eq!(
        PgFrameDescriptor::classify(b'Q', 24),
        PgFrameDescriptor {
            kind: PgFrameKind::Query,
            length: 24,
        }
    );
    assert_eq!(
        PgFrameDescriptor::classify(b'P', 24).kind,
        PgFrameKind::Parse
    );
    assert_eq!(
        PgFrameDescriptor::classify(b'Z', 24).kind,
        PgFrameKind::Other
    );
    let plugin = DbProxyPlugin::new();
    let query = pgwire_query_frame("SELECT * FROM users");
    let mut frame = tcp_frame(5432, 1000, &query);
    match plugin
        .on_net_outbound(&mut frame)
        .map_err(|err| format!("{err:?}"))?
    {
        FilterAction::Forward => (),
        action => {
            return Err(format!(
                "pgwire Query frame to port 5432 must forward after decode, got {action:?}"
            ));
        }
    }
    let key = FlowKey {
        client: SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 40000),
        server: SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 2), 5432),
    };
    let flows = plugin.net.lock_flows().map_err(|err| format!("{err:?}"))?;
    assert!(flows.contains_key(&key));
    assert_eq!(flows.len(), 1);
    drop(flows);
    let other = DbProxyPlugin::new();
    let mut plain = tcp_frame(6379, 1000, &query);
    match other
        .on_net_outbound(&mut plain)
        .map_err(|err| format!("{err:?}"))?
    {
        FilterAction::Forward => (),
        action => {
            return Err(format!(
                "non-5432 frame must forward untouched, got {action:?}"
            ));
        }
    }
    let other_flows = other.net.lock_flows().map_err(|err| format!("{err:?}"))?;
    assert!(other_flows.is_empty());
    Ok(())
}
