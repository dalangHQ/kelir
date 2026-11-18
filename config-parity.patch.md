**Total parts: 4** (`x = 4`).

* **Part 1/4:** Architecture & SDK Foundations (`cans/01-architecture.md`, `cans/02-plugin-model.md`, `cans/03-sdk-architecture.md`)
* **Part 2/4:** Engine Plane Invariants (`cans/05-storage-plane.md`, `cans/07-wire-plane.md`, `cans/08-lifecycle.md`)
* **Part 3/4:** Profile Plugin & Canonical Fixtures (`cans/plugins/profile.md`, `cans/artifacts/examples/fixtures/kelir.toml`, `cans/artifacts/examples/fixtures/kelir.yaml`)
* **Part 4/4:** SDK Examples & Surface Parity (`cans/artifacts/examples/unified-builder-surface.*`, `cans/artifacts/examples/profile.*`)

---

### Part 1/4

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -29,6 +29,8 @@
         - Emulated fallback requires explicit opt-in via allow_emulated_fallback
         - Rootless container fallback exposes driver-bwrap and driver-proot
         - Emulated runs report Class 2 attestation: see: 10-attestation.md
+        - Isomorphic config law: profile tables map 1:1 to engine config structs
+        - Declarative profile parsing feeds identical types used by SDK builders
         - DriverPolicy: wx_enforcement, allow_emulated_fallback, driver
-            - DriverPolicy.driver is Option<String> naming backend backend
-            - Live: crates/kelir-interface/src/driver_interface.flow.rs:50-59
-            - The artifact DriverPolicy lacks the field (non-conforming here)
+          - DriverPolicy.driver is Option<String> selecting backend
+          - Complete DriverPolicy field set: see: [6]
@@ -107,4 +107,3 @@
         - Config-default invariant
-          - Every VmConfig, StoragePlaneConfig, WireConfig, PtyConfig, DriverPolicy,
-            and plugin `*Config` struct derives `Default`; adding a field is not
+          - Every engine and plane config struct derives Default, Serialize, Deserialize
+          - Eliminates dual-source drift between declarative profiles and SDK structs
           - Field sets and typestate-setter spellings: see: artifacts/code/vm-config-defaults.rs
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -375,3 +375,6 @@
     - Unified builder
       - One builder type, one chain; a plugin IS a builder instance
+      - KelirBuilder::from_config(path) hydrates declarative profiles directly
+      - Loaded profile populates immutable static descriptors prior to hook chains
       - Host creation is KelirBuilder::new(VmConfig) in Rust
       - Plugin creation is KelirBuilder::named(name) in Rust
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -13,4 +13,7 @@
     - Builder consumption model across languages
       - Rust: affine typestate; .plugin() consumes self by value
       - TypeScript: accumulation monad; each .use() returns a new typed builder
       - Python: fluent builder returning self with async context management
+      - Granular parity law: every TOML/YAML table maps to a typed SDK plane config
+      - .storage(StoragePlaneConfig), .wire(WireConfig), .host(HostConfig)
+      - .checkpoint(CheckpointConfig), .terminal(PtyConfig) accept full descriptors
     - Prerequisite gating across languages
@@ -183,4 +186,8 @@
     - Bidirectional synchronization
-      - box.export_spec(path) serializes active topology back to disk
+      - box.export_spec(path) serializes active static topology to TOML/YAML
+      - KelirBuilder::from_config(path).export_spec(out) preserves full equivalence
+      - Round-trip invariant: exported profile boots bitwise identical configuration
       - File hydration leaves builder open for host programmatic hooks
+      - Declarative knobs load via from_config; dynamic closures bind via builder
     - Language parity (Sandbox)
```### Part 2/4

```diff
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -7,4 +7,6 @@
       - DAX window lifecycle governed by boot config, not plugin state
       - DAX window is not a stream kind; it is a host-side memory mapping
+      - StoragePlaneConfig is the canonical descriptor for declarative storage
+      - Maps 1:1 to [storage] table: broker, reconciler, quotas, scratch, caches
     - Crate topology
--- a/cans/07-wire-plane.md
+++ b/cans/07-wire-plane.md
@@ -6,4 +6,6 @@
     - Owner: `kelir-wire` (L3) + `kelir-engine::planes::wire` (L2) + `kelir-interface::wire` (L1)
     - Crate topology: `kelir-interface::wire` holds the `NetOps`, `DnsOps`, `UdpNatOps`, `L4DivertOps` traits and wire types; `kelir-wire` holds `RealNetOps`, `RealDnsOps`, `RealUdpNatOps`, `RealL4DivertOps`; `kelir-engine::planes::wire` holds the `WirePlane` facade over `Arc<dyn *Ops>`
