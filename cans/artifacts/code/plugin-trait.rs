// crates/kelir-core/src/plugin.rs
//
// The unified KelirPlugin trait implemented by every subsystem — internal
// or third-party — across the 4 VirtIO rings. kelir-core contains zero
// hardcoded capability logic; every capability is a plugin.
//
// Canonical spec home: cans/02-plugin-model.md#KelirPlugin-trait

use async_trait::async_trait;
use std::path::Path;
use crate::handlers::HandlerCtx;
use crate::protocol::messages::{TrapEvent, TrapAction};
use crate::session::Session;
use crate::vmm::driver::VmmDriver;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin execution rejected: {0}")]
    Halt(String),
    #[error("Internal I/O failure: {0}")]
    Io(#[from] std::io::Error),
}

/// Complete inode attribute set surfaced by storage-plane lookups. FUSE `LOOKUP`
/// replies are populated verbatim from this struct (`st_mode`, `st_uid`, `st_gid`,
/// `st_size`) alongside the content bytes.
#[derive(Debug, Clone)]
pub struct VfsInodeAttr {
    /// File mode bits (e.g. `0o400` read-only, `0o600` private)
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub content: Vec<u8>,
}

#[async_trait]
pub trait KelirPlugin: Send + Sync {
    /// Unique identifier for telemetry and ordering
    fn name(&self) -> &'static str;

    // ─── 1. MicroVM Lifecycle Hooks ─────────────────────────
    async fn on_vm_init(&self, _driver: &mut Box<dyn VmmDriver>) -> Result<(), PluginError> { Ok(()) }
    async fn on_vm_ready(&self, _session: &Session) -> Result<(), PluginError> { Ok(()) }
    async fn on_vm_shutdown(&self, _session: &Session) -> Result<(), PluginError> { Ok(()) }

    // ─── 2. Control Plane Hooks (virtio-vsock) ───────────────
    async fn on_trap(&self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Option<TrapAction>, PluginError> { Ok(None) }
    async fn on_process_exit(&self, _pid: u32, _exit_code: i32) -> Result<(), PluginError> { Ok(()) }

    // ─── 3. Terminal Plane Hooks (virtio-console) ────────────
    async fn on_pty_input(&self, _bytes: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
    async fn on_pty_output(&self, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }

    // ─── 4. Storage Plane Hooks (virtio-fs) ──────────────────
    async fn on_vfs_lookup(&self, _path: &Path) -> Result<Option<VfsInodeAttr>, PluginError> { Ok(None) }
    async fn on_vfs_read(&self, _path: &Path, _buffer: &mut [u8]) -> Result<(), PluginError> { Ok(()) }
    async fn on_vfs_write(&self, _path: &Path, _data: &[u8], _pid: u32, _ctx: &HandlerCtx) -> Result<bool, PluginError> { Ok(true) }

    // ─── 5. Wire Plane Hooks (virtio-net) ────────────────────
    async fn on_net_outbound(&self, _frame: &mut Vec<u8>) -> Result<bool, PluginError> { Ok(true) }
    async fn on_net_inbound(&self, _frame: &mut Vec<u8>) -> Result<bool, PluginError> { Ok(true) }
}
