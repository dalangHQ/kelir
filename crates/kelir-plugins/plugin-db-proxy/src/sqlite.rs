// see: cans/plugins/db-proxy.md#SQLite-UDS-bridge-mechanics:
use std::sync::Arc;

use kelir_interface::plugin::PluginError;
use kelir_interface::protocol::{StreamKind, Verdict};

use crate::service::SqlProxy;

// see: cans/plugins/db-proxy.md#Crate-dependencies-(db-proxy)
pub const SQLITE_BRIDGE_STREAM_KIND: StreamKind = StreamKind::DbSqlite;

// see: cans/plugins/db-proxy.md#SQLite-UDS-bridge-mechanics:
#[derive(Clone)]
pub struct SqliteBridge {
    proxy: Arc<SqlProxy>,
}

impl SqliteBridge {
    pub fn new(proxy: Arc<SqlProxy>) -> Self {
        Self { proxy }
    }

    // see: cans/plugins/db-proxy.md#SQLite-UDS-bridge-mechanics:
    // TODO (L3): core dispatches DB_SQLITE mux streams into this bridge; kelir-interface exposes no mux-kind binder yet.
    pub fn on_client_open(&self, stream_id: u32) -> Result<(), PluginError> {
        self.proxy.track_connection(stream_id)
    }

    // see: cans/plugins/db-proxy.md#SQLite-UDS-bridge-mechanics:
    pub fn on_client_bytes(
        &self,
        stream_id: u32,
        payload: &mut [u8],
    ) -> Result<Verdict, PluginError> {
        let _ = (stream_id, payload);
        // TODO (L3): decode the SQLite client protocol on the DB_SQLITE channel and mediate statements through SqlProxyService (Patch rewrite, Drop deny) per cans/plugins/db-proxy.md.
        Ok(Verdict::Pass)
    }

    // see: cans/plugins/db-proxy.md#SQLite-UDS-bridge-mechanics:
    pub fn on_client_close(&self, stream_id: u32) {
        self.proxy.release_connection(stream_id)
    }
}
