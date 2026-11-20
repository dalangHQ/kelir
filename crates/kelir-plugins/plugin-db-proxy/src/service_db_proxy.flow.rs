// see: cans/plugins/db-proxy.md#Plugin-db-proxy
// see: cans/plugins/transaction.md#Crate-dependencies-(transaction)
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use async_trait::async_trait;
use std::sync::Arc;

use kelir_interface::errno_types::Errno;
use kelir_interface::plugin_model::{
    KelirPlugin, MountScope, NetInterposer, PipelineBinder, PluginError, StreamFrameSink,
    VirtioDevice,
};
use kelir_interface::protocol_wire::{FilterAction, StreamKind, Verdict};

use crate::pgwire_db_proxy::PgFrameDescriptor;
use crate::reassembler_wire::{
    FlowKey, NetDirection, NetPlaneState, SegmentOutcome, TCP_FLAG_RST, parse_tcp_segment,
};
use crate::sqlite_db_proxy::SqliteBridge;
use crate::{
    DbProxyConfig, DbProxyPlugin, EphemeralCredentials, PgWireRewrite, SqlProxyCtx, SqliteOverVsock,
};

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlDialect {
    Postgres,
    Sqlite,
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatementHandle {
    pub id: u64,
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
#[async_trait]
pub trait SqlProxyService: Send + Sync {
    async fn dialect(&self) -> Result<SqlDialect, PluginError>;
    async fn register_statement(&self, sql: &str) -> Result<StatementHandle, PluginError>;
    async fn lookup_statement(&self, handle: StatementHandle) -> Result<String, PluginError>;
    async fn issue_commit(&self) -> Result<(), PluginError>;
    async fn issue_rollback(&self) -> Result<(), PluginError>;
    async fn inject_savepoint(&self, marker: &str) -> Result<(), PluginError>;
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
struct StatementTable {
    next_id: u64,
    statements: HashMap<u64, String>,
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
pub struct SqlProxy {
    dialect: SqlDialect,
    statements: Mutex<StatementTable>,
    connections: Mutex<HashSet<u32>>,
    rewrite_target: Mutex<String>,
}

impl SqlProxy {
    pub fn new(dialect: SqlDialect) -> Self {
        Self {
            dialect,
            statements: Mutex::new(StatementTable {
                next_id: 0,
                statements: HashMap::new(),
            }),
            connections: Mutex::new(HashSet::new()),
            rewrite_target: Mutex::new(String::new()),
        }
    }

    // see: cans/plugins/db-proxy.md#L7-pgwire-rewrite-and-2PC-commit
    pub fn set_rewrite_target(&self, target: &str) {
        if let Ok(mut current) = self.rewrite_target.lock() {
            *current = target.to_string();
        }
    }

    // see: cans/plugins/db-proxy.md#L7-pgwire-rewrite-and-2PC-commit
    pub fn rewrite_target(&self) -> String {
        self.rewrite_target
            .lock()
            .map(|current| current.clone())
            .unwrap_or_default()
    }

    pub fn track_connection(&self, stream_id: u32) -> Result<(), PluginError> {
        let mut connections = self
            .connections
            .lock()
            .map_err(|_| PluginError::Halt("proxy connection table poisoned".into()))?;
        connections.insert(stream_id);
        Ok(())
    }

    pub fn release_connection(&self, stream_id: u32) {
        if let Ok(mut connections) = self.connections.lock() {
            connections.remove(&stream_id);
        }
    }
}

#[async_trait]
impl SqlProxyService for SqlProxy {
    async fn dialect(&self) -> Result<SqlDialect, PluginError> {
        Ok(self.dialect)
    }

    async fn register_statement(&self, sql: &str) -> Result<StatementHandle, PluginError> {
        let mut table = self
            .statements
            .lock()
            .map_err(|_| PluginError::Halt("statement table poisoned".into()))?;
        table.next_id = table.next_id.saturating_add(1);
        let id = table.next_id;
        table.statements.insert(id, sql.into());
        Ok(StatementHandle { id })
    }

    async fn lookup_statement(&self, handle: StatementHandle) -> Result<String, PluginError> {
        let table = self
            .statements
            .lock()
            .map_err(|_| PluginError::Halt("statement table poisoned".into()))?;
        match table.statements.get(&handle.id) {
            Some(sql) => Ok(sql.clone()),
            None => Err(PluginError::Halt(
                "ENOENT: statement handle is not registered".into(),
            )),
        }
    }

    async fn issue_commit(&self) -> Result<(), PluginError> {
        // TODO (L3) [relational_transaction]: issue SQL COMMIT across every active proxy connection per cans/plugins/transaction.md; slop: COMMIT returns Ok(()) touching no connection, 2PC commits on one leg
        Ok(())
    }

    async fn issue_rollback(&self) -> Result<(), PluginError> {
        // TODO (L3) [relational_transaction]: issue SQL ROLLBACK across every active proxy connection per cans/plugins/transaction.md; slop: ROLLBACK returns Ok(()) touching no connection, locks held after abort
        Ok(())
    }

    async fn inject_savepoint(&self, marker: &str) -> Result<(), PluginError> {
        let _ = marker;
        // TODO (L3) [relational_transaction]: inject a transparent SQL SAVEPOINT marker into active pgwire connections per cans/plugins/transaction.md; slop: SAVEPOINT never injected, ROLLBACK TO has no marker to restore
        Ok(())
    }
}

impl Default for PgWireRewrite {
    fn default() -> Self {
        Self {
            listen_port: 5432,
            upstream: String::new(),
            schema_rewrites: Vec::new(),
            max_rewrites_per_conn: 3,
        }
    }
}

impl Default for SqliteOverVsock {
    fn default() -> Self {
        Self {
            guest_socket: "/run/kelir/db.sock".into(),
            host_path: String::new(),
        }
    }
}

impl Default for EphemeralCredentials {
    fn default() -> Self {
        Self {
            placeholder: String::new(),
            ttl_seconds: 60,
        }
    }
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
        if let Some(rule) = config.postgres.schema_rewrites.first() {
            self.proxy.set_rewrite_target(&rule.to_schema);
        }
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
        // TODO (L3) [rt_10_pgwire_frame_rewrite_in_place]: in-place Q/P SQL rewrite against schema_rewrites, per-connection rewrite tally above max_rewrites_per_conn aborting ECYCLICREWRITE, and pgwire ErrorResponse synthesis (EPERM→42501, ENOENT→42P01, unmapped→XX000) per cans/plugins/db-proxy.md; slop: frame Forwarded unrewritten, 4th rewrite passes, no 42501 synthesis
        Ok(FilterAction::Forward)
    }

    // see: cans/07-wire-plane.md#Wire-architecture
    pub(crate) fn intercept_ethernet_frame(
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
        // TODO (L3) [relational_transaction]: FIN half-close state machine per RFC 793; slop: half-close ignored, FIN state leaks, peer hangs or resets
        let state = flows.entry(key).or_default();
        match state.reassembly.push_segment(&segment)? {
            SegmentOutcome::Dropped => Ok(FilterAction::Drop {
                errno: Errno::Eperm,
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
                // TODO (L3) [relational_transaction]: row-data and ErrorResponse mediation on the server-to-client stream; slop: server rows unmediated, ErrorResponse unfiltered, leaks or spoofing pass through
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

impl SqlProxyCtx {
    // see: cans/02-plugin-model.md#KelirPlugin-trait
    pub fn service(&self) -> &Arc<SqlProxy> {
        &self.proxy
    }

    // see: cans/plugins/db-proxy.md#L7-pgwire-rewrite-and-2PC-commit
    pub async fn rewrite_target(&self) -> String {
        self.proxy.rewrite_target()
    }
}

// see: cans/plugins/db-proxy.md#SQLite-UDS-bridge-mechanics:
// see: cans/11-protocol.md#Kind-registration
struct DbSqliteStreamSink {
    bridge: SqliteBridge,
}

// see: cans/11-protocol.md#Kind-registration
impl StreamFrameSink for DbSqliteStreamSink {
    fn on_stream_frame(
        &self,
        stream_id: u32,
        kind: u16,
        _flags: u8,
        payload: &[u8],
    ) -> Result<(), PluginError> {
        if kind != StreamKind::DbSqlite as u16 {
            return Ok(());
        }
        let mut frame = payload.to_vec();
        match self.bridge.on_client_bytes(stream_id, &mut frame)? {
            Verdict::Pass => Ok(()),
            Verdict::Drop { errno, reason } => {
                Err(PluginError::Halt(format!("{}: {}", errno.as_str(), reason)))
            }
            Verdict::Abort { reason } => Err(PluginError::Halt(reason)),
            _ => Err(PluginError::Halt(
                "db-proxy stream suspend/patch verdict pending (L3)".into(),
            )),
        }
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
    // see: cans/11-protocol.md#Kind-registration
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        let net_stage: Arc<dyn NetInterposer> = Arc::new(self.clone());
        pipeline.bind_net_scoped(net_stage, self.default_scope());
        let sink: Arc<dyn StreamFrameSink> = Arc::new(DbSqliteStreamSink {
            bridge: self.sqlite_bridge(),
        });
        pipeline.bind_stream_kind_scoped(
            StreamKind::DbSqlite as u16,
            sink,
            MountScope::default().with_stream_kinds([StreamKind::DbSqlite]),
        );
    }
}
