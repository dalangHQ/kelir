// see: cans/plugins/profile.md#Synthetic-inode-ownership-(profile)

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
