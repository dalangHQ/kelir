// see: cans/plugins/egress-proxy.md#Service-exports-(egress-proxy)
use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::errno_types::Errno;
use kelir_interface::plugin_model::{
    KelirPlugin, MountScope, PluginDeps, PluginError, VirtioDevice,
};
use kelir_interface::wire::l4::L4Stream;

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressProxyConfig {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub all_proxy: Option<String>,
    pub no_proxy: Vec<String>,
    pub block_tor: bool,
    pub deny_socks5_resolve: bool,
}

impl Default for EgressProxyConfig {
    fn default() -> Self {
        Self {
            http_proxy: None,
            https_proxy: None,
            all_proxy: None,
            no_proxy: Vec::new(),
            block_tor: true,
            deny_socks5_resolve: true,
        }
    }
}

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
fn env_var_both(lower: &str, upper: &str) -> Option<String> {
    std::env::var(upper)
        .ok()
        .or_else(|| std::env::var(lower).ok())
}

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
impl EgressProxyConfig {
    pub fn from_env() -> Self {
        Self {
            http_proxy: env_var_both("http_proxy", "HTTP_PROXY"),
            https_proxy: env_var_both("https_proxy", "HTTPS_PROXY"),
            all_proxy: env_var_both("all_proxy", "ALL_PROXY"),
            no_proxy: env_var_both("no_proxy", "NO_PROXY")
                .map(|value| {
                    value
                        .split(',')
                        .map(|entry| entry.trim().to_string())
                        .filter(|entry| !entry.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            block_tor: true,
            deny_socks5_resolve: true,
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

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
pub const SOCKS5_CMD_RESOLVE: u8 = 0xF0;

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
pub fn is_onion_host(host: &str) -> bool {
    host.to_ascii_lowercase().ends_with(".onion")
}

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
pub fn validate_proxy_target(
    config: &EgressProxyConfig,
    target: &ProxyTarget,
) -> Result<(), EgressProxyError> {
    if config.block_tor && is_onion_host(&target.host) {
        return Err(EgressProxyError::Dial(format!(
            "egress proxy blocks Tor .onion target {}",
            target.host
        )));
    }
    Ok(())
}

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
pub fn socks5_request_command(request: &[u8]) -> Option<u8> {
    if request.len() > 1 && request[0] == 0x05 {
        Some(request[1])
    } else {
        None
    }
}

// see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
pub fn validate_socks5_request(
    config: &EgressProxyConfig,
    request: &[u8],
) -> Result<(), EgressProxyError> {
    if config.deny_socks5_resolve && socks5_request_command(request) == Some(SOCKS5_CMD_RESOLVE) {
        return Err(EgressProxyError::SocksMethodRejected(
            "SOCKS5 RESOLVE (0xF0) denied by deny_socks5_resolve".into(),
        ));
    }
    Ok(())
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

    // see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
    pub fn guard_no_proxy(&self, host: &str) -> Result<(), EgressProxyError> {
        let _ = host;
        // TODO: [wire_plane]: guard NO_PROXY against L4 spoofing so a spoofed bypass entry cannot route a proxied host direct; slop: a spoofed NO_PROXY entry bypasses the proxy, direct egress escapes policy. (deferred to L3)
        Err(EgressProxyError::Dial(
            "egress proxy NO_PROXY spoof guard is deferred to L3".into(),
        ))
    }
}

#[async_trait]
impl EgressProxyService for EgressProxy {
    // see: cans/plugins/egress-proxy.md#Invariants-and-policy-(egress-proxy)
    async fn dial(&self, target: &ProxyTarget) -> Result<L4Stream, EgressProxyError> {
        validate_proxy_target(&self.config, target)?;
        let _ = self.config.proxy_for(target);
        // TODO: [wp_13_upstream_dial_proxy_chain_auth]: open the raw L4 splice to the selected upstream proxy, run the CONNECT/SOCKS5 handshake over it, and return the spliced stream; no upstream dial exists yet; slop: guest performs the CONNECT handshake instead of the host netstack. (deferred to L3)
        // TODO: [wire_plane]: trap obfs4proxy/snowflake-client bridge dials before they leave the host; slop: a Tor pluggable transport dials out unexamined, Tor traffic escapes the egress proxy. (deferred to L3)
        Err(EgressProxyError::Dial(
            "egress proxy upstream dial is deferred to L3".into(),
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
    // TODO: [wp_13_upstream_dial_proxy_chain_auth]: bind EgressProxyService to the wire plane L4 dialer once the plane exposes a plugin-owned dial hook; slop: egress proxy never binds to the L4 dialer; upstream dials never leave the host netstack. (deferred to L3)
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
