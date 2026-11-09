// see: cans/plugins/vault.md#Plugin-vault
#![forbid(unsafe_code)]

pub mod service_vault;

use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::JoinHandle;

use async_trait::async_trait;
use kelir_interface::credential_vault::CredentialDescriptor;
use kelir_interface::driver_interface::Stream;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    HandlerCtx, KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope, PipelineBinder,
    TokenRevocation, VirtioDevice,
};
use kelir_interface::vsock_mesh::VAULT_METADATA_VSOCK_PORT;
use kelir_interface::vsock_mesh::{VsockPeer, VsockServiceHandler, VsockServiceSpec};

use crate::service_vault::{
    CredentialBroker, CredentialBrokerService, DEFAULT_TOKEN_TTL_SECONDS, IssuedToken,
    TokenRequest, VaultError,
};

// see: cans/plugins/vault.md#Metadata-service-(vault)
pub const METADATA_CREDENTIALS_PREFIX: &str = "/kelir/credentials/";
// see: cans/11-protocol.md#Wire-framing
const METADATA_MAX_REQUEST_BYTES: usize = 64 * 1024;
// see: cans/02-plugin-model.md#Latency-ceiling
const MAX_CONCURRENT_METADATA_SESSIONS: usize = 64;

// see: cans/07-wire-plane.md#Pre-policy-drop-zones
// TODO (L3): pin this constant against the kelir-core vsock transport host-bridge identity.
pub const WIRE_PLANE_BRIDGE_CID: u32 = 2;

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.vault.metadata_service
#[derive(Clone, Debug)]
pub struct VaultConfig {
    pub metadata_vsock_port: u32,
    pub token_ttl_seconds: u32,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            metadata_vsock_port: VAULT_METADATA_VSOCK_PORT,
            token_ttl_seconds: DEFAULT_TOKEN_TTL_SECONDS,
        }
    }
}

// see: cans/plugins/vault.md#Metadata-service-(vault)
pub struct MetadataEndpoint {
    config: VaultConfig,
    broker: Arc<CredentialBroker>,
    serving: Mutex<Vec<JoinHandle<()>>>,
}

impl MetadataEndpoint {
    pub fn new(config: VaultConfig, broker: Arc<CredentialBroker>) -> Self {
        Self {
            config,
            broker,
            serving: Mutex::new(Vec::new()),
        }
    }

    // see: cans/plugins/vault.md#Service-exports-(vault)
    pub fn broker(&self) -> &Arc<CredentialBroker> {
        &self.broker
    }

    // see: cans/02-plugin-model.md#Latency-ceiling
    fn lock_serving(&self) -> Result<MutexGuard<'_, Vec<JoinHandle<()>>>, PluginError> {
        self.serving
            .lock()
            .map_err(|_| PluginError::Halt("metadata serving table lock poisoned".into()))
    }

    // see: cans/02-plugin-model.md#Latency-ceiling
    fn track_serving(&self, handle: JoinHandle<()>) -> Result<(), PluginError> {
        let mut serving = self.lock_serving()?;
        serving.retain(|handle| !handle.is_finished());
        if serving.len() >= MAX_CONCURRENT_METADATA_SESSIONS {
            return Err(PluginError::reject(
                "ECONNREFUSED",
                "metadata service: serving capacity exceeded",
            ));
        }
        serving.push(handle);
        Ok(())
    }

    // see: cans/plugins/vault.md#Metadata-service-(vault)
    pub async fn serve(&self, request: &[u8]) -> Vec<u8> {
        let Some(line) = request_line(request) else {
            return error_response("EACCES", "malformed metadata request");
        };
        let mut parts = line.split_whitespace();
        let Some(method) = parts.next() else {
            return error_response("EACCES", "metadata request carries no method");
        };
        let Some(path) = parts.next() else {
            return error_response("EACCES", "metadata request carries no path");
        };
        if method != "GET" {
            return error_response("EACCES", "metadata service accepts GET only");
        }
        let Some(name) = path.strip_prefix(METADATA_CREDENTIALS_PREFIX) else {
            return error_response("EACCES", "non-conforming metadata request");
        };
        if name.is_empty() || name.contains('/') {
            return error_response("EACCES", "non-conforming credential request");
        }
        let token_request = TokenRequest {
            name: name.into(),
            ttl_seconds: self.config.token_ttl_seconds,
        };
        // TODO (L3): parse ttl_seconds and audience query parameters per cans/plugins/vault.md.
        match self.broker.issue(&token_request).await {
            Ok(token) => ok_response(&token),
            Err(err) => error_response(err.errno(), err.reason()),
        }
    }
}