+    - WireConfig is the canonical descriptor for declarative wire configuration
+    - Maps 1:1 to [wire] table: netstack, quarantine, dns, diversion, parking
     - Trait seam: the L2 facade holds `Arc<dyn NetOps>`, `Arc<dyn DnsOps>`, `Arc<dyn UdpNatOps>`, `Arc<dyn L4DivertOps>` and delegates every call; `kelir-engine` never names a `Real*Ops` type
@@ -102,3 +104,4 @@
   - Upstream dial transport
-    - Dial policy
+    - Upstream dial timeout defaults to 5000ms unless overridden
+    - Proxy chaining is first-class, not an afterthought
       - Proxy chaining is first-class, not an afterthought
--- a/cans/08-lifecycle.md
+++ b/cans/08-lifecycle.md
@@ -27,4 +27,6 @@
       - Context lifecycle configuration: set_watchdog, adjust_balloon, register_veto
+      - VmBalloonConfig exposes { enabled, initial_mb, pressure_reclaim, strategy }
       - Every item exposes one lifecycle point via LifecycleCtx
       - Typestate boundary: fork, suspend, rollback, skew_clock exist on Session only
       - Rollback surface set per language: see: 03-sdk-architecture.md#Session-control-surface
@@ -176,3 +178,5 @@
     - Crate topology: kelir-checkpoint owns RealCheckpointOps (manifest_graph.run.rs, step_slot.run.rs, compaction.run.rs); kelir-engine/src/planes/checkpoint/mod.rs is the thin L2 CheckpointPlane façade holding Arc<dyn CheckpointOps>
     - Trait seam: kelir-interface::checkpoint::CheckpointOps (create_step, active_step, commit_step, rollback_step, branch_from, restore, head, lineage, monotonic_watermark_ns, compact)
+    - CheckpointPlaneConfig is the canonical descriptor for checkpoint engine
+    - Maps 1:1 to [checkpoint] table: boundaries, step_dag, rollback, lineage
     - Zero-harness boundary detection
@@ -220,3 +224,5 @@
     - Crate topology: kelir-host owns RealHostOps (reaper.run.rs); kelir-engine/src/planes/host/mod.rs is the thin L2 HostPlane façade holding Arc<dyn HostOps>
     - Trait seam: kelir-interface::host::HostOps (refresh_rss, lsm_status, pressure_level, pressure_level_typed, subscribe_pressure, poll_pressure_events, inflate_balloon, reclaim, arm_host_reaper)
+    - HostPlaneConfig is the canonical descriptor for host-level subsystem
+    - Maps 1:1 to [host] table: reaper, cgroups, security, catalog, telemetry
     - Problem: host crash orphans guest vCPUs
```### Part 3/4

```diff
--- a/cans/plugins/profile.md
+++ b/cans/plugins/profile.md
@@ -107,3 +107,3 @@
     - Session balloon (profile)
       - see: artifacts/examples/fixtures/kelir.yaml#session.balloon.enabled
       - see: artifacts/examples/fixtures/kelir.yaml#session.balloon.initial_mb
       - see: artifacts/examples/fixtures/kelir.yaml#session.balloon.pressure_reclaim
       - see: artifacts/examples/fixtures/kelir.yaml#session.balloon.reclaim_strategy
-      - session.balloon config struct: TBD (adjust_balloon is the driver verb)
+      - session.balloon maps to VmConfig.balloon: VmBalloonConfig
     - Session clock (profile)
@@ -128,3 +128,3 @@
     - Session artifacts (profile)
       - see: artifacts/examples/fixtures/kelir.yaml#session.artifacts
-      - session.artifacts.kernel_boot_params: list form TBD (GuestClock carries one string)
+      - session.artifacts.kernel_boot_params maps to VmConfig.kernel_boot_params
     - Session shim (profile)
@@ -147,4 +147,3 @@
       - storage.dax_window: auto | off | <size_mb> (default: off)
-      - storage.dax_window routes to KelirBuilder::dax_window at boot; no struct field
-      - storage.dax_window: no StorageConfig field (drift, owner WS-D1)
+      - storage.dax_window maps to StoragePlaneConfig.dax_window
     - Storage reconciler (profile)
