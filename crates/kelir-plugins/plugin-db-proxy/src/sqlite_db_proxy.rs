// see: cans/plugins/db-proxy.md#SQLite-UDS-bridge-mechanics:
use std::sync::Arc;

use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::{StreamKind, Verdict};

use crate::service_db_proxy::SqlProxy;

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
    // TODO (L3) [rt_02_sqlite_uds_bridge_mux_delegation]: core dispatches DB_SQLITE mux streams into this bridge; kelir-interface exposes no mux-kind binder yet; slop: no mux-kind binder, DB_SQLITE stream never dispatched, bridge dead
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
        // TODO (L3) [rt_02_sqlite_uds_bridge_mux_delegation]: decode the SQLite client protocol on the DB_SQLITE channel and mediate statements through SqlProxyService (Patch rewrite, Drop deny) per cans/plugins/db-proxy.md; slop: SQLite bytes unmediated, statements bypass Patch rewrite and Drop deny
        Ok(Verdict::Pass)
    }

    // see: cans/plugins/db-proxy.md#SQLite-UDS-bridge-mechanics:
    pub fn on_client_close(&self, stream_id: u32) {
        self.proxy.release_connection(stream_id)
    }
}