// see: cans/plugins/vault.md#Metadata-service-(vault)
#[derive(Clone)]
pub struct VaultPlugin {
    config: VaultConfig,
    endpoint: Arc<MetadataEndpoint>,
    session_guest_cid: Option<u32>,
}

impl VaultPlugin {
    pub fn new() -> Self {
        Self {
            config: VaultConfig::default(),
            endpoint: Arc::new(MetadataEndpoint::new(
                VaultConfig::default(),
                Arc::new(CredentialBroker::new(Vec::new())),
            )),
            session_guest_cid: None,
        }
    }

    pub fn with_config(mut self, config: VaultConfig) -> Self {
        self.config = config.clone();
        self.endpoint = Arc::new(MetadataEndpoint::new(
            config,
            self.endpoint.broker().clone(),
        ));
        self
    }

    pub fn with_session_guest_cid(mut self, cid: u32) -> Self {
        self.session_guest_cid = Some(cid);
        self
    }

    pub fn with_credentials(mut self, descriptors: Vec<CredentialDescriptor>) -> Self {
        self.endpoint = Arc::new(MetadataEndpoint::new(
            self.config.clone(),
            Arc::new(CredentialBroker::new(descriptors)),
        ));
        self
    }

    // see: cans/plugins/vault.md#Metadata-service-(vault)
    fn is_session_peer(&self, peer: &VsockPeer) -> bool {
        // TODO (L3): core binds the session guest CID at boot; fail closed until that wire-up lands per cans/plugins/vault.md.
        self.session_guest_cid.is_some_and(|cid| cid == peer.cid)
    }

    // see: cans/02-plugin-model.md#Latency-ceiling
    // TODO (L3): replace the tracked blocking thread with pollable stream reads once the transport exposes readiness.
    fn serve_peer_off_worker(
        &self,
        peer: &VsockPeer,
        stream: Box<dyn Stream>,
    ) -> Result<(), PluginError> {
        let endpoint = Arc::clone(&self.endpoint);
        let peer = peer.clone();
        let handle = std::thread::Builder::new()
            .name("vault-metadata-session".into())
            .spawn(move || {
                // see: cans/plugins/vault.md#Metadata-service-(vault)
                let _ = serve_metadata_session(&endpoint, &peer, stream);
            })
            .map_err(PluginError::Io)?;
        self.endpoint.track_serving(handle)
    }
}

impl Default for VaultPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for VaultPlugin {
    type Context = CredentialBrokerCtx;
    fn context(&self) -> Self::Context {
        CredentialBrokerCtx {
            broker: self.endpoint.broker().clone(),
        }
    }
    fn name(&self) -> &'static str {
        "vault"
    }
    fn default_scope(&self) -> MountScope {
        // see: cans/plugins/vault.md#Crate-dependencies-(vault)
        MountScope::default().with_devices([VirtioDevice::Vsock])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        let handler: Arc<dyn VsockServiceHandler> = Arc::new(self.clone());
        pipeline.bind_vsock_service_scoped(
            VsockServiceSpec {
                port: self.config.metadata_vsock_port,
                service: "kelir-credentials".into(),
            },
            handler,
            self.default_scope(),
        );
        // see: cans/08-lifecycle.md#Rollback
        pipeline.bind_token_revoker(self.endpoint.broker().clone());
    }
}

// see: cans/plugins/vault.md#Service-exports-(vault)
#[derive(Clone)]
pub struct CredentialBrokerCtx {
    broker: Arc<CredentialBroker>,
}

impl CredentialBrokerCtx {
    pub fn broker(&self) -> &Arc<CredentialBroker> {
        &self.broker
    }

    // see: cans/plugins/vault.md#Revocation
    pub async fn revoke_all(&self, session_id: &str) -> Result<usize, VaultError> {
        CredentialBrokerService::revoke_all(self.broker.as_ref(), session_id).await
    }
}

// see: cans/plugins/vault.md#Return-errno-and-rejection-behavior-(vault)
fn halt(err: VaultError) -> PluginError {
    PluginError::reject(err.errno(), err.reason())
}

// see: cans/08-lifecycle.md#Rollback
#[async_trait]
impl TokenRevocation for CredentialBroker {
    // see: cans/08-lifecycle.md#Rollback
    async fn revoke_session_tokens(&self, session_id: &str) -> Result<(), PluginError> {
        CredentialBrokerService::revoke_all(self, session_id)
            .await
            .map_err(halt)?;
        Ok(())
    }
}

