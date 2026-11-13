// see: cans/05-storage-plane.md#Storage-quotas
use std::sync::atomic::{AtomicU64, Ordering};

pub struct StorageQuota {
    allocated_bytes: AtomicU64,
    allocated_inodes: AtomicU64,
    max_bytes: u64,
    max_inodes: u64,
}

impl StorageQuota {
    pub fn new(max_bytes: u64, max_inodes: u64) -> Self {
        Self {
            allocated_bytes: AtomicU64::new(0),
            allocated_inodes: AtomicU64::new(0),
            max_bytes,
            max_inodes,
        }
    }

    // see: cans/05-storage-plane.md#Storage-quotas
    pub fn unbounded() -> Self {
        Self::new(u64::MAX, u64::MAX)
    }

    // see: cans/05-storage-plane.md#Storage-quotas
    pub fn check_quota(
        &self,
        bytes_to_write: u64,
        new_inodes: u64,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        accrue_quota(
            &self.allocated_bytes,
            bytes_to_write,
            self.max_bytes,
            "byte",
        )?;
        if let Err(err) = accrue_quota(&self.allocated_inodes, new_inodes, self.max_inodes, "inode")
        {
            self.allocated_bytes
                .fetch_sub(bytes_to_write, Ordering::SeqCst);
            return Err(err);
        }
        Ok(())
    }

    pub fn allocated_bytes(&self) -> u64 {
        self.allocated_bytes.load(Ordering::SeqCst)
    }

    pub fn allocated_inodes(&self) -> u64 {
        self.allocated_inodes.load(Ordering::SeqCst)
    }
}

// see: cans/05-storage-plane.md#Storage-quotas
fn accrue_quota(
    counter: &AtomicU64,
    delta: u64,
    max: u64,
    kind: &str,
) -> Result<(), kelir_interface::plugin_model::PluginError> {
    let mut current = counter.load(Ordering::SeqCst);
    loop {
        let total = current.checked_add(delta).ok_or_else(|| {
            kelir_interface::plugin_model::PluginError::Halt(format!(
                "quota {kind} arithmetic overflow"
            ))
        })?;
        if total > max {
            return Err(kelir_interface::plugin_model::PluginError::reject(
                kelir_interface::protocol_wire::Errno::Enospc,
                format!("storage {kind} quota exceeded"),
            ));
        }
        match counter.compare_exchange(current, total, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return Ok(()),
            Err(observed) => current = observed,
        }
    }
}
