// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type { NativeAddonBinding } from "./fusion.plugin.js";
import { NativeAddonMissingError } from "./fusion.plugin.flow.js";

// TODO (L3) [bindings_ts]: import the precompiled NAPI addon once it ships next to the dist bundle; slop: addon never imported, fused plugin calls throw instead of dispatching.
export async function loadNativeAddon(): Promise<NativeAddonBinding> {
  throw new NativeAddonMissingError(
    "kelir native NAPI addon is missing: install the precompiled @kelir/core native build (L3)",
  );
}
