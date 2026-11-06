use std::path::Path;

use kelir_interface::plugin::PluginError;
use kelir_interface::telemetry::CausalDelta;

use crate::plugin::KelirPlugin;

#[derive(Debug, Clone)]
pub struct OtelConfig {
    pub endpoint: String,
    pub service_name: String,
    pub propagate_traceparent: bool,
}

#[derive(Debug, Clone)]
pub struct ReceiptConfig {
    pub include_syscall_count: bool,
}

#[derive(Debug, Clone)]
pub struct LedgerConfig {
    pub store: String,
    pub merkle_chain: bool,
    pub otel: Option<OtelConfig>,
    pub receipts: ReceiptConfig,
}

#[derive(Debug, Clone)]
pub struct FinalReceipt {
    pub merkle_root: String,
    pub signed_jws: String,
}

#[derive(Debug, Clone)]
pub struct FinalizeRequest {
    pub include_delta: Option<CausalDelta>,
    pub include_merkle_root: bool,
}

#[derive(Debug, Clone, Default)]
pub struct LedgerPlugin;

impl LedgerPlugin {
    pub fn new(_config: LedgerConfig) -> Self {
        Self
    }
}

impl KelirPlugin for LedgerPlugin {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn name(&self) -> &'static str {
        "ledger"
    }
}

#[derive(Debug, Clone, Default)]
pub struct LedgerCtx;

impl LedgerCtx {
    pub async fn flush(&self) -> Result<(), PluginError> {
        Ok(())
    }

    pub async fn count_denials(&self, _pid: u32) -> usize {
        0
    }

    pub async fn count_by_kind(&self, _kind: &str) -> Result<usize, PluginError> {
        Ok(0)
    }

    pub async fn finalize(&self, _req: FinalizeRequest) -> Result<FinalReceipt, PluginError> {
        Ok(FinalReceipt {
            merkle_root: String::new(),
            signed_jws: String::new(),
        })
    }

    pub async fn blake3_of_producing_process(&self, _path: &Path) -> Result<String, PluginError> {
        Ok(String::new())
    }
}
