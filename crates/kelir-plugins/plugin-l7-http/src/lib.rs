// see: cans/plugins/l7-http.md#Plugin-l7-http
#![forbid(unsafe_code)]

#[path = "l7_parser.flow.rs"]
pub mod l7_parser;
#[path = "l7_tls.rs"]
pub mod l7_tls;
#[path = "service_l7_http.flow.rs"]
pub mod service_l7_http;
#[path = "service_l7_http.run.rs"]
pub mod service_l7_http_run;

pub use l7_parser::{
    HttpRequest, HttpResponse, JsonRpcEnvelope, L7Message, L7ParseError, L7Parser,
    MAX_L7_BODY_BYTES, SseEvent, content_length, normalize_token_usage, parse_http_request,
    parse_http_response, parse_json_rpc, path_traversal_rejected, sse_chunk_contains_tool_use,
    split_headers, split_sse_frames, synthetic_403_receipt, synthetic_403_receipt_with_guidance,
};
pub use l7_tls::{HostKeyringDecryptor, TlsDecryptor};
pub use service_l7_http::{
    DeclaredServiceConfig, HttpPolicy, HttpPolicyService, HttpVerdict, L7HttpConfig, L7HttpPlugin,
    L7PolicyError, SinkholeConfig, SinkholeRule,
};

// see: cans/plugins/l7-http.md#Crate-dependencies-(l7-http)
pub const L7_HTTP_PLUGIN_NAME: &str = "l7-http";
