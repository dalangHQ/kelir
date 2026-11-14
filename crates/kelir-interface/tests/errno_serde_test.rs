// see: cans/02-plugin-model.md#Unified-verdict
use std::str::FromStr;

use kelir_interface::errno_types::Errno;
use kelir_interface::error_envelope::config_envelope;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::Verdict;

const WIRE: [(Errno, &str); 24] = [
    (Errno::Eperm, "EPERM"),
    (Errno::Eacces, "EACCES"),
    (Errno::Enoent, "ENOENT"),
    (Errno::Einval, "EINVAL"),
    (Errno::Enospc, "ENOSPC"),
    (Errno::Erofs, "EROFS"),
    (Errno::Econnrefused, "ECONNREFUSED"),
    (Errno::Enxio, "ENXIO"),
    (Errno::Ewouldblock, "EWOULDBLOCK"),
    (Errno::Efault, "EFAULT"),
    (Errno::Ebusy, "EBUSY"),
    (Errno::Exdev, "EXDEV"),
    (Errno::Epipe, "EPIPE"),
    (Errno::Enodata, "ENODATA"),
    (Errno::Eproto, "EPROTO"),
    (Errno::Epolicy, "EPOLICY"),
    (Errno::Etoolquota, "ETOOLQUOTA"),
    (Errno::Ewxviolation, "EWXVIOLATION"),
    (Errno::Econfig, "EConfig"),
    (Errno::Esecurityviolation, "ESecurityViolation"),
    (Errno::Eunknownkind, "EUnknownKind"),
    (Errno::Eclockregress, "EClockRegress"),
    (Errno::PathTraversal, "PathTraversal"),
    (Errno::Etoolquota, "ETOOLQUOTA"),
];

#[test]
fn errno_serializes_as_bare_wire_string() -> Result<(), String> {
    for (errno, wire) in WIRE {
        assert_eq!(errno.as_str(), wire);
        let json = serde_json::to_string(&errno).map_err(|err| err.to_string())?;
        assert_eq!(json, format!("\"{wire}\""));
        let back: Errno = serde_json::from_str(&json).map_err(|err| err.to_string())?;
        assert_eq!(back, errno);
    }
    Ok(())
}

#[test]
fn verdict_drop_round_trips_errno_as_string() -> Result<(), String> {
    let verdict = Verdict::Drop {
        errno: Errno::Eperm,
        reason: "denied".to_string(),
    };
    let json = serde_json::to_string(&verdict).map_err(|err| err.to_string())?;
    assert!(json.contains("\"errno\":\"EPERM\""));
    let back: Verdict = serde_json::from_str(&json).map_err(|err| err.to_string())?;
    assert_eq!(back, verdict);
    Ok(())
}

#[test]
fn unknown_errno_string_is_rejected_at_deserialization() -> Result<(), String> {
    let json = "{\"Drop\":{\"errno\":\"EBOGUS\",\"reason\":\"denied\"}}";
    assert!(serde_json::from_str::<Verdict>(json).is_err());
    assert!(serde_json::from_str::<Errno>("\"EBOGUS\"").is_err());
    assert!(Errno::from_str("EBOGUS").is_err());
    Ok(())
}

#[test]
fn halt_display_keeps_wire_code() -> Result<(), String> {
    assert_eq!(
        config_envelope("bad config").to_string(),
        "Plugin halted: EConfig: bad config"
    );
    assert_eq!(
        PluginError::Halt("ESecurityViolation: denied".to_string()).to_string(),
        "Plugin halted: ESecurityViolation: denied"
    );
    Ok(())
}

#[test]
fn raw_os_error_maps_to_named_errno() -> Result<(), String> {
    assert_eq!(
        Errno::from_raw_os_error(libc::ENOENT).map(|errno| errno.as_str()),
        Some("ENOENT")
    );
    assert_eq!(
        Errno::from_raw_os_error(libc::EPIPE).map(|errno| errno.as_str()),
        Some("EPIPE")
    );
    assert_eq!(Errno::from_raw_os_error(9999), None);
    Ok(())
}
