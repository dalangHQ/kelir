// see: cans/plugins/db-proxy.md#Plugin-db-proxy
#![forbid(unsafe_code)]

// TODO (L3) [relational_transaction]: tests/test_db_proxy.rs per cans/artifacts/config/project-structure.txt; slop: test_db_proxy.rs missing, proxy behavior untested, Forward stub passes vacuously
#[path = "pgwire_db_proxy.flow.rs"]
pub mod pgwire_db_proxy;
#[path = "reassembler_wire.flow.rs"]
pub mod reassembler_wire;
#[path = "receipt_sql.flow.rs"]
pub mod receipt_sql;
#[path = "service_db_proxy.flow.rs"]
pub mod service_db_proxy;
#[path = "service_db_proxy.run.rs"]
pub mod service_db_proxy_run;
#[path = "sqlite_db_proxy.flow.rs"]
pub mod sqlite_db_proxy;
pub use receipt_sql::{NET_L7_SQL_NAMESPACE, NetL7SqlExt, SqlReceiptExt};
pub use service_db_proxy::{
    CancelKeyMap, PgExtendedQueryState, PgExtendedState, PreparedCache, PreparedEntry,
    SqlPolicyDecision, SqlProxyService, ast_parse_sql, evaluate_sql_policies, pg_error_response,
    redis_command_allowed, redis_deny_reason,
};

use std::sync::Arc;

use kelir_interface::plugin_model::PluginDeps;

use crate::reassembler_wire::NetPlaneState;
use crate::service_db_proxy::SqlProxy;

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.db_proxy.postgres.schema_rewrites
#[derive(Clone, Debug, Default)]
pub struct PgSchemaRewrite {
    pub from_schema: String,
    pub to_schema: String,
}

// see: cans/artifacts/examples/db-backed-runtime.py#PgWireRewrite
#[derive(Clone, Debug)]
pub struct PgWireRewrite {
    pub listen_port: u16,
    pub upstream: String,
    // see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.db_proxy.postgres.schema_rewrites
    pub schema_rewrites: Vec<PgSchemaRewrite>,
    pub max_rewrites_per_conn: u32,
    pub ast_parser: String,
    pub require_where_on_mutations: bool,
    pub deny_mode_in_tx: String,
    pub block_unsafe_io: bool,
    pub catalog_resolution: bool,
    pub synthetic_cancel_keys: bool,
}

// see: cans/plugins/db-proxy.md#SQLite-UDS-bridge-mechanics:
#[derive(Clone, Debug)]
pub struct SqliteOverVsock {
    pub guest_socket: String,
    pub host_path: String,
}

// see: cans/plugins/db-proxy.md#Ephemeral-credentials-injected-at-host-connection-boundary
#[derive(Clone, Debug)]
pub struct EphemeralCredentials {
    pub placeholder: String,
    pub ttl_seconds: u32,
}

// TODO (L3) [rt_12_ephemeral_db_credentials_injection]: resolve closure injection at the host connection boundary per cans/plugins/db-proxy.md; slop: guest connection string carries password, secret enters guest

// see: cans/artifacts/examples/db-backed-runtime.py#DbProxyConfig
#[derive(Clone, Debug, Default)]
pub struct DbProxyConfig {
    pub postgres: PgWireRewrite,
    pub sqlite: SqliteOverVsock,
    pub credentials: Option<EphemeralCredentials>,
}

// see: cans/plugins/db-proxy.md#Plugin-db-proxy
#[derive(Clone, PluginDeps)]
pub struct DbProxyPlugin {
    pub config: DbProxyConfig,
    pub proxy: Arc<SqlProxy>,
    pub net: Arc<NetPlaneState>,
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
#[derive(Clone)]
pub struct SqlProxyCtx {
    proxy: Arc<SqlProxy>,
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
pub type SqlProxyHandle = SqlProxyCtx;

// see: cans/02-plugin-model.md#KelirPlugin-trait
