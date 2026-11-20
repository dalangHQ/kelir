// see: cans/plugins/profile.md#Synthetic-inode-ownership-(profile)
use std::path::Path;

use kelir_interface::plugin_model::{FsInterposer, PluginError};
use kelir_interface::protocol_wire::Verdict;

use crate::synthetic_identity::ProfileSyntheticFiles;

impl FsInterposer for ProfileSyntheticFiles {
    fn on_fs_lookup(&self, path: &Path) -> Result<Verdict, PluginError> {
        let _ = path;
        // TODO (L3) [bindings_py]: return the synthetic-inode Patch for the matching file, else Pass.; slop: the synthetic-inode Patch is never returned, matching files Pass through unpatched, profile identity is silently absent.
        Err(PluginError::Halt(
            "TODO (L3): return the synthetic-inode Patch for the matching file".into(),
        ))
    }
}