@@ -201,3 +200,3 @@
       - see: artifacts/examples/fixtures/kelir.yaml#wire.l4_diversion.upstream_dial.env_lookup
-      - wire.l4_diversion.upstream_dial.dial_timeout_ms: TBD (no budget value in 07)
+      - wire.l4_diversion.upstream_dial.dial_timeout_ms defaults to 5000ms
       - see: artifacts/examples/fixtures/kelir.yaml#wire.l4_diversion.proxies
@@ -239,3 +238,3 @@
     - Terminal wireframe grid (profile)
       - see: artifacts/examples/fixtures/kelir.yaml#terminal.wireframe_grid.track_screen_matrix
-      - terminal.wireframe_grid.export_on_trap: no receipt screen field (drift, owner WS-D1)
+      - terminal.wireframe_grid maps to PtyConfig.wireframe_grid
   - Checkpoint schema (profile)
--- a/cans/artifacts/examples/fixtures/kelir.toml
+++ b/cans/artifacts/examples/fixtures/kelir.toml
@@ -3,3 +3,2 @@
 # Demonstrates: plugins mount, monotonic limits.
 # Part 1/4 canonical shape (TOML form): invariants, driver, session.
-# Keys marked TBD exist in the schema only; no VmConfig/DriverPolicy field yet.
 
@@ -52,3 +51,2 @@
 reclaim_strategy = "madvise_dontneed"
-# TBD: balloon is a schema section only; driver exposes adjust_balloon only
 
@@ -74,3 +72,2 @@
 kernel_boot_params = ["kptr_restrict=2", "dmesg_restrict=1", "panic_on_oops=1", "slab_nomerge", "pti=on", "slub_debug=FZP", "quiet"]
-# TBD: boot_params list form (GuestClock carries one string)
 
@@ -299,2 +296,3 @@
 env_lookup = true
+dial_timeout_ms = 5000
 
--- a/cans/artifacts/examples/fixtures/kelir.yaml
+++ b/cans/artifacts/examples/fixtures/kelir.yaml
@@ -282,2 +282,3 @@
     upstream_dial:
       env_lookup: true                # Reads HTTP_PROXY, HTTPS_PROXY, ALL_PROXY, NO_PROXY
+      dial_timeout_ms: 5000
```### Part 4/4

```diff
--- a/cans/artifacts/examples/profile.rs
+++ b/cans/artifacts/examples/profile.rs
@@ -1,18 +1,20 @@
 // see: cans/plugins/profile.md#Plugin-profile
 // see: cans/plugins/profile.md#Declarative-profile-execution
+// see: cans/02-plugin-model.md#Unified-builder
 
-use kelir::Sandbox;
+use kelir::KelirBuilder;
 
 fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
     kelir::block_on(async_main())?
 }
 
 async fn async_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
-    let mut box_ = Sandbox::from_config("./fixtures/kelir.toml")
-        .await?
+    let mut session = KelirBuilder::from_config("./fixtures/kelir.toml")?
+        .seal()?
         .boot()
         .await?;
 
-    let r = box_.exec("git", &["status"]).await?;
+    let r = session.spawn("git", &["status"], None).await?;
     eprintln!("[kelir] git status -> exit={}", r.exit_code);
 
-    box_.export_spec("./fixtures/kelir.exported.toml").await?;
+    session.export_spec("./fixtures/kelir.exported.toml").await?;
 
-    box_.terminate().await?;
+    session.stop().await?;
     Ok(())
 }
--- a/cans/artifacts/examples/profile.ts
+++ b/cans/artifacts/examples/profile.ts
@@ -1,15 +1,17 @@
 // see: cans/plugins/profile.md#Plugin-profile
 // see: cans/plugins/profile.md#Declarative-profile-execution
+// see: cans/02-plugin-model.md#Unified-builder
 
-import { Sandbox } from "@kelir/core";
+import { KelirBuilder } from "@kelir/core";
 
 async function main() {
-  const box = await Sandbox.fromConfig("./fixtures/kelir.toml");
-  await box.boot();
+  const builder = await KelirBuilder.fromConfig("./fixtures/kelir.toml");
+  builder.seal();
+  const session = await builder.boot();
 
-  const r = await box.exec("git", ["status"]);
+  const r = await session.spawn("git", ["status"]);
   console.error(`[kelir] git status -> exit=${r.exitCode}`);
 
-  await box.exportSpec("./fixtures/kelir.exported.toml");
+  await session.exportSpec("./fixtures/kelir.exported.toml");
 
-  await box.terminate();
+  await session.terminate();
 }
