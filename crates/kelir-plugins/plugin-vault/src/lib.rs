// see: cans/plugins/vault.md
#![forbid(unsafe_code)]

pub mod service;

use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::driver::Stream;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    HandlerCtx, KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope, PipelineBinder,
    VirtioDevice,
};
use kelir_interface::vsock::{VsockPeer, VsockServiceHandler, VsockServiceSpec};

use crate::service::{
    CredentialBroker, CredentialBrokerService, CredentialDescriptor, DEFAULT_TOKEN_TTL_SECONDS,
    IssuedToken, TokenRequest,
};

// see: cans/plugins/vault.md
pub const METADATA_CREDENTIALS_PREFIX: &str = "/kelir/credentials/";
// see: cans/11-protocol.md#Wire-framing
const METADATA_MAX_REQUEST_BYTES: usize = 64 * 1024;

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.vault.metadata_service
#[derive(Clone, Debug)]
pub struct VaultConfig {
    pub metadata_vsock_port: u32,
    pub token_ttl_seconds: u32,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            metadata_vsock_port: 80,
            token_ttl_seconds: DEFAULT_TOKEN_TTL_SECONDS,
        }
    }
}

// see: cans/plugins/vault.md
#[derive(Clone)]
pub struct VaultPlugin {
    config: VaultConfig,
    broker: Arc<CredentialBroker>,
    session_guest_cid: Option<u32>,
}

impl VaultPlugin {
    pub fn new() -> Self {
        Self {
            config: VaultConfig::default(),
            broker: Arc::new(CredentialBroker::new(Vec::new())),
            session_guest_cid: None,
        }
    }

    pub fn with_config(mut self, config: VaultConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_session_guest_cid(mut self, cid: u32) -> Self {
        self.session_guest_cid = Some(cid);
        self
    }

    pub fn with_credentials(mut self, descriptors: Vec<CredentialDescriptor>) -> Self {
        self.broker = Arc::new(CredentialBroker::new(descriptors));
        self
    }

    fn is_session_peer(&self, peer: &VsockPeer) -> bool {
        // TODO (L3): core binds the session guest CID at boot; fail closed until that wire-up lands per cans/plugins/vault.md.
        self.session_guest_cid.is_some_and(|cid| cid == peer.cid)
    }

    async fn serve_metadata_request(&self, request: &[u8]) -> Vec<u8> {
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

impl Default for VaultPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for VaultPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "vault"
    }
    fn default_scope(&self) -> MountScope {
        // see: cans/plugins/vault.md
        MountScope::default().with_devices([VirtioDevice::Vsock])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        let handler: Arc<dyn VsockServiceHandler> = Arc::new(self.clone());
        pipeline.bind_vsock_service(
            VsockServiceSpec {
                port: self.config.metadata_vsock_port,
                service: "kelir-credentials".into(),
            },
            handler,
        );
        let broker: Arc<dyn CredentialBrokerService> = self.broker.clone();
        pipeline.service_registry().register(broker);
    }
}

// see: cans/plugins/vault.md
#[async_trait]
impl LifecycleParticipant for VaultPlugin {
    async fn on_vm_shutdown(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        self.broker.revoke_session_tokens();
        Ok(())
    }
}

// see: cans/plugins/vault.md
#[async_trait]
impl VsockServiceHandler for VaultPlugin {
    async fn on_connect(
        &self,
        peer: &VsockPeer,
        mut stream: Box<dyn Stream>,
        ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        let _ = ctx;
        if !self.is_session_peer(peer) {
            return Err(PluginError::reject(
                "ECONNREFUSED",
                "metadata service: non-session vsock peer",
            ));
        }
        let request = read_metadata_request(stream.as_mut())?;
        let response = self.serve_metadata_request(&request).await;
        write_metadata_response(stream.as_mut(), &response)
    }
}

// see: cans/plugins/vault.md
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

// see: cans/plugins/vault.md
fn find_header_end(request: &[u8]) -> Option<usize> {
    request.windows(4).position(|window| window == b"\r\n\r\n")
}

fn request_line(request: &[u8]) -> Option<&str> {
    let end = request.iter().position(|byte| *byte == b'\n')?;
    std::str::from_utf8(&request[..end]).ok()
}

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
