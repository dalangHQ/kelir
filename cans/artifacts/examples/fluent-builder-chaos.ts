// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/03-sdk-architecture.md#Fluent-plugin-surface
// see: cans/artifacts/examples/fluent-builder-chaos.rs#build_chaos_plugin
// see: cans/artifacts/examples/fluent-builder-chaos.py#build_chaos_plugin

import { createPlugin, FilterAction, type KelirPlugin } from "@kelir/core";

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// The canonical chaos plugin: identical behaviour in all three languages.
// Requires ledger, mount scope restricted to /workspace, nested .fs and
// .net scopes, pass/forward policy. L1/L2: descriptors only, no execution.
// The .fs() / .net() entry points are the A4 contract (DX-4); they do not
// exist on the live PluginContract yet.
export function buildChaosPlugin(): KelirPlugin {
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
    )
    .build();
}
