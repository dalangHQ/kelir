// see: cans/11-protocol.md#Wire-framing
// see: cans/artifacts/test-plan/cases/protocol_mux/pm_11_wire_framing_checked_math_validation.yaml#case_id
use kelir_engine::bus::{BusDispatcher, MAX_FRAME_PAYLOAD};
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::MsgType;
use kelir_transport::Transport;
use kelir_transport::spsc_ring::OwnedSpscRing;

// see: cans/artifacts/test-plan/cases/protocol_mux/pm_11_wire_framing_checked_math_validation.yaml#case_id
#[test]
fn pm_11_wire_framing_checked_math_validation() -> Result<(), String> {
    let mut control = OwnedSpscRing::new(4096);
    let mut mux = OwnedSpscRing::new(4096);
    let mut writer = mux.transport();
    let mut dispatcher = BusDispatcher::new(control.transport(), mux.transport());
    let mut hostile = [0u8; 9];
    hostile[0..4].copy_from_slice(&0u32.to_le_bytes());
    hostile[4] = 0x02;
    hostile[5..9].copy_from_slice(&u32::MAX.to_le_bytes());
    let written = writer.write(&hostile).map_err(|err| err.to_string())?;
    assert_eq!(written, 9);
    match dispatcher.poll_ingress() {
        Err(PluginError::Halt(_)) => {}
        Err(other) => {
            return Err(format!(
                "hostile frame rejected with unexpected error: {other:?}"
            ));
        }
        Ok(frame) => {
            return Err(format!(
                "hostile frame with u32::MAX length was accepted: {frame:?}"
            ));
        }
    }
    let oversized = vec![0u8; MAX_FRAME_PAYLOAD + 1];
    match dispatcher.send_control_msg(MsgType::Trap, &oversized) {
        Err(PluginError::Halt(_)) => Ok(()),
        Err(other) => Err(format!(
            "oversized control payload rejected with unexpected error: {other:?}"
        )),
        Ok(()) => Err("oversized control payload past the 64 KiB budget was accepted".to_string()),
    }
}