// see: cans/plugins/vault.md#Revocation
#[async_trait]
impl LifecycleParticipant for VaultPlugin {
    // see: cans/plugins/vault.md#Revocation
    async fn on_vm_init(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        self.endpoint
            .broker()
            .bind_session(ctx.session_id)
            .map_err(halt)
    }

    // see: cans/plugins/vault.md#Revocation
    async fn on_vm_shutdown(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        self.endpoint
            .broker()
            .revoke_all(ctx.session_id)
            .await
            .map_err(halt)?;
        Ok(())
    }
}

// see: cans/07-wire-plane.md#Pre-policy-drop-zones
#[async_trait]
impl VsockServiceHandler for VaultPlugin {
    async fn on_connect(
        &self,
        peer: &VsockPeer,
        stream: Box<dyn Stream>,
        ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        let _ = ctx;
        // see: cans/plugins/vault.md#Error-and-return-semantics-(vault)
        if !self.is_session_peer(peer) && peer.cid != WIRE_PLANE_BRIDGE_CID {
            return Err(PluginError::reject(
                "ECONNREFUSED",
                "metadata service: non-session vsock peer",
            ));
        }
        self.serve_peer_off_worker(peer, stream)
    }
}

// see: cans/plugins/vault.md#Metadata-service-(vault)
fn serve_metadata_session(
    endpoint: &MetadataEndpoint,
    _peer: &VsockPeer,
    mut stream: Box<dyn Stream>,
) -> Result<(), PluginError> {
    let request = read_metadata_request(stream.as_mut())?;
    let response = block_on(endpoint.serve(&request));
    write_metadata_response(stream.as_mut(), &response)
}

// see: cans/02-plugin-model.md#Latency-ceiling
struct ThreadWaker(std::thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

// see: cans/02-plugin-model.md#Latency-ceiling
// TODO (L3): drive the broker issue on the async runtime once the transport exposes a runtime handle.
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
        }
    }
}

// see: cans/plugins/vault.md#Metadata-service-(vault)
fn read_metadata_request(stream: &mut dyn Stream) -> Result<Vec<u8>, PluginError> {
    let mut request = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        if request.len() >= METADATA_MAX_REQUEST_BYTES {
            return Err(PluginError::reject(
                "EINVAL",
                "metadata request exceeds the 64KB frame budget",
            ));
        }
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
        if find_header_end(&request).is_some() {
            break;
        }
    }
    Ok(request)
}

// see: cans/plugins/vault.md#Metadata-service-(vault)
fn find_header_end(request: &[u8]) -> Option<usize> {
    request.windows(4).position(|window| window == b"\r\n\r\n")
}

fn request_line(request: &[u8]) -> Option<&str> {
    let end = request.iter().position(|byte| *byte == b'\n')?;
    std::str::from_utf8(&request[..end]).ok()
}

// see: cans/plugins/vault.md#Metadata-service-(vault)
fn write_metadata_response(stream: &mut dyn Stream, response: &[u8]) -> Result<(), PluginError> {
    let mut written = 0;
    while written < response.len() {
        let written_now = stream.write(&response[written..])?;
        if written_now == 0 {
            return Err(PluginError::reject(
                "EPIPE",
                "metadata client closed the vsock stream",
            ));
        }
        written += written_now;
    }
    Ok(stream.flush()?)
}

// see: cans/plugins/vault.md#Out-of-band-credential-brokerage
fn ok_response(token: &IssuedToken) -> Vec<u8> {
    let body = format!(
        "{{\"token\":\"{}\",\"ttl_seconds\":{},\"scope\":\"{}\",\"audience\":\"{}\"}}",
        json_string(&token.token),
        token.ttl_seconds,
        json_string(&token.scope),
        json_string(&token.audience),
    );
    http_response("200 OK", &body)
}

// see: cans/plugins/vault.md#Error-and-return-semantics-(vault)
fn error_response(errno: &str, reason: &str) -> Vec<u8> {
    let body = format!(
        "{{\"errno\":\"{}\",\"reason\":\"{}\"}}",
        json_string(errno),
        json_string(reason),
    );
    let status = match errno {
        "ENODATA" => "404 Not Found",
        "EINVAL" => "400 Bad Request",
        _ => "403 Forbidden",
    };
    http_response(status, &body)
}

// see: cans/plugins/vault.md#Guest-integration
fn http_response(status: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len(),
    )
    .into_bytes()
}

// TODO (L3): replace with the workspace JSON encoder once dependencies permit; minimal escaping skeleton only.
fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ if (ch as u32) < 0x20 => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            _ => escaped.push(ch),
        }
    }
    escaped
}
