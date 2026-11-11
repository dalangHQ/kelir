// see: cans/02-plugin-model.md#Declarative-error-mapping
// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
use kelir_interface::error_envelope::{
    EAMBIGUOUSMUTATION_CODE, ECONFIG_CODE, EFFIQUEUESATURATED_CODE, EHALT_CODE, EWXVIOLATION_CODE,
    ErrorEnvelope,
};
use kelir_interface::plugin_model::PluginError;

#[test]
fn wx_violation_maps_to_ewxviolation_envelope() -> Result<(), String> {
    let envelope = ErrorEnvelope::from(PluginError::WxViolation {
        pid: 4242,
        operation: "mprotect(PROT_EXEC)".to_string(),
    });
    assert_eq!(envelope.code, EWXVIOLATION_CODE);
    assert_eq!(envelope.pid, Some(4242));
    assert_eq!(envelope.errno, Some("EPERM".to_string()));
    assert!(envelope.message.contains("4242"));
    assert!(envelope.message.contains("mprotect"));
    Ok(())
}

#[test]
fn ffi_queue_saturation_maps_to_ebusy_envelope() -> Result<(), String> {
    let envelope = ErrorEnvelope::from(PluginError::FfiQueueSaturated { depth: 4096 });
    assert_eq!(envelope.code, EFFIQUEUESATURATED_CODE);
    assert_eq!(envelope.errno, Some("EBUSY".to_string()));
    assert_eq!(envelope.pid, None);
    assert!(envelope.message.contains("4096"));
    Ok(())
}

#[test]
fn ambiguous_mutation_maps_to_eambiguousmutation_envelope() -> Result<(), String> {
    let envelope = ErrorEnvelope::from(PluginError::ambiguous_mutation(
        "/workspace/db.sqlite",
        "plugin-a writes, plugin-b writes",
    ));
    assert_eq!(envelope.code, EAMBIGUOUSMUTATION_CODE);
    assert!(envelope.message.contains("/workspace/db.sqlite"));
    assert_eq!(envelope.errno, None);
    Ok(())
}

#[test]
fn io_failure_maps_to_errno_envelope() -> Result<(), String> {
    let io = std::io::Error::from_raw_os_error(libc::ENOENT);
    let envelope = ErrorEnvelope::from(PluginError::Io(io));
    assert_eq!(envelope.code, "EIO");
    assert_eq!(envelope.errno, Some("2".to_string()));
    assert!(envelope.message.contains("os error 2"));
    Ok(())
}

#[test]
fn reject_maps_errno_to_envelope_code() -> Result<(), String> {
    let envelope = ErrorEnvelope::from(PluginError::reject("EACCES", "git push denied"));
    assert_eq!(envelope.code, "EACCES");
    assert_eq!(envelope.errno, Some("EACCES".to_string()));
    assert_eq!(envelope.message, "git push denied");
    assert_eq!(envelope.pid, None);
    Ok(())
}

#[test]
fn reject_econfig_still_maps_to_econfig_code() -> Result<(), String> {
    let envelope = ErrorEnvelope::from(PluginError::reject(
        "EConfig",
        "profile JSON parse failed: bad syntax",
    ));
    assert_eq!(envelope.code, ECONFIG_CODE);
    assert_eq!(envelope.errno, Some("EConfig".to_string()));
    assert!(envelope.message.contains("profile JSON parse failed"));
    Ok(())
}

#[test]
fn halt_maps_to_ehalt_envelope() -> Result<(), String> {
    let envelope = ErrorEnvelope::from(PluginError::Halt("checkpoint missing".to_string()));
    assert_eq!(envelope.code, EHALT_CODE);
    assert_eq!(envelope.message, "checkpoint missing");
    assert_eq!(envelope.pid, None);
    assert_eq!(envelope.errno, None);
    Ok(())
}

#[test]
fn envelope_serializes_full_context_for_bindings() -> Result<(), String> {
    let envelope = ErrorEnvelope::from(PluginError::WxViolation {
        pid: 4242,
        operation: "memfd_create".to_string(),
    });
    let json = serde_json::to_string(&envelope).map_err(|err| err.to_string())?;
    assert!(json.contains("\"code\":\"EWXVIOLATION\""));
    assert!(json.contains("\"pid\":4242"));
    assert!(json.contains("\"errno\":\"EPERM\""));
    assert!(json.contains("\"driver\":null"));
    Ok(())
}