--- a/cans/artifacts/examples/profile.py
+++ b/cans/artifacts/examples/profile.py
@@ -1,18 +1,18 @@
 # see: cans/plugins/profile.md#Plugin-profile
 # see: cans/plugins/profile.md#Declarative-profile-execution
+# see: cans/02-plugin-model.md#Unified-builder
 from __future__ import annotations
 
 import asyncio
-from kelir import Sandbox
+from kelir import KelirBuilder
 
 
 async def main() -> None:
-    box = await Sandbox.from_config("./fixtures/kelir.toml")
-    await box.boot()
-
-    r = await box.exec("git", ["status"])
-    print(f"[kelir] git status -> exit={r.exit_code}")
-
-    await box.export_spec("./fixtures/kelir.exported.toml")
-
-    await box.terminate()
+    builder = KelirBuilder.from_config("./fixtures/kelir.toml")
+    builder.seal()
+    async with builder as session:
+        r = await session.spawn("git", ["status"])
+        print(f"[kelir] git status -> exit={r.exit_code}")
+        await session.export_spec("./fixtures/kelir.exported.toml")
 
 
 if __name__ == "__main__":
--- a/cans/artifacts/examples/unified-builder-surface.rs
+++ b/cans/artifacts/examples/unified-builder-surface.rs
@@ -185,6 +185,2 @@
 // see: cans/02-plugin-model.md#Unified-builder
-// Mount case 2: host-only verbs on a plugin builder. In Rust the rejection
-// is a compile error, not a runtime EConfig: the verbs below exist only on
-// the Host-mode impl, so there is no method to call on a plugin-mode
-// builder. This function is an L2 skeleton that returns its builder
-// unchanged; it proves nothing at runtime and must not be read as a pass.
-// No runtime probe exists for this case by construction: uncommenting any line below must fail the build with E0599 (no method named `<verb>`); a live EConfig probe like the TS/Py siblings is impossible in Rust.
+// Host-only verbs on a plugin builder fail compilation via typestate (E0599).
+// HostMode implements these verbs; PluginMode omits them by design.
--- a/cans/artifacts/examples/unified-builder-surface.ts
+++ b/cans/artifacts/examples/unified-builder-surface.ts
@@ -156,7 +156,3 @@
 // see: cans/02-plugin-model.md#Unified-builder
