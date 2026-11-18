// see: cans/plugins/vault.md#Synthetic-inode-ownership-(vault)
use std::path::Path;

use kelir_interface::plugin_model::{FsInterposer, PluginError};
use kelir_interface::protocol_wire::{PatchPayload, Verdict};

// see: cans/plugins/vault.md#Synthetic-inode-ownership-(vault)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyntheticEnvFile {
    pub path: String,
    pub content: String,
    pub mode: u32,
}

impl SyntheticEnvFile {
    pub fn new(path: &str, content: &str) -> Self {
        Self {
            path: path.to_string(),
            content: content.to_string(),
            mode: 0o400,
        }
    }

    pub fn with_mode(path: &str, content: &str, mode: u32) -> Self {
        Self {
            path: path.to_string(),
            content: content.to_string(),
            mode,
        }
    }
}

// see: cans/plugins/vault.md#Synthetic-inode-ownership-(vault)
#[derive(Debug, Clone, Default)]
pub struct VaultSyntheticFiles {
    files: Vec<SyntheticEnvFile>,
}

impl VaultSyntheticFiles {
    pub fn new(files: Vec<SyntheticEnvFile>) -> Self {
        Self { files }
    }

    pub fn with_file(mut self, file: SyntheticEnvFile) -> Self {
        self.files.push(file);
        self
    }

    pub fn files(&self) -> &[SyntheticEnvFile] {
        &self.files
    }
}

impl FsInterposer for VaultSyntheticFiles {
    fn on_fs_lookup(&self, path: &Path) -> Result<Verdict, PluginError> {
        for file in &self.files {
            if Path::new(&file.path) == path {
                return Ok(Verdict::Patch(PatchPayload::FsInode {
                    mode: file.mode,
                    uid: 0,
                    gid: 0,
                    size: file.content.len() as u64,
                    content: file.content.clone().into_bytes(),
                }));
            }
        }
        Ok(Verdict::Pass)
    }
}
