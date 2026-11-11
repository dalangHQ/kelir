# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
# see: cans/03-sdk-architecture.md#Fluent-plugin-surface
# see: cans/artifacts/examples/fluent-builder-chaos.rs#build_chaos_plugin
# see: cans/artifacts/examples/fluent-builder-chaos.ts#buildChaosPlugin

from kelir import FilterAction, PluginBuilder
from kelir.plugin_builder import _PluginWrapper


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
# The canonical chaos plugin: identical behaviour in all three languages.
# Requires ledger, mount scope restricted to /workspace, nested .fs and
# .net scopes, pass/forward policy. L1/L2: descriptors only, no execution.
# The .fs() / .net() entry points are the A5 contract (DX-5); they do not
# exist on the live PluginBuilder yet.
def build_chaos_plugin() -> _PluginWrapper:
    return (
        PluginBuilder("chaos")
        .require("ledger")
        .scope({"paths": ["/workspace"]})
        .fs(lambda f: f.on_fs_write(lambda path, data, pid, ctx: FilterAction.forward()))
        .net(lambda n: n.on_net_outbound(lambda frame: FilterAction.forward()))
        .build()
    )
