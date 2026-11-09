// see: cans/plugins/db-proxy.md#Plugin-db-proxy
#![forbid(unsafe_code)]

// TODO (L3): tests/test_db_proxy.rs per cans/artifacts/config/project-structure.txt.
pub mod pgwire_db_proxy;
pub mod reassembler_wire;
pub mod service_db_proxy;
pub mod sqlite_db_proxy;
pub use service_db_proxy::SqlProxyService;

use std::sync::Arc;

use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    KelirPlugin, MountScope, NetInterposer, PipelineBinder, VirtioDevice,
};
use kelir_interface::protocol_wire::FilterAction;

use crate::pgwire_db_proxy::PgFrameDescriptor;
use crate::reassembler_wire::{
    FlowKey, NetDirection, NetPlaneState, SegmentOutcome, TCP_FLAG_RST, parse_tcp_segment,
};
use crate::service_db_proxy::{SqlDialect, SqlProxy, StatementHandle};
use crate::sqlite_db_proxy::SqliteBridge;

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
    pub net: Arc<NetPlaneState>,
}

impl DbProxyPlugin {
    pub fn new() -> Self {
        Self {
            config: DbProxyConfig::default(),
            proxy: Arc::new(SqlProxy::new(SqlDialect::Postgres)),
            net: Arc::new(NetPlaneState::new()),
        }
    }

    pub fn with_config(mut self, config: DbProxyConfig) -> Self {
        self.config = config;
        self
    }

    // see: cans/plugins/db-proxy.md#SQLite-UDS-bridge-mechanics:
    pub fn sqlite_bridge(&self) -> SqliteBridge {
        SqliteBridge::new(Arc::clone(&self.proxy))
    }

    fn handle_pgwire_frame(
        &self,
        descriptor: PgFrameDescriptor,
    ) -> Result<FilterAction, PluginError> {
        let _ = descriptor;
        // TODO (L3): in-place Q/P SQL rewrite against rewrite_rules, per-connection rewrite tally above max_rewrites_per_conn aborting ECYCLICREWRITE, and pgwire ErrorResponse synthesis (EPERM→42501, ENOENT→42P01, unmapped→XX000) per cans/plugins/db-proxy.md.
        Ok(FilterAction::Forward)
    }

    // see: cans/07-wire-plane.md#Wire-architecture
    fn intercept_ethernet_frame(
        &self,
        direction: NetDirection,
        frame: &mut [u8],
    ) -> Result<FilterAction, PluginError> {
        let Some(segment) = parse_tcp_segment(frame) else {
            return Ok(FilterAction::Forward);
        };
        // see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
        let (client, server, diverted) = match direction {
            NetDirection::GuestToUpstream => (
                segment.src,
                segment.dst,
                segment.dst.port() == self.config.postgres.listen_port,
            ),
            NetDirection::UpstreamToGuest => (
                segment.dst,
                segment.src,
                segment.src.port() == self.config.postgres.listen_port,
            ),
        };
        if !diverted {
            return Ok(FilterAction::Forward);
        }
        let key = FlowKey { client, server };
        let mut flows = self.net.lock_flows()?;
        if segment.flags & TCP_FLAG_RST != 0 {
            // see: cans/07-wire-plane.md#Frame-drop-and-forward-semantics
            flows.remove(&key);
            return Ok(FilterAction::Forward);
        }
        // TODO (L3): FIN half-close state machine per RFC 793.
        let state = flows.entry(key).or_default();
        match state.reassembly.push_segment(&segment)? {
            SegmentOutcome::Dropped => Ok(FilterAction::Drop {
                errno: None,
                reason:
                    "db-proxy: reassembly carryover saturated; silent discard awaits retransmit"
                        .into(),
            }),
            SegmentOutcome::Forwarded => Ok(FilterAction::Forward),
            SegmentOutcome::Stream(bytes) => match direction {
                NetDirection::GuestToUpstream => {
                    let mut action = FilterAction::Forward;
                    for descriptor in state.decoder.push(&bytes)? {
                        action = self.handle_pgwire_frame(descriptor)?;
                        if action != FilterAction::Forward {
                            break;
                        }
                    }
                    Ok(action)
                }
                // TODO (L3): row-data and ErrorResponse mediation on the server-to-client stream.
                NetDirection::UpstreamToGuest => Ok(FilterAction::Forward),
            },
        }
    }
}

impl Default for DbProxyPlugin {
    fn default() -> Self {
        Self::new()
    }
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
#[derive(Clone)]
pub struct SqlProxyCtx {
    proxy: Arc<SqlProxy>,
}

impl SqlProxyCtx {
    pub fn proxy(&self) -> &Arc<SqlProxy> {
        &self.proxy
    }
}

// see: cans/plugins/transaction.md#Crate-dependencies-(transaction)
#[async_trait::async_trait]
impl SqlProxyService for SqlProxyCtx {
    async fn dialect(&self) -> Result<SqlDialect, PluginError> {
        self.proxy.dialect().await
    }

    async fn register_statement(&self, sql: &str) -> Result<StatementHandle, PluginError> {
        self.proxy.register_statement(sql).await
    }

    async fn lookup_statement(&self, handle: StatementHandle) -> Result<String, PluginError> {
        self.proxy.lookup_statement(handle).await
    }

    async fn issue_commit(&self) -> Result<(), PluginError> {
        self.proxy.issue_commit().await
    }

    async fn issue_rollback(&self) -> Result<(), PluginError> {
        self.proxy.issue_rollback().await
    }

    async fn inject_savepoint(&self, marker: &str) -> Result<(), PluginError> {
        self.proxy.inject_savepoint(marker).await
    }
}

impl KelirPlugin for DbProxyPlugin {
    type Context = SqlProxyCtx;
    fn context(&self) -> Self::Context {
        SqlProxyCtx {
            proxy: Arc::clone(&self.proxy),
        }
    }
    fn name(&self) -> &'static str {
        "db-proxy"
    }
    fn default_scope(&self) -> MountScope {
        // see: cans/plugins/db-proxy.md#Crate-dependencies-(db-proxy)
        MountScope::default().with_devices([VirtioDevice::Vsock, VirtioDevice::Net])
    }
    // see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        let net_stage: Arc<dyn NetInterposer> = Arc::new(self.clone());
        pipeline.bind_net_scoped(net_stage, self.default_scope());
    }
}

// see: cans/plugins/db-proxy.md#L7-pgwire-rewrite-and-2PC-commit
impl NetInterposer for DbProxyPlugin {
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        self.intercept_ethernet_frame(NetDirection::GuestToUpstream, frame)
    }

    fn on_net_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        self.intercept_ethernet_frame(NetDirection::UpstreamToGuest, frame)
    }
}
