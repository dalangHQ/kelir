// see: cans/artifacts/test-plan/cases/plugin_engine/pe_15_interface_rejects_emulated_deterministic_clock.yaml#case_id
use kelir_interface::driver_interface::{VmmError, reject_emulated_deterministic_clock};

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_15_interface_rejects_emulated_deterministic_clock.yaml#case_id
#[test]
fn pe_15_interface_rejects_emulated_deterministic_clock() -> Result<(), String> {
    match reject_emulated_deterministic_clock("bwrap", true) {
        Err(VmmError::Unsupported(_)) => {}
        Err(e) => {
            return Err(format!(
                "emulated deterministic clock gave wrong error: {e}"
            ));
        }
        Ok(()) => return Err("emulated deterministic clock allowed".to_string()),
    }
    match reject_emulated_deterministic_clock("bwrap", false) {
        Ok(()) => {}
        Err(e) => return Err(format!("non-deterministic clock rejected: {e}")),
    }
    match reject_emulated_deterministic_clock("kvm", true) {
        Err(VmmError::Unsupported(_)) => {}
        Err(e) => {
            return Err(format!(
                "hardware deterministic clock gave wrong error: {e}"
            ));
        }
        Ok(()) => return Err("hardware deterministic clock allowed".to_string()),
    }
    match reject_emulated_deterministic_clock("kvm", false) {
        Ok(()) => {}
        Err(e) => return Err(format!("hardware non-deterministic clock rejected: {e}")),
    }
    Ok(())
}
