// see: cans/02-plugin-model.md#Facade-blindness
#![forbid(unsafe_code)]

// see: cans/02-plugin-model.md#Facade-blindness
pub use kelir;
pub use kelir::interface;

// see: cans/plugins/anomaly.md#Plugin-anomaly
pub use plugin_anomaly::{
    AnomalyConfig, AnomalyCtx, AnomalyDetectionService, AnomalyHandle, AnomalyPlugin,
    FeatureVector, TelemetryRing, TelemetryRingConfig, TelemetrySample,
};
// see: cans/plugins/chaos.md#Plugin-chaos
pub use plugin_chaos::{
    ChaosConfig, ChaosPlugin, FaultDirection, FaultInjectionService, NetworkFaultRule,
    StorageFaultRule,
};
pub use plugin_container::{ContainerConfig, ContainerCtx, ContainerHandle, ContainerPlugin};
pub use plugin_db_proxy::{
    DbProxyConfig, DbProxyPlugin, EphemeralCredentials, NetL7SqlExt, PgSchemaRewrite,
    PgWireRewrite, SqlProxyCtx, SqlProxyHandle, SqlReceiptExt,
};
// see: cans/plugins/egress-proxy.md#Plugin-egress-proxy
pub use plugin_egress_proxy::{EgressProxyConfig, EgressProxyPlugin};
// see: cans/plugins/l7-http.md#Plugin-l7-http
pub use plugin_l7_http::{L7HttpConfig, L7HttpPlugin};
// see: cans/plugins/ledger.md#Plugin-ledger
pub use plugin_ledger::{
    DualHash, FinalReceipt, FinalizeRequest, HashingStandard, LedgerConfig, LedgerCtx,
    LedgerHandle, LedgerPlugin, LedgerSink, MerkleChain, MerkleWalConfig, OtelConfig,
    ReceiptConfig, SignedTreeHead, WitnessClient, WitnessCosignature,
};
pub use plugin_mcp::{McpConfig, McpCtx, McpEndpointService, McpPlugin};
// see: cans/plugins/mesh.md#Plugin-mesh
pub use plugin_mesh::{
    MeshConfig, MeshCtx, MeshHandle, MeshPlugin, PeerIdentity, PeerListener,
    TraceParentPropagation, VectorClock,
};
pub use plugin_profile::{ProfileConfig, ProfileCtx, ProfilePlugin};
// see: cans/plugins/redaction.md#Plugin-redaction
pub use plugin_redaction::{MaskReadConfig, RedactionConfig, RedactionPlugin};
// see: cans/plugins/replay.md#Plugin-replay
pub use plugin_replay::{ReplayConfig, ReplayCtx, ReplayPlugin};
// see: cans/plugins/step-eval.md#Plugin-step-eval
pub use plugin_step_eval::{
    PostExecAction, StepEvalConfig, StepEvalPlugin, StepEvaluationCtx, StepEvaluationService,
    StepEvaluator, StepVerdict,
};
// see: cans/plugins/supply-chain.md#Plugin-supply-chain
pub use plugin_supply_chain::{
    AuthorizedJitEngine, JitValidationService, PackageGuardConfig, SupplyChainConfig,
    SupplyChainCtx, SupplyChainPlugin, WxJitConfig,
};
// see: cans/plugins/taint.md#Plugin-taint
pub use plugin_taint::{TaintPlugin, TaintTrackingService};
// see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
pub use plugin_terminal_guard::{TerminalGuardConfig, TerminalGuardPlugin};
// see: cans/plugins/token-shield.md#Plugin-token-shield
pub use plugin_token_shield::{TokenShieldConfig, TokenShieldPlugin};
pub use plugin_tool_guard::{ToolGuardConfig, ToolGuardCtx, ToolGuardPlugin, ToolSpec};
pub use plugin_transaction::{
    TransactionConfig, TransactionCtx, TransactionHandle, TransactionPlugin, Tx,
};
pub use plugin_vault::{CredentialBrokerCtx, CredentialDescriptor, VaultConfig, VaultPlugin};
pub use plugin_wire_ingress::{
    IngressRoutingService, SpliceRoute, WireIngressConfig, WireIngressPlugin,
};

// see: cans/02-plugin-model.md#Facade-blindness
#[path = "plugin_profile_schema.flow.rs"]
pub mod plugin_profile_schema;
// see: cans/02-plugin-model.md#Facade-blindness
pub use plugin_profile_schema::{
    LedgerPluginProfile, McpInterpositionProfile, McpPluginProfile, MerkleWalProfile, OtelProfile,
    PluginsProfile, TaintGovernanceProfile, ToolGuardPluginProfile,
};

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
pub mod tool_guard_ext {
    use kelir::hlist_sdk::HList;
    use kelir::{Host, KelirBuilder, Unsealed};
    use plugin_tool_guard::{ToolGuardPlugin, ToolSpec};

    // see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
    pub trait ToolGuardExt: Sized {
        fn tool(self, spec: ToolSpec) -> Self;
    }

    // see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
    impl<Plugins: HList, Params: 'static, Ctx, Deps: HList> ToolGuardExt
        for KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params>
    {
        fn tool(self, spec: ToolSpec) -> Self {
            let plugin = ToolGuardPlugin::new().tool(spec);
            self.mount_instance(plugin)
        }
    }
}

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
pub use tool_guard_ext::ToolGuardExt;

// see: cans/plugins/ledger.md#Canonical-accessor-(SSOT-A1-pin)
pub mod ledger_ext {
    use kelir::hlist_sdk::{HList, NoParams};
    use kelir::interface::handlers_plugin::Capability;
    use kelir::interface::plugin_model::PluginError;
    use kelir::session_lifecycle::Session;
    use plugin_ledger::LedgerCtx;

    // see: cans/plugins/ledger.md#Canonical-accessor-(SSOT-A1-pin)
    pub trait LedgerSessionExt: Sized {
        // see: cans/plugins/ledger.md#Causal-delta-access-(ledger)
        async fn get_causal_delta_from_parent(
            &self,
        ) -> Result<kelir::interface::telemetry::CausalDelta, PluginError>;

        // see: cans/plugins/ledger.md#Ledger-handler-API
        async fn count_denials(&self, ppid: u32) -> Result<u64, PluginError>;
    }

    // see: cans/plugins/ledger.md#Canonical-accessor-(SSOT-A1-pin)
    impl<Plugins, Params> LedgerSessionExt for Session<Plugins, Params>
    where
        Plugins: HList + Capability<LedgerCtx>,
    {
        // see: cans/plugins/ledger.md#Causal-delta-access-(ledger)
        async fn get_causal_delta_from_parent(
            &self,
        ) -> Result<kelir::interface::telemetry::CausalDelta, PluginError> {
            let ledger = self.cap::<LedgerCtx>().ok_or_else(|| {
                PluginError::Halt("ECONFIG: ledger capability not mounted".into())
            })?;
            ledger.get_causal_delta_from_parent().await
        }

        // see: cans/plugins/ledger.md#Ledger-handler-API
        async fn count_denials(&self, ppid: u32) -> Result<u64, PluginError> {
            let ledger = self.cap::<LedgerCtx>().ok_or_else(|| {
                PluginError::Halt("ECONFIG: ledger capability not mounted".into())
            })?;
            ledger.count_denials(ppid).await
        }
    }
}
