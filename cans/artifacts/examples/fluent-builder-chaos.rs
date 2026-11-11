// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/03-sdk-architecture.md#Fluent-plugin-surface
// see: cans/artifacts/examples/fluent-builder-chaos.ts#buildChaosPlugin
// see: cans/artifacts/examples/fluent-builder-chaos.py#build_chaos_plugin

use kelir::hlist_sdk::{HCons, HNil};
use kelir::plugin_model::{FilterAction, MountScope, PluginError};
use kelir::{LedgerCtx, LedgerPlugin, PluginBuilder, PluginWrapper};

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// The canonical chaos plugin: identical behaviour in all three languages.
// Requires ledger, mount scope restricted to /workspace, nested .fs and
// .net scopes, pass/forward policy. L1/L2: descriptors only, no execution.
// The .fs() / .net() entry points and the Result build are the A3 contract
// (DX-3); they do not exist on the live builder yet.
pub fn build_chaos_plugin() -> Result<PluginWrapper<(), HCons<LedgerCtx, HNil>>, PluginError> {
    PluginBuilder::new("chaos")
        .require::<LedgerPlugin>()
        .scope(MountScope::default().with_paths(["/workspace"]))
        .fs(|f| f.on_fs_write(|_, _, _, _| Ok(FilterAction::Forward)))
        .net(|n| n.on_net_outbound(|_| Ok(FilterAction::Forward)))
        .build()
}