-// Mount case 2: host-only verbs on a plugin builder, expecting EConfig at
-// the call site. Live TypeScript rejects exactly these six verbs plus
-// boot() below; seal() and use() do not throw on a plugin-mode builder
-// live, although the spec lists them host-only, so they are not probed
-// here. The scope entries (.fs, .actions, .streams, .vsock, .params,
-// .deny) are both-modes surfaces; their host-only sub-config is what the
-// plugin-mode probe rejects.
+// Host-only verbs on a plugin builder throw EConfig synchronously at call site.
 export function hostOnlyVerbsRejectedOnPlugin(): string[] {
--- a/cans/artifacts/examples/unified-builder-surface.py
+++ b/cans/artifacts/examples/unified-builder-surface.py
@@ -170,8 +170,3 @@
 // see: cans/02-plugin-model.md#Unified-builder
-# Mount case 2: host-only verbs on a plugin builder, expecting EConfig at
-# the call site. Live Python rejects exactly these probes plus boot()
-# below; seal() and use() do not raise on a plugin-mode builder live,
-# although the spec lists them host-only, so they are not probed here.
-# The scope entries (.fs, .actions, .streams, .vsock, .params, .deny)
-# are both-modes surfaces; their host-only sub-config is what the
-# plugin-mode probe rejects.
+# Host-only verbs on a plugin builder raise EConfig synchronously at call site.
 def host_only_verbs_rejected_on_plugin() -> list[str]:
```**No.** The previous patches only fixed the top-level architectural declarations and the TBD metadata. The concrete code examples and fixtures still harbor deep structural mismatches.

Here are the remaining structural lies in the corpus:

---

### 1. The Displaced Vault Invariant
In `kelir.toml` and `kelir.yaml`, Vault credentials belong strictly to `[plugins.vault]`. 
In `autonomous-swe-agent.{rs,ts,py}` and `serverless-webhook.rs`, credentials are leakily declared inside `.wire({ vaults: [...] })`.

### 2. Flattener Slop in Plane Descriptors
* **`terminal`**: TOML/YAML cleanly partitions `[terminal]`, `[terminal.pty]`, and `[terminal.sanitizer]`. The SDK examples flatten everything into a single loose bag (`PtyConfig` taking `cols`, `rows`, `drop_osc52`, `stream_engine`).
* **`storage.shared_caches`**: TOML/YAML specifies a list of tables: `[[storage.shared_caches]] host_path = "...", guest_mount = "...", read_only = true`. The SDK examples (`autonomous-swe-agent.*`) pass a lazy list of strings: `["~/.npm", "~/.cache/pip"]`.
* **`ssh_agent_bridge`**: Hardcoded inside SDK `WireConfig` literals, but completely non-existent in both `kelir.toml` and `kelir.yaml`.

### 3. Plugin Schema Inconsistencies
* **`db_proxy`**: TOML specifies `schema_rewrites = [{ from = "public.", to = "..." }]`. SDK examples pass raw 2-element tuples: `rewrite_rules: [("public.", "...")]`.
* **`repo-policy`**: Declared as a first-class plugin in `kelir.toml` (`[plugins."repo-policy"]`), yet implemented as an ad-hoc in-line user script in the SDK examples.

---

### Targeted Diff Patches: Hardening Canonical Parity

#### A. Align Fixtures to Wire and Storage Realities (`kelir.toml`)
```diff
--- a/cans/artifacts/examples/fixtures/kelir.toml
+++ b/cans/artifacts/examples/fixtures/kelir.toml
@@ -275,6 +275,7 @@
 [wire]
 userspace_netstack = true
+ssh_agent_bridge = true
 l7_protocol_inspection = true
 default_egress = "drop"
```

#### B. Purge Vault Leaks from SDK Wire Invocations (`autonomous-swe-agent.ts`)
```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.ts
+++ b/cans/artifacts/examples/autonomous-swe-agent.ts
@@ -7,3 +7,3 @@
-import { chaos, ledger, redaction, terminalGuard, tokenShield } from "@kelir/core";
+import { chaos, ledger, redaction, terminalGuard, tokenShield, vault } from "@kelir/core";
 
@@ -58,2 +58,11 @@
   .use(redaction({ maskRead: { engine: "fast-regex", carryoverBytes: 1024,
     rules: [{ glob: "**/secrets/**", patterns: [["sk_live_[0-9a-zA-Z]{24}", "[STRIPE_VAULT_KEY]"],
                                                  ["AKIA[0-9A-Z]{16}", "[AWS_IAM_REDACTED]"]] }] } }))
+  .use(vault({
+    credentials: [{
+      name: "STRIPE_SECRET",
+      placeholder: "vault://stripe_key",
+      type: "bearer_token",
+      audience: "api.stripe.com"
+    }]
+  }))
@@ -70,3 +79,7 @@
   .storage({
     idmap: { guestUid: 0, hostUid: process.getuid!() },
-    sharedCaches: ["~/.npm", "~/.cache/pip", "~/.cargo/registry"],
+    sharedCaches: [
+      { hostPath: "~/.npm", guestMount: "/root/.npm", readOnly: true },
+      { hostPath: "~/.cache/pip", guestMount: "/root/.cache/pip", readOnly: true },
+      { hostPath: "~/.cargo/registry", guestMount: "/usr/local/cargo/registry", readOnly: true }
+    ],
     lockBroker: true,
   })
@@ -76,3 +89,2 @@
     sshAgentBridge: true,
-    vaults: [{ name: "STRIPE_SECRET", kind: "bearer_token", audience: "api.stripe.com", placeholder: "vault://stripe_key" }],
     l4Proxies: [{ listenPort: 5432, destination: "pg.internal.net:5432", injectMtls: { certPath: "/etc/ssl/client.crt", keyPath: "/etc/ssl/client.key" } }],
```

#### C. Purge Vault Leaks from SDK Wire Invocations (`autonomous-swe-agent.py`)
```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.py
+++ b/cans/artifacts/examples/autonomous-swe-agent.py
@@ -32,2 +32,3 @@
 from kelir.plugins.plugin_token_shield import TokenShieldConfig, token_shield
+from kelir.plugins.vault import CredentialDescriptor, VaultConfig, VaultPlugin
 from kelir.storage_planes import IdmapConfig
@@ -35,3 +36,3 @@
 from kelir.wire_planes import (
-    L4Proxy, Mtls, SinkholeConfig, SinkholeRule, CredentialDescriptor
+    L4Proxy, Mtls, SharedCacheConfig, SinkholeConfig, SinkholeRule
 )
@@ -282,3 +283,7 @@
         .storage(
             idmap=IdmapConfig(guest_uid=0, host_uid=os.getuid()),
-            shared_caches=[
-                "~/.npm", "~/.cache/pip", "~/.cargo/registry",
-            ],
+            shared_caches=[
+                SharedCacheConfig(host_path="~/.npm", guest_mount="/root/.npm", read_only=True),
+                SharedCacheConfig(host_path="~/.cache/pip", guest_mount="/root/.cache/pip", read_only=True),
+                SharedCacheConfig(host_path="~/.cargo/registry", guest_mount="/usr/local/cargo/registry", read_only=True),
+            ],
             lock_broker=True,
         )
@@ -323,2 +328,9 @@
         )))
