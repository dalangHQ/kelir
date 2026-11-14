// see: cans/02-plugin-model.md#Unified-builder
// see: cans/03-sdk-architecture.md#Unified-builder-surface
// see: cans/artifacts/examples/fluent-builder-chaos.ts#buildChaosPlugin
// see: cans/artifacts/examples/fluent-builder-chaos.py#build_chaos_plugin

use std::sync::Arc;

use kelir::plugin_model::{FilterAction, MountScope};
use kelir::{
    ChaosConfig, ChaosPlugin, FaultDirection, FaultInjectionService, HCons, HNil, Host,
    KelirBuilder, LedgerCtx, LedgerPlugin, NetworkFaultRule, NoParams, Plugin, StorageFaultRule,
    Unsealed,
};
use kelir::driver_interface::default_vm_config;
use kelir::protocol_wire::Errno;

// see: cans/02-plugin-model.md#Unified-builder
// The canonical chaos plugin: identical behaviour in all three languages.
// Requires ledger, mount scope restricted to /workspace, nested .fs and
// .net scopes, pass/forward policy. L1/L2: descriptors only, no execution.
// No build(): the plugin-mode builder is mounted as-is, and the host's
// seal() validates it. The concrete KelirBuilder type parameters follow
// the spec arity pin (cans/02-plugin-model.md#Unified-builder, A1).
// .end() is a Rust type-system artifact, not a semantic one: the Rust
// scope closure must return the builder, and .end() is how it does it;
// TS/Py have no .end() (cans/02-plugin-model.md#Chain-decisions (e)).
pub fn build_chaos_plugin(
) -> KelirBuilder<Plugin, Unsealed, (), HCons<LedgerCtx, HNil>, HNil, NoParams> {
    KelirBuilder::named("chaos")
        .require::<LedgerPlugin>()
        .scope(MountScope::default().with_paths(["/workspace"]))
        .fs(|f| f.on_fs_write(|_, _, _, _| Ok(FilterAction::Forward)).end())
        .net(|n| n.on_net_outbound(|_| Ok(FilterAction::Forward)).end())
}

// see: cans/plugins/chaos.md#Network-chaos
// see: cans/plugins/chaos.md#Storage-chaos
pub fn chaos_config() -> ChaosConfig {
    ChaosConfig {
        network: vec![NetworkFaultRule {
            target: "example.invalid".to_string(),
            direction: FaultDirection::Outbound,
            base_ms: 0,
            jitter_ms: 0,
            drop_rate: 0.0,
            inject_rst: true,
            bandwidth_kbps: None,
        }],
        fs: vec![StorageFaultRule {
            target_path: "/workspace/scratch".to_string(),
            simulate_error: Errno::Enospc,
            error_probability: 1.0,
        }],
    }
}

// see: cans/02-plugin-model.md#Unified-builder
pub type ChaosHostBuilder = KelirBuilder<
    Host,
    Unsealed,
    (),
    HNil,
    HCons<Arc<dyn FaultInjectionService>, HNil>,
    NoParams,
>;

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/plugins/chaos.md#Plugin-chaos
pub fn build_chaos_host(config: ChaosConfig) -> ChaosHostBuilder {
    // see: cans/01-architecture.md#Embedded-profile
    KelirBuilder::new(default_vm_config(
        Some(1),
        Some(256),
        Some(format!(
            "./fixtures/vmlinux-6.6-{}-kelir",
            std::env::consts::ARCH
        )),
        Some("./fixtures/rootfs-chaos.erofs".into()),
        Some(true),
        Some(false),
    ))
    .plugin_instance(ChaosPlugin::new(config))
}

// see: cans/02-plugin-model.md#Unified-builder
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _sealed = build_chaos_host(chaos_config()).seal()?;
    let _plugin = build_chaos_plugin();
    Ok(())
}
