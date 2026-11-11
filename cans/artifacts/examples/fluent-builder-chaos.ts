// see: cans/02-plugin-model.md#Unified-builder
// see: cans/03-sdk-architecture.md#Unified-builder-surface
// see: cans/artifacts/examples/fluent-builder-chaos.rs#build_chaos_plugin
// see: cans/artifacts/examples/fluent-builder-chaos.py#build_chaos_plugin

import { createPlugin, FilterAction, type KelirBuilder } from "@kelir/core";

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