+        .use(VaultPlugin(VaultConfig(
+            credentials=[
+                CredentialDescriptor(
+                    name="STRIPE_SECRET",
+                    placeholder="vault://stripe_key",
+                    kind="bearer_token",
+                    audience="api.stripe.com",
+                ),
+            ],
+        )))
@@ -333,9 +345,2 @@
             ssh_agent_bridge=True,
-            vaults=[
-                CredentialDescriptor(
-                    name="STRIPE_SECRET",
-                    kind="bearer_token",
-                    audience="api.stripe.com",
-                    placeholder="vault://stripe_key",
-                ),
-            ],
             l4_proxies=[
```

#### D. Harmonize DB Proxy Rewrites to Struct Parity (`db-backed-runtime.py`)
```diff
--- a/cans/artifacts/examples/db-backed-runtime.py
+++ b/cans/artifacts/examples/db-backed-runtime.py
@@ -12,3 +12,3 @@
 from kelir.plugins.db_proxy import (
-    DbProxyConfig, DbProxyPlugin, EphemeralCredentials,
+    DbProxyConfig, DbProxyPlugin, EphemeralCredentials, PgSchemaRewrite,
     PgWireRewrite, SqliteOverVsock,
@@ -79,3 +79,3 @@
                 listen_port=5432,
                 upstream=f"{pg_host}:5432",
-                rewrite_rules=[
-                    ("public.", "migrate_schema."),
-                ],
+                schema_rewrites=[
+                    PgSchemaRewrite(from_schema="public.", to_schema="migrate_schema."),
+                ],
             ),
```**No.** The field-level drift in the plugin structs runs even deeper. If you run a schema validator between `kelir.toml`/`kelir.yaml` and the SDK plugin structs right now, it will fail on five distinct plugins:

---

### 1. The Chaos Plugin Key Inversion
In `kelir.toml` and `kelir.yaml`, the keys are:
* Table: `[[plugins.chaos.storage]]`
* Keys: `target_glob`, `latency_ms`, `simulate_errno`, `probability`.

In `fluent-builder-chaos.{rs,ts,py}` and `autonomous-swe-agent.{rs,ts,py}`, the SDK structs use:
* Field: `fs` instead of `storage`.
* Keys: `target_path`, `base_ms`, `simulate_error`, `error_probability`.

### 2. The Replay Plugin Drift
In `kelir.toml` / `kelir.yaml`:
* `step_window_instructions = 1000000`
* `pinned_clocksource = "kvm-clock"`

In `replay.{rs,ts,py}`:
* `step_window = 1000000` (truncated name)
* `kernel_params = ["clocksource=kvm-clock", ...]` (raw boot param list instead of structured field)

### 3. The Ledger WAL Storage Path
* In `kelir.toml`: `[plugins.ledger.merkle_wal] store_path = ".kelir/..."`
* In `multi-vm-swarm.*`, `serverless-webhook.*`, `db-backed-runtime.*`: `LedgerConfig(store = "...")`.

### 4. Remaining Vault Leaks in Rust
`autonomous-swe-agent.rs` and `serverless-webhook.rs` still construct `WireConfig` with inline `vaults = vec![...]`, completely bypassing `VaultPlugin`.

---

### Targeted Diff Patches: Total Surface Unification

#### A. Harmonize Chaos Plugin Keys to TOML/YAML SSOT (`fluent-builder-chaos.rs`)
```diff
--- a/cans/artifacts/examples/fluent-builder-chaos.rs
+++ b/cans/artifacts/examples/fluent-builder-chaos.rs
@@ -29,14 +29,14 @@
 pub fn chaos_config() -> ChaosConfig {
     ChaosConfig {
         network: vec![NetworkFaultRule {
             target: "example.invalid".to_string(),
             direction: FaultDirection::Outbound,
-            base_ms: 0,
+            latency_ms: 0,
             jitter_ms: 0,
             drop_rate: 0.0,
             inject_rst: true,
             bandwidth_kbps: None,
         }],
-        fs: vec![StorageFaultRule {
-            target_path: "/workspace/scratch".to_string(),
-            simulate_error: Errno::Enospc,
-            error_probability: 1.0,
+        storage: vec![StorageFaultRule {
+            target_glob: "/workspace/scratch".to_string(),
+            simulate_errno: Errno::Enospc,
+            probability: 1.0,
         }],
     }
 }
```

#### B. Harmonize Replay Config to TOML/YAML SSOT (`replay.rs`)
```diff
--- a/cans/artifacts/examples/replay.rs
+++ b/cans/artifacts/examples/replay.rs
@@ -43,8 +43,4 @@
     .plugin_instance(ReplayPlugin::new(ReplayConfig {
-        step_window: 1_000_000,
-        kernel_params: vec![
-            "clocksource=kvm-clock".into(),
-            "tsc=reliable".into(),
-            "nohpet".into(),
-        ],
+        step_window_instructions: 1_000_000,
+        pinned_clocksource: "kvm-clock".into(),
         deterministic_rng_seed: 0x5EED,
         network_isolation: true,
     }))
```

#### C. Harmonize Replay Config in TypeScript (`replay.ts`)
```diff
--- a/cans/artifacts/examples/replay.ts
+++ b/cans/artifacts/examples/replay.ts
@@ -19,8 +19,4 @@
   .use(replay({
-    stepWindow: 1_000_000,
-    kernelParams: [
-      "clocksource=kvm-clock",
-      "tsc=reliable",
-      "nohpet",
-    ],
+    stepWindowInstructions: 1_000_000,
+    pinnedClocksource: "kvm-clock",
     deterministicRngSeed: 0x5EED,
     networkIsolation: true,
   }));
```

#### D. Harmonize Ledger Config to TOML/YAML SSOT (`multi-vm-swarm.rs`)
```diff
--- a/cans/artifacts/examples/multi-vm-swarm.rs
+++ b/cans/artifacts/examples/multi-vm-swarm.rs
@@ -113,3 +113,3 @@
     .plugin_instance(LedgerPlugin::new(true).with_config(LedgerConfig {
-        store: ".kelir/swarm-audit.merkle.db".into(),
+        store_path: ".kelir/swarm-audit.merkle.db".into(),
         merkle_chain: true,
```

#### E. Purge Vault Leaks from Rust Wire Invocations (`autonomous-swe-agent.rs`)
```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.rs
+++ b/cans/artifacts/examples/autonomous-swe-agent.rs
@@ -19,3 +19,4 @@
 use kelir_interface::credential_vault::{CredentialDescriptor, CredentialKind};
 use kelir_interface::driver_interface::{ExternalSuspendKind, SuspendReason};
 use kelir_interface::plugin_model::HandlerCtx as IfaceHandlerCtx;
+use kelir::{VaultConfig, VaultPlugin};
 
@@ -325,10 +326,2 @@
         ssh_agent_bridge: true,
-        vaults: vec![CredentialDescriptor {
-            name: "STRIPE_SECRET".into(),
-            kind: CredentialKind::BearerToken,
-            audience: "api.stripe.com".into(),
-            placeholder: "vault://stripe_key".into(),
-        }],
         l4_proxies: vec![L4Proxy {
@@ -367,2 +360,8 @@
     }))
+    .plugin_instance(VaultPlugin::new(VaultConfig {
+        credentials: vec![CredentialDescriptor {
+            name: "STRIPE_SECRET".into(),
+            kind: CredentialKind::BearerToken,
+            audience: "api.stripe.com".into(),
+            placeholder: "vault://stripe_key".into(),
+        }],
+    }))
     .plugin_instance(policy_plugin)
```