// see: cans/plugins/container.md#Plugin-container
// see: cans/plugins/container.md#UID/GID-mapping-isolation
use std::path::Path;

use kelir_interface::plugin_model::PluginError;

// see: cans/plugins/container.md#Plugin-container
pub(crate) fn check_tarball_readable(tarball: &Path) -> Result<(), PluginError> {
    // TODO (L3) [plugin_engine]: stat the tarball and return an ENOENT halt when it is not readable.; slop: an unreadable tarball is never halted, container boot proceeds without an image, the failure surfaces later as a cryptic error.
    let _ = tarball;
    Err(PluginError::Halt(
        "TODO (L3): container tarball readability check pending (L3)".into(),
    ))
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
pub(crate) fn effective_host_uid() -> Result<u32, PluginError> {
    // TODO (L3) [plugin_engine]: read the host uid from the host identity probe.; slop: the host uid is never read, container identity defaults silently, ownership mapping is wrong.
    Err(PluginError::Halt(
        "TODO (L3): host identity probe pending (L3)".into(),
    ))
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
pub(crate) fn passwd_name_for_uid(uid: u32) -> Option<String> {
    // TODO (L3) [plugin_engine]: look up the passwd name for the uid in the host passwd file.; slop: the passwd name is never looked up, the container runs under a wrong or missing user name, permission mapping is silently wrong.
    let _ = uid;
    None
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
pub(crate) fn read_subid_file(path: &str) -> Result<String, PluginError> {
    // TODO (L3) [plugin_engine]: read the subid file at path and return its contents.; slop: the subid file is never read, subordinate id ranges are missing, user-namespace mapping silently fails.
    let _ = path;
    Err(PluginError::Halt(
        "TODO (L3): subid file read pending (L3)".into(),
    ))
}
