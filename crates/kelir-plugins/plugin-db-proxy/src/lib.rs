// see: cans/plugins/db-proxy.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_db_proxy.rs per cans/artifacts/config/project-structure.txt.
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{KelirPlugin, MountScope, PipelineBinder, VirtioDevice};
use kelir_interface::protocol::StreamKind;

pub const PGWIRE_PORT: u16 = 5432;
pub const MAX_REWRITE_ABORTS: u32 = 3;

#[derive(Debug)]
pub struct SqlProxyService {
    pub host_path: PathBuf,
    pub guest_socket: PathBuf,
    aborts: AtomicU32,
}

impl SqlProxyService {
    pub fn new(host_path: PathBuf) -> Self {
        Self {
            host_path,
            guest_socket: PathBuf::from("/run/kelir/db.sock"),
            aborts: AtomicU32::new(0),
        }
    }

    pub fn stream_kind() -> StreamKind {
        StreamKind::DbSqlite
    }

    pub fn handle_db_sqlite(&self, _frame: &[u8]) -> Result<Vec<u8>, PluginError> {
        // TODO (L3): decode pgwire Q/P frames and rewrite SQL in place; the pgwire parser and TCP reassembly are L3.
        Err(PluginError::Halt(
            "db-proxy: pgwire Q/P decode needs the L3 parser (DB_SQLITE 0x0011)".into(),
        ))
    }

    pub fn record_abort(&self) -> Result<(), PluginError> {
        let aborts = self.aborts.fetch_add(1, Ordering::SeqCst) + 1;
        if aborts > MAX_REWRITE_ABORTS {
            return Err(PluginError::Halt(
                "db-proxy: rewrite aborts exceeded 3 (ECYCLICREWRITE)".into(),
            ));
        }
        Ok(())
    }
}

impl Clone for SqlProxyService {
    fn clone(&self) -> Self {
        Self {
            host_path: self.host_path.clone(),
            guest_socket: self.guest_socket.clone(),
            aborts: AtomicU32::new(self.aborts.load(Ordering::SeqCst)),
        }
    }
}

impl Default for SqlProxyService {
    fn default() -> Self {
        Self::new(PathBuf::from("/var/lib/app/data.sqlite"))
    }
}

#[derive(Clone)]
pub struct DbProxyPlugin {
    pub service: SqlProxyService,
}

impl DbProxyPlugin {
    pub fn new() -> Self {
        Self {
            service: SqlProxyService::default(),
        }
    }
}

impl Default for DbProxyPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for DbProxyPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "db-proxy"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default().with_devices([VirtioDevice::Vsock])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline
            .service_registry()
            .register(Arc::new(self.service.clone()));
    }
}
