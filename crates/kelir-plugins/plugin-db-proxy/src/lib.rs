// see: cans/plugins/db-proxy.md#Plugin-db-proxy
#![forbid(unsafe_code)]

// TODO (L3): tests/test_db_proxy.rs per cans/artifacts/config/project-structure.txt.
pub mod pgwire;
pub mod service;
pub use service::SqlProxyService;

use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    HandlerCtx, KelirPlugin, MountScope, NetInterposer, PipelineBinder, StreamInterposer,
    VirtioDevice,
};
use kelir_interface::protocol::{FilterAction, StreamDirection, StreamKind, Verdict};

use crate::pgwire::PgFrameDescriptor;
use crate::service::{SqlDialect, SqlProxy};

// see: cans/artifacts/examples/db-backed-runtime.py#PgWireRewrite
#[derive(Clone, Debug)]
pub struct PgWireRewrite {
    pub listen_port: u16,
    pub upstream: String,
    pub rewrite_rules: Vec<(String, String)>,
    pub max_rewrites_per_conn: u32,
}

impl Default for PgWireRewrite {
    fn default() -> Self {
        Self {
            listen_port: 5432,
            upstream: String::new(),
            rewrite_rules: Vec::new(),
            max_rewrites_per_conn: 3,
        }
    }
}

// see: cans/plugins/db-proxy.md#SQLite-UDS-bridge-mechanics:
#[derive(Clone, Debug)]
pub struct SqliteOverVsock {
    pub guest_socket: String,
    pub host_path: String,
}

impl Default for SqliteOverVsock {
    fn default() -> Self {
        Self {
            guest_socket: "/run/kelir/db.sock".into(),
            host_path: String::new(),
        }
    }
}

// see: cans/plugins/db-proxy.md#Ephemeral-credentials-injected-at-host-connection-boundary
#[derive(Clone, Debug)]
pub struct EphemeralCredentials {
    pub placeholder: String,
    pub ttl_seconds: u32,
}

// TODO (L3): resolve closure injection at the host connection boundary per cans/plugins/db-proxy.md.

// see: cans/artifacts/examples/db-backed-runtime.py#DbProxyConfig
#[derive(Clone, Debug, Default)]
pub struct DbProxyConfig {
    pub postgres: PgWireRewrite,
    pub sqlite: SqliteOverVsock,
    pub credentials: Option<EphemeralCredentials>,
}

// see: cans/plugins/db-proxy.md#Plugin-db-proxy
#[derive(Clone)]
pub struct DbProxyPlugin {
    pub config: DbProxyConfig,
    pub proxy: Arc<SqlProxy>,
}

impl DbProxyPlugin {
    pub fn new() -> Self {
        Self {
            config: DbProxyConfig::default(),
            proxy: Arc::new(SqlProxy::new(SqlDialect::Postgres)),
        }
    }

    pub fn with_config(mut self, config: DbProxyConfig) -> Self {
        self.config = config;
        self
    }

    async fn guard_sqlite_client_bytes(
        &self,
        stream_id: u32,
        payload: &mut [u8],
    ) -> Result<Verdict, PluginError> {
        let _ = (stream_id, payload);
        // TODO (L3): decode the SQLite client protocol on the DB_SQLITE channel and mediate statements through SqlProxyService (Patch rewrite, Drop deny) per cans/plugins/db-proxy.md.
        Ok(Verdict::Pass)
    }

    fn handle_pgwire_frame(
        &self,
        descriptor: PgFrameDescriptor,
    ) -> Result<FilterAction, PluginError> {
        let _ = descriptor;
        // TODO (L3): in-place Q/P SQL rewrite against rewrite_rules, per-connection rewrite tally above max_rewrites_per_conn aborting ECYCLICREWRITE, and pgwire ErrorResponse synthesis (EPERM→42501, ENOENT→42P01, unmapped→XX000) per cans/plugins/db-proxy.md.
        Ok(FilterAction::Forward)
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
        // see: cans/plugins/db-proxy.md#Crate-dependencies-(db-proxy)
        MountScope::default().with_devices([VirtioDevice::Vsock, VirtioDevice::Net])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        let sqlite_stage: Arc<dyn StreamInterposer> = Arc::new(self.clone());
        pipeline.bind_stream_scoped(StreamKind::DbSqlite, sqlite_stage, self.default_scope());
        let net_stage: Arc<dyn NetInterposer> = Arc::new(self.clone());
        pipeline.bind_net_scoped(net_stage, self.default_scope());
        let service: Arc<dyn SqlProxyService> = self.proxy.clone();
        pipeline.service_registry().register(service);
    }
}

// see: cans/plugins/db-proxy.md#SQLite-UDS-bridge-mechanics:
#[async_trait]
impl StreamInterposer for DbProxyPlugin {
    async fn on_stream_open(
        &self,
        stream_id: u32,
        tags: &mut Vec<u8>,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let _ = (tags, ctx);
        self.proxy.track_connection(stream_id)?;
        // TODO (L3): proxy the stream to the host SQLite file at sqlite.host_path per cans/plugins/db-proxy.md.
        Ok(Verdict::Pass)
    }

    async fn on_stream_data(
        &self,
        direction: StreamDirection,
        stream_id: u32,
        payload: &mut Vec<u8>,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let _ = ctx;
        match direction {
            StreamDirection::GuestToHost => {
                self.guard_sqlite_client_bytes(stream_id, payload).await
            }
            StreamDirection::HostToGuest => Ok(Verdict::Pass),
        }
    }

    async fn on_stream_close(&self, stream_id: u32, ctx: &HandlerCtx) -> Result<(), PluginError> {
        let _ = ctx;
        self.proxy.release_connection(stream_id);
        Ok(())
    }
}

// see: cans/plugins/db-proxy.md#L7-pgwire-rewrite-and-2PC-commit
impl NetInterposer for DbProxyPlugin {
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        match PgFrameDescriptor::decode(frame)? {
            Some(descriptor) => self.handle_pgwire_frame(descriptor),
            None => Ok(FilterAction::Forward),
        }
    }
}
