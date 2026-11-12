// see: cans/02-plugin-model.md#Unified-builder
// see: cans/03-sdk-architecture.md#Unified-builder-surface
// see: cans/artifacts/examples/fluent-builder-chaos.ts#buildChaosPlugin
// see: cans/artifacts/examples/fluent-builder-chaos.py#build_chaos_plugin

use kelir::plugin_model::{FilterAction, MountScope};
use kelir::{HCons, HNil, KelirBuilder, LedgerCtx, LedgerPlugin, NoParams, Plugin, Unsealed};

// see: cans/02-plugin-model.md#Unified-builder
// The canonical chaos plugin: identical behaviour in all three languages.
// Requires ledger, mount scope restricted to /workspace, nested .fs and
// .net scopes, pass/forward policy. L1/L2: descriptors only, no execution.
// No build(): the plugin-mode builder is mounted as-is, and the host's
// seal() validates it. The concrete KelirBuilder type parameters follow
// the spec arity pin (cans/02-plugin-model.md#Unified-builder, A1).
pub fn build_chaos_plugin(
) -> KelirBuilder<Plugin, Unsealed, (), HCons<LedgerCtx, HNil>, HNil, NoParams> {
    KelirBuilder::named("chaos")
        .require::<LedgerPlugin>()
        .scope(MountScope::default().with_paths(["/workspace"]))
        .fs(|f| f.on_fs_write(|_, _, _, _| Ok(FilterAction::Forward)).end())
        .net(|n| n.on_net_outbound(|_| Ok(FilterAction::Forward)).end())
}
