use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use kelir_interface::driver_interface::{
    DriverCapabilities, DriverError, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver,
    WatchdogAction,
};

// see: cans/artifacts/code/plugin-trait.rs#LifecycleCtx
pub(crate) struct NullDriver;

#[async_trait]
impl VmmDriver for NullDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "unbound",
            cold_boot_budget_ms: 0,
            rollback_budget_ms: 0,
            fork_budget_ms: 0,
            requires_hardware_vmm: false,
            supports_cow_fork: false,
        }
    }

    async fn boot(&mut self, _config: VmConfig) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("no VMM driver bound (L2)".into()))
    }

    async fn set_entropy_backend(&mut self, _backend: EntropyBackend) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("no VMM driver bound (L2)".into()))
    }

    async fn entropy_position(&self) -> Result<EntropyStreamPosition, DriverError> {
        Err(DriverError::Unsupported("no VMM driver bound (L2)".into()))
    }

    async fn restore_entropy_position(
        &mut self,
        _position: EntropyStreamPosition,
    ) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("no VMM driver bound (L2)".into()))
    }

    async fn set_watchdog(
        &mut self,
        _timeout: Duration,
        _action: WatchdogAction,
    ) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("no VMM driver bound (L2)".into()))
    }
}

// see: cans/08-lifecycle.md#Session-incarnation
pub(crate) fn mint_uuid_v7() -> [u8; 16] {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = u64::from(std::process::id());
    let mut id = [0u8; 16];
    let ts_ms = elapsed.as_millis() as u64 & 0x0000_FFFF_FFFF_FFFF;
    id[0..6].copy_from_slice(&ts_ms.to_be_bytes()[2..8]);
    let rand_a = counter & 0x0FFF;
    id[6] = 0x70 | (rand_a >> 8) as u8;
    id[7] = (rand_a & 0xFF) as u8;
    let rand_b =
        ((u64::from(elapsed.subsec_nanos()) << 20) ^ (pid << 8) ^ counter) & 0x3FFF_FFFF_FFFF_FFFF;
    id[8] = 0x80 | (rand_b >> 56) as u8;
    id[9..16].copy_from_slice(&(rand_b & 0x00FF_FFFF_FFFF_FFFF).to_be_bytes()[1..8]);
    id
}

// see: cans/05-storage-plane.md#Storage-quotas
pub(crate) const DEFAULT_WORKSPACE_QUOTA_BYTES: u64 = 512 * 1024 * 1024;

// see: cans/05-storage-plane.md#Storage-quotas
pub(crate) const DEFAULT_WORKSPACE_QUOTA_INODES: u64 = 65_536;
