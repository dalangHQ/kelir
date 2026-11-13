# see: cans/02-plugin-model.md#Unified-builder
# see: cans/03-sdk-architecture.md#Unified-builder-surface
# see: cans/artifacts/examples/fluent-builder-chaos.rs#build_chaos_plugin
# see: cans/artifacts/examples/fluent-builder-chaos.ts#buildChaosPlugin

import asyncio
import platform

import kelir
from kelir import FilterAction, KelirBuilder
from kelir.plugins.plugin_chaos import (
    ChaosConfig,
    NetworkFaultRule,
    StorageFaultRule,
    chaos,
)


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


# see: cans/plugins/chaos.md#Network-chaos
# see: cans/plugins/chaos.md#Storage-chaos
def chaos_config() -> ChaosConfig:
    return ChaosConfig(
        network=[
            NetworkFaultRule(
                target="example.invalid",
                direction="outbound",
                base_ms=0,
                jitter_ms=0,
                drop_rate=0.0,
                inject_rst=True,
            )
        ],
        fs=[
            StorageFaultRule(
                target_path="/workspace/scratch",
                simulate_error="enospc",
                error_probability=1.0,
            )
        ],
    )


# see: cans/02-plugin-model.md#Unified-builder
# see: cans/plugins/chaos.md#Plugin-chaos
def build_chaos_host(config: ChaosConfig) -> KelirBuilder:
    return (
        kelir.builder(
            vcpus=1,
            memory_mb=256,
            kernel=f"./fixtures/vmlinux-6.6-{platform.machine()}-kelir",
            rootfs_erofs="./fixtures/rootfs-chaos.erofs",
            tsc_locked=True,
            enable_watchdog=False,
            driver="auto",
        ).use(chaos(config))
    )


# see: cans/02-plugin-model.md#Unified-builder
async def main() -> None:
    build_chaos_host(chaos_config()).seal()
    build_chaos_plugin()


if __name__ == "__main__":
    asyncio.run(main())
