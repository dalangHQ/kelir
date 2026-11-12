// see: cans/artifacts/test-plan/cases/bindings_ts/bt_04_napi_boot_before_fuse_hooks_fails_closed.yaml#case_id
import test from "node:test";

// see: cans/artifacts/test-plan/cases/bindings_ts/bt_04_napi_boot_before_fuse_hooks_fails_closed.yaml#case_id
interface NativeKelirOptionsBinding {
  readonly vcpus?: number;
  readonly memoryMb?: number;
  readonly kernel?: string;
  readonly rootfsErofs?: string;
  readonly tscLocked?: boolean;
  readonly enableWatchdog?: boolean;
  readonly driver?: string;
}

interface NativeBuilderBinding {
  readonly trampoline_entry_count: () => number;
  readonly boot: () => Promise<unknown>;
}

async function bt_04_napi_boot_before_fuse_hooks_fails_closed(): Promise<void> {
  let binding: {
    readonly NativeKelirBuilder: new (
      options?: NativeKelirOptionsBinding,
    ) => NativeBuilderBinding;
  };
  try {
    binding = (await import("../../dist/native/kelir.node")) as {
      readonly NativeKelirBuilder: new (
        options?: NativeKelirOptionsBinding,
      ) => NativeBuilderBinding;
    };
  } catch {
    throw new Error(
      "bt_04: the precompiled NAPI addon is not shipped at dist/native/kelir.node, so an unfused boot never reaches the native 'boot: fuse_hooks must run before boot' rejection",
    );
  }
  const nativeBuilder = new binding.NativeKelirBuilder();
  if (nativeBuilder.trampoline_entry_count() !== 0) {
    throw new Error(
      "bt_04: a never-fused NativeKelirBuilder already carries trampoline entries",
    );
  }
  try {
    await nativeBuilder.boot();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (message.includes("fuse_hooks must run before boot")) {
      return;
    }
    throw new Error(
      `bt_04: unfused boot failed closed with the wrong reason: ${message}`,
    );
  }
  throw new Error(
    "bt_04: unfused boot completed instead of failing closed before vCPU launch",
  );
}

test(
  "bt_04_napi_boot_before_fuse_hooks_fails_closed",
  bt_04_napi_boot_before_fuse_hooks_fails_closed,
);
