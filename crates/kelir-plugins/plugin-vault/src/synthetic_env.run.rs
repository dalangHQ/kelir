// see: cans/plugins/vault.md#Synthetic-inode-ownership-(vault)
use std::path::Path;

use kelir_interface::plugin_model::{FsInterposer, PluginError};
use kelir_interface::protocol_wire::Verdict;

use crate::synthetic_env::VaultSyntheticFiles;

impl FsInterposer for VaultSyntheticFiles {
    fn on_fs_lookup(&self, _path: &Path) -> Result<Verdict, PluginError> {
        // TODO (L3) [governance_vault]: return the synthetic inode for registered env files; stub passes every lookup.; slop: every lookup passes without a synthetic inode, registered env files are never served, vault-backed env reads silently miss.
        Err(PluginError::Halt(
            "TODO (L3): return the synthetic inode for registered env files".into(),
        ))
    }
}
