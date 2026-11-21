// see: cans/plugins/supply-chain.md#Plugin-supply-chain
use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::driver_interface::Stream;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{HandlerCtx, KelirPlugin, PipelineBinder, PluginDeps};
use kelir_interface::vsock_mesh::{VsockPeer, VsockServiceHandler, VsockServiceSpec};

use crate::wx_token::{WxAllocToken, WxTokenChannel, WxTokenLedger};
use crate::{JitValidationService, PackageGuardConfig, SupplyChainConfig, WX_ALLOC_VSOCK_PORT};

impl Default for PackageGuardConfig {
    fn default() -> Self {
        Self {
            deny_lifecycle_scripts: true,
            managers: vec!["npm".into(), "pip".into(), "cargo".into()],
            bypass_runners: vec!["node".into(), "python".into()],
            resolve_lockfiles: true,
            advisories: vec![
                "osv".into(),
                "snyk".into(),
                "socket".into(),
                "deps.dev".into(),
            ],
            strip_private_scopes: true,
            vet_skills: true,
        }
    }
}

// see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
struct SupplyChainState {
    config: SupplyChainConfig,
    ledger: WxTokenLedger,
    channel: WxTokenChannel,
}

impl SupplyChainState {
    fn new(config: SupplyChainConfig) -> Self {
        Self {
            config,
            ledger: WxTokenLedger::new(),
            channel: WxTokenChannel::new(),
        }
    }

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    fn is_authorized(&self, binary_blake3: &[u8; 32]) -> bool {
        // L3-GAP: [supply_chain_security]: catalog storage internals; resolve allowed_jit_binaries disk paths to BLAKE3 digests for the verified JIT engine catalog (Node, Bun, V8); slop: paths resolved by file name, swapped binary passes catalog
        self.config
            .wx_jit
            .allowed_jit_binaries
            .iter()
            .any(|engine| engine.binary_blake3 == *binary_blake3)
    }
}

// see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
#[derive(Clone)]
pub struct SupplyChainCtx {
    state: Arc<SupplyChainState>,
}

impl SupplyChainCtx {
    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    pub fn consume_wx_token(&self, nonce: u128) -> Result<WxAllocToken, PluginError> {
        self.state.ledger.consume(nonce)
    }
}

// see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
impl JitValidationService for SupplyChainCtx {
    fn is_authorized(&self, binary_blake3: &[u8; 32]) -> bool {
        self.state.is_authorized(binary_blake3)
    }
}

#[derive(Clone, PluginDeps)]
pub struct SupplyChainPlugin {
    state: Arc<SupplyChainState>,
}

impl SupplyChainPlugin {
    pub fn new(config: SupplyChainConfig) -> Self {
        Self {
            state: Arc::new(SupplyChainState::new(config)),
        }
    }

    // see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
    pub fn ctx(&self) -> SupplyChainCtx {
        SupplyChainCtx {
            state: Arc::clone(&self.state),
        }
    }
}

impl Default for SupplyChainPlugin {
    fn default() -> Self {
        Self::new(SupplyChainConfig::default())
    }
}

// see: cans/plugins/supply-chain.md#Plugin-supply-chain
impl KelirPlugin for SupplyChainPlugin {
    // see: cans/plugins/supply-chain.md#Crate-dependencies-(supply-chain)
    type Context = SupplyChainCtx;
    fn context(&self) -> Self::Context {
        self.ctx()
    }
    fn name(&self) -> &'static str {
        "supply-chain"
    }
    // see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
    fn fork_state(
        &self,
        _child_session_id: &str,
    ) -> Box<dyn kelir_interface::plugin_model::AnyPlugin> {
        // see: cans/08-lifecycle.md#CoW-fork
        Box::new(Self::new(self.state.config.clone()))
    }

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline.bind_trap_scoped(Arc::new(self.clone()), self.default_scope());
        let service: Arc<dyn VsockServiceHandler> = Arc::new(WxTokenVsockService {
            state: Arc::clone(&self.state),
        });
        pipeline.bind_vsock_service_scoped(
            VsockServiceSpec {
                port: WX_ALLOC_VSOCK_PORT,
                service: "wx-alloc".into(),
                schema: None,
            },
            service,
            self.default_scope(),
        );
    }
}

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
struct WxTokenVsockService {
    state: Arc<SupplyChainState>,
}

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
#[async_trait]
impl VsockServiceHandler for WxTokenVsockService {
    async fn on_connect(
        &self,
        peer: &VsockPeer,
        stream: Box<dyn Stream>,
        _ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        // L3-GAP: [supply_chain_security]: read the guest shim handshake on this channel without blocking the async worker; slop: shim handshake read blocks worker, async loop stalls
        self.state.channel.register(peer.clone(), stream)
    }
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
