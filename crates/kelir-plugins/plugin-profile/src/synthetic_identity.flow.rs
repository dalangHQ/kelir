// see: cans/plugins/profile.md#Synthetic-inode-ownership-(profile)
use std::path::Path;

use kelir_interface::plugin_model::{FsInterposer, PluginError};
use kelir_interface::protocol_wire::{PatchPayload, Verdict};

// see: cans/plugins/profile.md#Synthetic-inode-ownership-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyntheticIdentityFile {
    pub path: String,
    pub content: String,
    pub mode: u32,
}

impl SyntheticIdentityFile {
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

// see: cans/plugins/profile.md#Synthetic-inode-ownership-(profile)
#[derive(Debug, Clone, Default)]
pub struct ProfileSyntheticFiles {
    files: Vec<SyntheticIdentityFile>,
}

impl ProfileSyntheticFiles {
    pub fn new(files: Vec<SyntheticIdentityFile>) -> Self {
        Self { files }
    }

    pub fn with_file(mut self, file: SyntheticIdentityFile) -> Self {
        self.files.push(file);
        self
    }

    pub fn files(&self) -> &[SyntheticIdentityFile] {
        &self.files
    }
}

impl FsInterposer for ProfileSyntheticFiles {
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
