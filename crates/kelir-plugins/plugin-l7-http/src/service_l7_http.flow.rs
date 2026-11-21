// see: cans/plugins/l7-http.md#Service-exports-(l7-http)
use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::errno_types::Errno;
use kelir_interface::plugin_model::{
    KelirPlugin, MountScope, PipelineBinder, PluginDeps, VirtioDevice,
};

use crate::l7_parser::{
    HttpRequest, HttpResponse, L7ParseError, L7Parser, is_path_traversal, synthetic_403_receipt,
};
use crate::l7_tls::{HostKeyringDecryptor, TlsDecryptor};

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinkholeRule {
    pub host: String,
    pub methods: Option<Vec<String>>,
    pub paths: Option<Vec<String>>,
    pub allow: bool,
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SinkholeConfig {
    pub rules: Vec<SinkholeRule>,
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
impl SinkholeConfig {
    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    pub fn ensure_drop_all(&mut self) {
        if !self.rules.iter().any(|rule| !rule.allow) {
            self.rules.push(SinkholeRule {
                host: "*".to_string(),
                methods: None,
                paths: None,
                allow: false,
            });
        }
    }

    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    pub fn verdict(&self, host: &str, method: &str, path: &str) -> bool {
        for rule in &self.rules {
            if !host_matches(&rule.host, host) {
                continue;
            }
            if let Some(methods) = &rule.methods
                && !methods.iter().any(|candidate| candidate == method)
            {
                continue;
            }
            if let Some(paths) = &rule.paths
                && !paths
                    .iter()
                    .any(|candidate| path.starts_with(candidate.as_str()))
            {
                continue;
            }
            return rule.allow;
        }
        true
    }
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
fn host_matches(pattern: &str, name: &str) -> bool {
    pattern == "*"
        || pattern == name
        || name
            .strip_suffix(pattern)
            .is_some_and(|head| head.ends_with('.'))
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclaredServiceConfig {
    pub name: String,
    pub env_var: String,
    pub upstream: String,
    pub block_direct_egress: bool,
    pub llm_dialect: String,
    pub dlp_redact_prompts: bool,
    pub inspect_sse_tool_use: bool,
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct L7HttpConfig {
    pub sinkhole: SinkholeConfig,
    pub protocol_inspection: bool,
    pub declared_services: Vec<DeclaredServiceConfig>,
    pub threat_feeds: Vec<String>,
    pub hash_only_llm_retention: bool,
}

// see: cans/plugins/l7-http.md#Error-and-return-semantics-(l7-http)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L7PolicyError {
    UpstreamTls(String),
}

// see: cans/plugins/l7-http.md#Error-and-return-semantics-(l7-http)
impl L7PolicyError {
    pub fn errno(&self) -> Errno {
        match self {
            Self::UpstreamTls(_) => Errno::Econnrefused,
        }
    }

    // see: cans/plugins/l7-http.md#Error-and-return-semantics-(l7-http)
    pub fn receipt(&self) -> HttpResponse {
        match self {
            Self::UpstreamTls(reason) => HttpResponse {
                status: 502,
                reason: "Bad Gateway".to_string(),
                headers: vec![("content-type".to_string(), "application/json".to_string())],
                body: format!("{{\"error\":\"E_UPSTREAM_TLS\",\"reason\":\"{reason}\"}}")
                    .into_bytes(),
            },
        }
    }
}

// see: cans/plugins/l7-http.md#Service-exports-(l7-http)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpVerdict {
    Allow,
    Deny,
}

// see: cans/plugins/l7-http.md#Service-exports-(l7-http)
#[async_trait]
pub trait HttpPolicyService: Send + Sync {
    async fn verdict(
        &self,
        request: &HttpRequest,
        pid: u32,
        binary: &str,
    ) -> Result<HttpVerdict, L7PolicyError>;
    // see: cans/plugins/l7-http.md#Error-and-return-semantics-(l7-http)
    fn deny_receipt(&self, reason: &str, pid: u32, binary: &str) -> HttpResponse {
        let _ = self;
        synthetic_403_receipt(reason, pid, binary)
    }
}

// see: cans/plugins/l7-http.md#Service-exports-(l7-http)
#[derive(Debug, Clone)]
pub struct HttpPolicy {
    config: L7HttpConfig,
}

// see: cans/plugins/l7-http.md#Service-exports-(l7-http)
impl HttpPolicy {
    pub fn new(config: L7HttpConfig) -> Self {
        Self { config }
    }

    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    pub fn config(&self) -> &L7HttpConfig {
        &self.config
    }

    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    pub fn request_host(request: &HttpRequest) -> &str {
        request.header("host").unwrap_or("")
    }

    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    pub fn request_path(request: &HttpRequest) -> &str {
        request.target.split('?').next().unwrap_or("")
    }

    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    pub fn declared_service(&self, host: &str) -> Option<&DeclaredServiceConfig> {
        self.config.declared_services.iter().find(|service| {
            service
                .upstream
                .split("://")
                .nth(1)
                .unwrap_or(&service.upstream)
                .split('/')
                .next()
                .unwrap_or("")
                == host
        })
    }
}

#[async_trait]
impl HttpPolicyService for HttpPolicy {
    // see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
    async fn verdict(
        &self,
        request: &HttpRequest,
        _pid: u32,
        _binary: &str,
    ) -> Result<HttpVerdict, L7PolicyError> {
        if !self.config.protocol_inspection {
            return Ok(HttpVerdict::Allow);
        }
        let host = Self::request_host(request);
        let path = Self::request_path(request);
        if is_path_traversal(path) {
            return Ok(HttpVerdict::Deny);
        }
        if self.config.sinkhole.verdict(host, &request.method, path) {
            Ok(HttpVerdict::Allow)
        } else {
            Ok(HttpVerdict::Deny)
        }
    }
}

// see: cans/plugins/l7-http.md#Service-exports-(l7-http)
#[derive(Clone, PluginDeps)]
pub struct L7HttpPlugin {
    policy: Arc<HttpPolicy>,
}

impl L7HttpPlugin {
    pub fn new() -> Self {
        Self::with_config(L7HttpConfig {
            sinkhole: SinkholeConfig::default(),
            protocol_inspection: true,
            declared_services: Vec::new(),
            threat_feeds: Vec::new(),
            hash_only_llm_retention: false,
        })
    }

    pub fn with_config(config: L7HttpConfig) -> Self {
        Self {
            policy: Arc::new(HttpPolicy::new(config)),
        }
    }

    pub fn policy(&self) -> &Arc<HttpPolicy> {
        &self.policy
    }
}

impl Default for L7HttpPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for L7HttpPlugin {
    type Context = Arc<HttpPolicy>;
    fn context(&self) -> Self::Context {
        Arc::clone(&self.policy)
    }
    fn name(&self) -> &'static str {
        "l7-http"
    }
    fn default_scope(&self) -> MountScope {
        // see: cans/plugins/l7-http.md#Crate-dependencies-(l7-http)
        MountScope::default().with_devices([VirtioDevice::Net])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        // see: cans/plugins/l7-http.md#Service-exports-(l7-http)
        pipeline.bind_net_scoped(Arc::new(self.policy.as_ref().clone()), self.default_scope());
    }
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
pub fn parser_with_config(config: &L7HttpConfig) -> L7Parser {
    L7Parser::new(config.protocol_inspection)
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
impl TlsDecryptor for HostKeyringDecryptor {
    fn decrypt(&self, _record: &[u8]) -> Result<Vec<u8>, L7ParseError> {
        // TODO: [wire_plane]: fetch the session key from the host keyring and decrypt the TLS record; no keyring read exists yet.; slop: TLS records pass through undecrypted, L7 rules blind (deferred to L3)
        Err(L7ParseError::TlsUnavailable(
 "host keyring tls decrypt is deferred to L3".into(),
        ))
    }
}
