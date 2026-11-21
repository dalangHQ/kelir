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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PgExtendedState {
    #[default]
    Idle,
    Parse,
    Bind,
    Execute,
    Sync,
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PgExtendedQueryState {
    pub state: PgExtendedState,
    pub statement: Option<String>,
    pub portal: Option<String>,
}

impl PgExtendedQueryState {
    pub fn new() -> Self {
        Self::default()
    }

    // see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
    pub fn transition(&mut self, tag: u8) -> Result<(), PluginError> {
        let next = match (self.state, tag) {
            (PgExtendedState::Idle, b'P') => PgExtendedState::Parse,
            (PgExtendedState::Sync, b'P') => PgExtendedState::Parse,
            (PgExtendedState::Parse, b'B') => PgExtendedState::Bind,
            (PgExtendedState::Parse, b'D') => PgExtendedState::Parse,
            (PgExtendedState::Bind, b'E') => PgExtendedState::Execute,
            (PgExtendedState::Bind, b'D') => PgExtendedState::Bind,
            (PgExtendedState::Idle, b'Q') => PgExtendedState::Execute,
            (PgExtendedState::Sync, b'Q') => PgExtendedState::Execute,
            (PgExtendedState::Execute, b'S') => PgExtendedState::Sync,
            (PgExtendedState::Bind, b'S') => PgExtendedState::Sync,
            (PgExtendedState::Parse, b'S') => PgExtendedState::Sync,
            (_, b'X') => PgExtendedState::Idle,
            (_, b'H') => PgExtendedState::Idle,
            _ => {
                return Err(PluginError::Halt(format!(
                    "EPIPE: pgwire extended-query tag {} is invalid in state {:?}",
                    tag as char, self.state
                )));
            }
        };
        self.state = next;
        if tag == b'S' || tag == b'X' || tag == b'H' {
            self.statement = None;
            self.portal = None;
        }
        Ok(())
    }
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
#[derive(Debug, Clone, Default)]
pub struct CancelKeyMap {
    keys: Vec<(u32, u32)>,
}

impl CancelKeyMap {
    pub fn new() -> Self {
        Self::default()
    }

    // see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
    pub fn synthetic_key(process_id: u32) -> u32 {
        process_id.wrapping_mul(0x9E37_79B1).rotate_left(13) ^ 0xA5A5_A5A5
    }

    // see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
    pub fn issue(&mut self, process_id: u32) -> u32 {
        let secret = Self::synthetic_key(process_id);
        if let Some(entry) = self.keys.iter_mut().find(|entry| entry.0 == process_id) {
            entry.1 = secret;
        } else {
            self.keys.push((process_id, secret));
        }
        secret
    }

    // see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
    pub fn validate(&self, process_id: u32, secret: u32) -> bool {
        self.keys
            .iter()
            .any(|entry| entry.0 == process_id && entry.1 == secret)
    }
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedEntry {
    pub name: String,
    pub sql: String,
    pub search_path: String,
    pub role: String,
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
#[derive(Debug, Clone, Default)]
pub struct PreparedCache {
    entries: Vec<PreparedEntry>,
}

impl PreparedCache {
    pub fn new() -> Self {
        Self::default()
    }

    // see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
    pub fn store(&mut self, name: &str, sql: &str, search_path: &str, role: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.name == name) {
            entry.sql = sql.to_string();
            entry.search_path = search_path.to_string();
            entry.role = role.to_string();
        } else {
            self.entries.push(PreparedEntry {
                name: name.to_string(),
                sql: sql.to_string(),
                search_path: search_path.to_string(),
                role: role.to_string(),
            });
        }
    }

    // see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
    pub fn lookup(&self, name: &str) -> Option<String> {
        self.entries
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.sql.clone())
    }

    // see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
    pub fn invalidate_on_context_change(&mut self, search_path: &str, role: &str) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|entry| entry.search_path == search_path && entry.role == role);
        before - self.entries.len()
    }
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlPolicyDecision {
    Allow,
    Deny,
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
pub fn evaluate_sql_policies(
    sql: &str,
    config: &PgWireRewrite,
) -> (SqlPolicyDecision, Vec<String>) {
    let mut reasons = Vec::new();
    let lower = sql.to_ascii_lowercase();
    let trimmed = lower.trim_start();
    if config.require_where_on_mutations
        && (trimmed.starts_with("update") || trimmed.starts_with("delete"))
        && !lower.contains(" where ")
    {
        reasons.push("mutation without WHERE".to_string());
    }
    if config.block_unsafe_io {
        if lower.contains("copy") && lower.contains("program") {
            reasons.push("COPY PROGRAM is unsafe IO".to_string());
        }
        for function in ["pg_read_file", "pg_ls_dir", "dblink"] {
            if lower.contains(function) {
                reasons.push(format!("{function} is unsafe IO"));
            }
        }
    }
    let statements = lower
        .split(';')
        .filter(|part| !part.trim().is_empty())
        .count();
    if statements > 1 {
        reasons.push("multi-statement query".to_string());
    }
    if reasons.is_empty() {
        (SqlPolicyDecision::Allow, reasons)
    } else {
        (SqlPolicyDecision::Deny, reasons)
    }
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
pub fn redis_command_allowed(command: &str) -> bool {
    redis_deny_reason(command).is_none()
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
pub fn redis_deny_reason(command: &str) -> Option<String> {
    let upper = command.to_ascii_uppercase();
    let name = upper.split_ascii_whitespace().next().unwrap_or("");
    if ["EVAL", "EVALSHA", "CONFIG", "DEBUG"].contains(&name) || name.starts_with("FLUSH") {
        Some(format!(
            "Redis command {name} is denied by the dangerous-command gate"
        ))
    } else {
        None
    }
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
pub fn pg_error_response(severity: &str, code: &str, message: &str, hint: Option<&str>) -> Vec<u8> {
    let mut fields = Vec::new();
    for (kind, value) in [
        ('S', severity),
        ('V', severity),
        ('C', code),
        ('M', message),
    ] {
        fields.push(kind as u8);
        fields.extend_from_slice(value.as_bytes());
        fields.push(0);
    }
    if let Some(hint) = hint {
        fields.push(b'H');
        fields.extend_from_slice(hint.as_bytes());
        fields.push(0);
    }
    fields.push(0);
    let len = (fields.len() + 4) as u32;
    let mut out = Vec::with_capacity(1 + fields.len() + 4);
    out.push(b'E');
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&fields);
    out
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
pub use crate::service_db_proxy_run::ast_parse_sql;

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
        // TODO: [relational_transaction]: issue SQL COMMIT across every active proxy connection per cans/plugins/transaction.md; slop: COMMIT returns Ok(()) touching no connection, 2PC commits on one leg (deferred to L3)
        Ok(())
    }

    async fn issue_rollback(&self) -> Result<(), PluginError> {
        // TODO: [relational_transaction]: issue SQL ROLLBACK across every active proxy connection per cans/plugins/transaction.md; slop: ROLLBACK returns Ok(()) touching no connection, locks held after abort (deferred to L3)
        Ok(())
    }

    async fn inject_savepoint(&self, marker: &str) -> Result<(), PluginError> {
        let _ = marker;
        // TODO: [relational_transaction]: inject a transparent SQL SAVEPOINT marker into active pgwire connections per cans/plugins/transaction.md; slop: SAVEPOINT never injected, ROLLBACK TO has no marker to restore (deferred to L3)
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
            ast_parser: "libpgquery".to_string(),
            require_where_on_mutations: true,
            deny_mode_in_tx: "terminate".to_string(),
            block_unsafe_io: true,
            catalog_resolution: true,
            synthetic_cancel_keys: true,
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

    // see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
    pub fn evaluate_sql(&self, sql: &str) -> (SqlPolicyDecision, Vec<String>) {
        evaluate_sql_policies(sql, &self.config.postgres)
    }

    fn handle_pgwire_frame(
        &self,
        descriptor: PgFrameDescriptor,
    ) -> Result<FilterAction, PluginError> {
        let _ = descriptor;
        // TODO: [rt_10_pgwire_frame_rewrite_in_place]: in-place Q/P SQL rewrite against schema_rewrites, per-connection rewrite tally above max_rewrites_per_conn aborting ECYCLICREWRITE, and pgwire ErrorResponse synthesis (EPERM→42501, ENOENT→42P01, unmapped→XX000) per cans/plugins/db-proxy.md; slop: frame Forwarded unrewritten, 4th rewrite passes, no 42501 synthesis (deferred to L3)
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
        // TODO: [relational_transaction]: FIN half-close state machine per RFC 793; slop: half-close ignored, FIN state leaks, peer hangs or resets (deferred to L3)
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
                // TODO: [relational_transaction]: row-data and ErrorResponse mediation on the server-to-client stream; slop: server rows unmediated, ErrorResponse unfiltered, leaks or spoofing pass through (deferred to L3)
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
                "db-proxy stream suspend/patch verdict pending".into(),
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

// see: cans/plugins/db-proxy.md#L7-pgwire-rewrite-and-2PC-commit
impl NetInterposer for DbProxyPlugin {
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        self.intercept_ethernet_frame(NetDirection::GuestToUpstream, frame)
    }

    fn on_net_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        self.intercept_ethernet_frame(NetDirection::UpstreamToGuest, frame)
    }
}
