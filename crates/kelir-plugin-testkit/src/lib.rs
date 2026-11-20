// see: cans/02-plugin-model.md#Plugin-testkit
#![forbid(unsafe_code)]

#[path = "assertions_testkit.rs"]
pub mod assertions_contract;
#[path = "assertions_testkit.flow.rs"]
pub mod assertions_testkit;
pub mod fixtures_testkit;
#[path = "handlers_testkit.flow.rs"]
pub mod handlers_testkit;
#[path = "loopback_stream.flow.rs"]
pub mod loopback_stream;
#[path = "vectors_parse.flow.rs"]
pub mod vectors_parse;
#[path = "vectors_scope_cases.flow.rs"]
pub mod vectors_scope_cases;
#[path = "vectors_testkit.flow.rs"]
pub mod vectors_testkit;

pub use loopback_stream::block_on;

use std::collections::VecDeque;

// see: cans/01-architecture.md#Transport-engine-abstraction
#[derive(Debug)]
pub struct LoopbackStream {
    channel_id: u32,
    buffer: VecDeque<u8>,
}
