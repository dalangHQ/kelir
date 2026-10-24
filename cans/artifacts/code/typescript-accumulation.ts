// ILLUSTRATIVE EXCERPT — not standalone-executable.
//
// Demonstrates TypeScript SDK's two-phase builder pattern:
// Plugins register via .use(); .configure() receives the fully unified TContext.
// SyntheticFile: FUSE-presented paths with no host backing file.
// Verification Reject: c.deny returns a POSIX errno.
//
// Full runnable example: artifacts/examples/autonomous-swe-agent.ts
// Canonical spec home:    cans/03-sdk-architecture.md#TypeScript-SDK

import { kelir, createPlugin, t } from "@kelir/core";
import { vfs } from "@kelir/plugin-vfs";
import { ledger } from "@kelir/plugin-ledger";

// 1. Level 3 leaf plugin
const secretMask = createPlugin("secret-mask")
  .decorate("maskActive", () => true);

// 2. Level 2 composite plugin (nests Level 3 + vfs)
const secureWorkspace = createPlugin("secure-workspace")
  .use(vfs(), { paths: ["/workspace/**"] })
  .use(secretMask);

// 3. Level 1 runtime builder mounts composite
const runtime = kelir({ vcpus: 4, memoryMb: 4096 })
  .use(secureWorkspace)
  // Null-object pattern: c.ledger is 100% typed; dispatch table skips hooks at boot
  .use(ledger({ enabled: false }))
  // 2. Mount scoping: filter to target devices and binaries before dispatch evaluation
  .use(repoPolicyPlugin({ maxDiffBytes: 64 * 1024 }), {
    devices: ["virtio-vsock", "virtio-fs"],
    binaries: ["git", "npm"],
  })
  // 3. Declarative POSIX errno mapping (replaces unhandled throw aborts)
  .error({
    EACCES: [UnauthorizedAccessError],
    ENOENT: [MissingConfigError],
  })
  // 4. Lazy per-event context derivation (zero-cost if unread)
  .derive("processLineage", async (c) => c.trap.ancestor_chain.join(" -> "))
  .configure((c) => {
    // 5. Schema push-down: AOT-compiled to native Rust validator at boot
    .guard({
      schema: t.Object({
        action: t.Literal("fix-webhook"),
        issueId: t.String(),
      }),
    })
    // 6. Nano-tracer lifecycle tap
    .trace((t) => {
      if (t.ffi_hop_ns > 10_000) console.warn("FFI boundary spike:", t);
    })
    // 7. `c.policy`, `c.fs`, and `c.processLineage` are fully inferred on unified context:
    .before("git commit *", async (c) => {
      if (c.policy.violations > 0) {
        return c.deny(1, `EPOLICY: ${c.policy.violations} violation(s)`);
      }
      return c.resume();
    })
  });
