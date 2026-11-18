// see: cans/05-storage-plane.md#Storage-architecture
#[path = "storage_snoop.rs"]
pub mod contract;

pub use contract::{SnoopOnWrite, SnoopRule};

impl SnoopRule {
    pub fn new(path: impl Into<String>, sync: bool, on_write: SnoopOnWrite) -> Self {
        Self {
            path: path.into(),
            sync,
            on_write,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl std::fmt::Debug for SnoopRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnoopRule")
            .field("path", &self.path)
            .field("sync", &self.sync)
            .field("on_write", &"callback")
            .finish()
    }
}
