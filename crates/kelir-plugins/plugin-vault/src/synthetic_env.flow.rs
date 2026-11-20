// see: cans/plugins/vault.md#Synthetic-inode-ownership-(vault)

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
