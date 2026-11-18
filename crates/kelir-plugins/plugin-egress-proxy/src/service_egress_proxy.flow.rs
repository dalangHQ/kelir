// see: cans/plugins/egress-proxy.md#Service-exports-(egress-proxy)
use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::errno_types::Errno;
use kelir_interface::plugin_model::{
    KelirPlugin, MountScope, PluginDeps, PluginError, VirtioDevice,
};
use kelir_interface::wire::l4::L4Stream;

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EgressProxyConfig {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub all_proxy: Option<String>,
    pub no_proxy: Vec<String>,
}

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
impl EgressProxyConfig {
    pub fn from_env() -> Self {
        Self {
            http_proxy: std::env::var("HTTP_PROXY").ok(),
            https_proxy: std::env::var("HTTPS_PROXY").ok(),
            all_proxy: std::env::var("ALL_PROXY").ok(),
            no_proxy: std::env::var("NO_PROXY")
                .map(|value| {
                    value
                        .split(',')
                        .map(|entry| entry.trim().to_string())
                        .filter(|entry| !entry.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    // see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
    pub fn proxy_for(&self, target: &ProxyTarget) -> Option<&str> {
        if self.is_no_proxy(&target.host) {
            return None;
        }
        match target.tls {
            true => self.https_proxy.as_deref().or(self.all_proxy.as_deref()),
            false => self.http_proxy.as_deref().or(self.all_proxy.as_deref()),
        }
    }

    // see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
    pub fn is_no_proxy(&self, host: &str) -> bool {
        self.no_proxy.iter().any(|entry| {
            entry == "*"
                || entry == host
                || host
                    .strip_suffix(entry.as_str())
                    .is_some_and(|head| head.ends_with('.'))
                || entry
                    .strip_prefix('.')
                    .is_some_and(|suffix| host.ends_with(suffix))
        })
    }
}

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyTarget {
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

// see: cans/plugins/egress-proxy.md#Error-and-return-semantics-(egress-proxy)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EgressProxyError {
    ProxyAuth(String),
    SocksMethodRejected(String),
    Dial(String),
}

// see: cans/plugins/egress-proxy.md#Error-and-return-semantics-(egress-proxy)
impl EgressProxyError {
    pub fn errno(&self) -> Errno {
        match self {
            Self::ProxyAuth(_) => Errno::Eperm,
            Self::SocksMethodRejected(_) => Errno::Econnrefused,
            Self::Dial(_) => Errno::Econnrefused,
        }
    }

    pub fn reason(&self) -> &str {
        match self {
            Self::ProxyAuth(reason) | Self::SocksMethodRejected(reason) | Self::Dial(reason) => {
                reason
            }
        }
    }
}

impl std::fmt::Display for EgressProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.errno(), self.reason())
    }
}

impl std::error::Error for EgressProxyError {}

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Socks5Method {
    NoAuth,
    UsernamePassword,
}

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
pub fn build_connect_request(target: &ProxyTarget) -> Vec<u8> {
    format!(
        "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n\r\n",
        target.host, target.port, target.host, target.port
    )
    .into_bytes()
}

// see: cans/plugins/egress-proxy.md#Error-and-return-semantics-(egress-proxy)
pub fn parse_connect_response(response: &[u8]) -> Result<(), EgressProxyError> {
    let head = std::str::from_utf8(response)
        .map_err(|_| EgressProxyError::Dial("proxy CONNECT response is not valid UTF-8".into()))?;
    let status_line = head.lines().next().unwrap_or_default();
    let mut parts = status_line.split_ascii_whitespace();
    let version = parts.next().unwrap_or_default();
    let status = parts.next().unwrap_or_default();
    if !version.starts_with("HTTP/") || status.is_empty() {
        return Err(EgressProxyError::Dial(
            "proxy CONNECT response carries no HTTP status line".into(),
        ));
    }
    match status {
        "200" => Ok(()),
        "407" => Err(EgressProxyError::ProxyAuth(
            "upstream proxy auth failure: 407 passthrough to guest".into(),
        )),
        other => Err(EgressProxyError::Dial(format!(
            "proxy CONNECT rejected with HTTP status {other}"
        ))),
    }
}

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
pub fn build_socks5_greeting(methods: &[Socks5Method]) -> Vec<u8> {
    let mut request = vec![0x05, methods.len() as u8];
    for method in methods {
        request.push(match method {
            Socks5Method::NoAuth => 0x00,
            Socks5Method::UsernamePassword => 0x02,
        });
    }
    request
}

// see: cans/plugins/egress-proxy.md#Error-and-return-semantics-(egress-proxy)
pub fn parse_socks5_method_selection(response: &[u8]) -> Result<Socks5Method, EgressProxyError> {
    if response.len() < 2 || response[0] != 0x05 {
        return Err(EgressProxyError::SocksMethodRejected(
            "SOCKS5 method selection is not a version 5 reply".into(),
        ));
    }
    match response[1] {
        0x00 => Ok(Socks5Method::NoAuth),
        0x02 => Ok(Socks5Method::UsernamePassword),
        0xff => Err(EgressProxyError::SocksMethodRejected(
            "SOCKS5 proxy accepts no offered method".into(),
        )),
        other => Err(EgressProxyError::SocksMethodRejected(format!(
            "SOCKS5 proxy selected unknown method {other}"
        ))),
    }
}

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
pub fn build_socks5_connect_request(target: &ProxyTarget) -> Vec<u8> {
    let mut request = vec![0x05, 0x01, 0x00, 0x03, target.host.len() as u8];
    request.extend_from_slice(target.host.as_bytes());
    request.extend_from_slice(&target.port.to_be_bytes());
    request
}

// see: cans/plugins/egress-proxy.md#Service-exports-(egress-proxy)
#[async_trait]
pub trait EgressProxyService: Send + Sync {
    async fn dial(&self, target: &ProxyTarget) -> Result<L4Stream, EgressProxyError>;
}

// see: cans/plugins/egress-proxy.md#Service-exports-(egress-proxy)
#[derive(Debug, Clone)]
pub struct EgressProxy {
    config: EgressProxyConfig,
}

// see: cans/plugins/egress-proxy.md#Service-exports-(egress-proxy)
impl EgressProxy {
    pub fn new(config: EgressProxyConfig) -> Self {
        Self { config }
    }

    // see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
    pub fn config(&self) -> &EgressProxyConfig {
        &self.config
    }
}

#[async_trait]
impl EgressProxyService for EgressProxy {
    // see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
    async fn dial(&self, target: &ProxyTarget) -> Result<L4Stream, EgressProxyError> {
        let _ = self.config.proxy_for(target);
        // TODO (L3) [wp_13_upstream_dial_proxy_chain_auth]: open the raw L4 splice to the selected upstream proxy, run the CONNECT/SOCKS5 handshake over it, and return the spliced stream; no upstream dial exists yet; slop: guest performs the CONNECT handshake instead of the host netstack.
        Err(EgressProxyError::Dial(
            "egress proxy upstream dial is TODO (L3)".into(),
        ))
    }
}

// see: cans/plugins/egress-proxy.md#Plugin-egress-proxy
#[derive(Clone, PluginDeps)]
pub struct EgressProxyPlugin {
    proxy: Arc<EgressProxy>,
}

impl EgressProxyPlugin {
    pub fn new() -> Self {
        Self::with_config(EgressProxyConfig::from_env())
    }

    pub fn with_config(config: EgressProxyConfig) -> Self {
        Self {
            proxy: Arc::new(EgressProxy::new(config)),
        }
    }

    pub fn proxy(&self) -> &Arc<EgressProxy> {
        &self.proxy
    }
}

impl Default for EgressProxyPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for EgressProxyPlugin {
    type Context = Arc<EgressProxy>;
    fn context(&self) -> Self::Context {
        Arc::clone(&self.proxy)
    }
    fn name(&self) -> &'static str {
        "egress-proxy"
    }
    fn default_scope(&self) -> MountScope {
        // see: cans/plugins/egress-proxy.md#Crate-dependencies-(egress-proxy)
        MountScope::default().with_devices([VirtioDevice::Net])
    }
    // TODO (L3) [wp_13_upstream_dial_proxy_chain_auth]: bind EgressProxyService to the wire plane L4 dialer once the plane exposes a plugin-owned dial hook; slop: egress proxy never binds to the L4 dialer; upstream dials never leave the host netstack.
    fn register_interceptors(
        &self,
        _pipeline: &mut dyn kelir_interface::plugin_model::PipelineBinder,
    ) {
    }
}

// see: cans/plugins/egress-proxy.md#Service-exports-(egress-proxy)
impl From<EgressProxyError> for PluginError {
    fn from(error: EgressProxyError) -> Self {
        PluginError::Halt(error.to_string())
    }
}
