// see: cans/02-plugin-model.md#Plugin-testkit
#![forbid(unsafe_code)]

pub mod assertions_testkit;
pub mod fixtures_testkit;
pub mod handlers_testkit;
pub mod vectors_chain_testkit;
pub mod vectors_parse;
pub mod vectors_testkit;

pub use kelir_interface::runtime_block::block_on;

use std::collections::VecDeque;

use kelir_interface::driver_interface::Stream;

// see: cans/01-architecture.md#Transport-engine-abstraction
#[derive(Debug)]
pub struct LoopbackStream {
    channel_id: u32,
    buffer: VecDeque<u8>,
}

impl LoopbackStream {
    pub fn new(channel_id: u32) -> Self {
        Self {
            channel_id,
            buffer: VecDeque::new(),
        }
    }

    pub fn channel_id(&self) -> u32 {
        self.channel_id
    }
}

impl Stream for LoopbackStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let n = buf.len().min(self.buffer.len());
        let bytes: Vec<u8> = self.buffer.drain(..n).collect();
        buf[..n].copy_from_slice(&bytes);
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.buffer.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}
