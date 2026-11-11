# see: cans/02-plugin-model.md#Unified-builder
# see: cans/03-sdk-architecture.md#Unified-builder-surface
# see: cans/artifacts/examples/fluent-builder-chaos.rs#build_chaos_plugin
# see: cans/artifacts/examples/fluent-builder-chaos.ts#buildChaosPlugin

from kelir import FilterAction, KelirBuilder


# see: cans/02-plugin-model.md#Unified-builder
# The canonical chaos plugin: identical behaviour in all three languages.
# Requires ledger, mount scope restricted to /workspace, nested .fs and
# .net scopes, pass/forward policy. L1/L2: descriptors only, no execution.
# No build(): the plugin-mode builder is mounted as-is, and the host's
# boot() validates it.
def build_chaos_plugin() -> KelirBuilder:
    return (
        KelirBuilder.plugin("chaos")
        .require("ledger")
        .scope({"paths": ["/workspace"]})
        .fs(lambda f: f.on_fs_write(lambda path, data, pid, ctx: FilterAction.forward()))
        .net(lambda n: n.on_net_outbound(lambda frame: FilterAction.forward()))
    )
