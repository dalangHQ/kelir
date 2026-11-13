// see: cans/02-plugin-model.md#Unified-builder
// see: cans/03-sdk-architecture.md#Unified-builder-surface
// see: cans/artifacts/examples/fluent-builder-chaos.rs#build_chaos_plugin
// see: cans/artifacts/examples/fluent-builder-chaos.py#build_chaos_plugin

import {
  chaos,
  createPlugin,
  FilterAction,
  kelir,
  type ChaosConfig,
  type KelirBuilder,
} from "@kelir/core";

// see: cans/02-plugin-model.md#Unified-builder
// The canonical chaos plugin: identical behaviour in all three languages.
// Requires ledger, mount scope restricted to /workspace, nested .fs and
// .net scopes, pass/forward policy. L1/L2: descriptors only, no execution.
// No build(): the plugin-mode builder is mounted as-is, and the host's
// fuse() validates it.
export function buildChaosPlugin(): KelirBuilder<"chaos", unknown, "ledger", unknown> {
  return createPlugin("chaos")
    .requires<"ledger">()
    .scope({ paths: ["/workspace"] })
    .fs((f) =>
      f.onFsWrite((path, buffer, pid, ctx) => {
        void path;
        void buffer;
        void pid;
        void ctx;
        return FilterAction.Forward;
      }),
    )
    .net((n) =>
      n.onNetOutbound((frame) => {
        void frame;
        return FilterAction.Forward;
      }),
    );
}

// see: cans/plugins/chaos.md#Network-chaos
// see: cans/plugins/chaos.md#Storage-chaos
export function chaosConfig(): ChaosConfig {
  return {
    network: [
      {
        target: "example.invalid",
        direction: "outbound",
        baseMs: 0,
        jitterMs: 0,
        dropRate: 0,
        injectRst: true,
      },
    ],
    fs: [{ targetPath: "/workspace/scratch", simulateError: "enospc", errorProbability: 1 }],
  };
}

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/plugins/chaos.md#Plugin-chaos
export function buildChaosHost(config: ChaosConfig) {
  return kelir({
    vcpus: 1,
    memoryMb: 256,
    kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
    rootfsErofs: "./fixtures/rootfs-chaos.erofs",
    tscLocked: true,
    enableWatchdog: false,
    driver: "auto",
  }).use(chaos(config));
}

// see: cans/02-plugin-model.md#Unified-builder
async function main(): Promise<void> {
  buildChaosHost(chaosConfig()).seal();
  buildChaosPlugin();
}

main().catch(console.error);
