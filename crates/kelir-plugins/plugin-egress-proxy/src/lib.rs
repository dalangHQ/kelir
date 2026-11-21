// see: cans/plugins/egress-proxy.md#Plugin-egress-proxy
#![forbid(unsafe_code)]

#[path = "service_egress_proxy.flow.rs"]
pub mod service_egress_proxy;

pub use service_egress_proxy::{
    EgressProxy, EgressProxyConfig, EgressProxyError, EgressProxyPlugin, EgressProxyService,
    ProxyTarget, SOCKS5_CMD_RESOLVE, Socks5Method, build_connect_request,
    build_socks5_connect_request, build_socks5_greeting, is_onion_host, parse_connect_response,
    parse_socks5_method_selection, socks5_request_command, validate_proxy_target,
    validate_socks5_request,
};

// see: cans/plugins/egress-proxy.md#Crate-dependencies-(egress-proxy)
pub const EGRESS_PROXY_PLUGIN_NAME: &str = "egress-proxy";
