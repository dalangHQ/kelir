## Parts: 7

Grouping by plane keeps each part self-contained and avoids cross-part re-reads:

| Part | Scope | Files touched |
|---|---|---|
| **1** | Wire Plane | `07-wire-plane.md`, `11-protocol.md`, `plugins/vault.md`, `plugins/transaction.md`, `plugins/mcp.md`, `plugins/l7-http.md` (new), `plugins/egress-proxy.md` (new) |
| 2 | Terminal Plane | `04-terminal-plane.md`, `plugins/token-shield.md` (new), `plugins/terminal-guard.md` (new) |
| 3 | Checkpoint & Lifecycle | `08-lifecycle.md`, `10-attestation.md`, `plugins/tool-guard.md`, `plugins/wire-ingress.md`, `plugins/step-eval.md` (new) |
| 4 | Trap Pipeline & Protocol | `11-protocol.md`, `02-plugin-model.md`, `plugins/supply-chain.md`, `plugins/mcp.md`, `plugins/tool-guard.md`, `plugins/taint.md` (new), `plugins/vault.md` |
| 5 | Storage Plane | `05-storage-plane.md`, `plugins/vault.md`, `plugins/profile.md`, `plugins/redaction.md` (new) |
| 6 | Attestation & Observability | `10-attestation.md`, `14-observability.md`, `plugins/ledger.md`, `plugins/db-proxy.md`, `plugins/anomaly.md` (new) |
| 7 | CLI, fixtures, catalog index | `15-operations.md`, `00-overview.md`, `02-plugin-model.md` catalog, `artifacts/examples/fixtures/kelir.{toml,yaml}` |

---

# Part 1/7 — Wire Plane

Canonical post-extraction shape: core wire stops at L2/L3/L4. L7 parse, TLS, HTTP `403` receipts, IMDS special-casing, sagas, and proxy-chaining all move to plugins.

---

### `cans/07-wire-plane.md` — node removals

```
@@ #Transparent-L4-stream-diversion @@
-  - In-ring L7 parsing (HTTP headers, SSE frames, JSON-RPC bodies)
-  - TLS interception via host keyring
-  - SinkholeRule.methods / SinkholeRule.paths
+  - L4 byte-stream diversion only; L7 payloads are opaque to core
+  - Diverted streams delivered raw to plugins/l7-http (see: plugins/l7-http.md)

@@ #Pre-policy-drop-zones @@
-  - Hardcoded IMDS redirect: 169.254.169.254 → plugin-vault vsock
-  - imds_exception special-case in L3 drop path
+  - Generic L3 drop rules; no address literals in core
+  - plugin-vault registers its own boot-time diversion (see: plugins/vault.md)

@@ #Egress-side-effect-outbox @@
-  - Irreversible-operation outboxes
-  - Undo webhooks dispatched on step failure
-  - Compensating-saga orchestration
+  - L4 stream pause / flush / drop primitives only
+  - Saga + undo-webhook ownership → plugins/transaction.md

@@ #Upstream-dial-transport @@
-  - HTTP CONNECT handshake parsing
-  - SOCKS5 auth negotiation in core netstack
+  - L4 dialer trait exposed to plugins
+  - Handshake ownership → plugins/egress-proxy.md
```

---

### `cans/11-protocol.md` — node removal

```
@@ #Policy-rejection-receipt @@
-  - Core synthesizes HTTP 403 {"error":"E_POLICY_DENIED", ...} JSON body
+  - Core emits raw trap verdict; on-wire HTTP 403 synthesis → plugins/l7-http.md
```

---

### `cans/plugins/vault.md`

```diff
--- a/cans/plugins/vault.md
+++ b/cans/plugins/vault.md
@@ -11,6 +11,9 @@
     - Lifecycle attachment (vault): see: 08-lifecycle.md#Lifecycle-vetoes
+    - Registers L3/L4 packet-diversion rule at boot
+      - Redirects IMDS destination 169.254.169.254 to this vsock service
+      - Core wire plane carries no IMDS special-case
   - Service exports (vault)
     - Exports CredentialBrokerService via PipelineBinder for scoped token issuance
   - Invariants and policy (vault)
@@ -41,9 +44,10 @@
     - In-ring carrier secret injection
-      - Wire plane natively interposes outbound L7 streams matching vault rules
-      - Substitutes vault:// placeholders with real bearer tokens in flight
+      - Plugin interposes outbound L4-diverted streams matching vault rules
+      - Parses L7, substitutes vault:// placeholders with bearer tokens in flight
+      - Core wire plane delivers raw L4 streams; no L7 parse in core
       - Eliminates external sidecar daemons, preserving strict Class 1 attestation
     - Memory isolation and zeroization
```

---

### `cans/plugins/transaction.md` — node additions

Insert under `- Invariants and policy (transaction)`:

```
- Compensating sagas and undo webhooks
  - Irreversible-operation outboxes owned here, not core
  - On step failure, dispatch undo webhooks for registered operations
  - Core wire plane exposes only L4 stream pause/flush/drop primitives
```

---

### `cans/plugins/mcp.md`

```diff
--- a/cans/plugins/mcp.md
+++ b/cans/plugins/mcp.md
@@ -14,6 +14,7 @@
   - Invariants and policy (mcp)
     - Thin in-process adapter routing JSON-RPC stdio directly to VirtIO rings
+    - Owns all JSON-RPC L7 parsing; core wire plane carries raw L4 only
     - MCP plugin: maps tools directly to virtio-console, virtio-fs, and traps
```

---

### `cans/plugins/l7-http.md` — NEW

```
- Plugin l7-http
  - Crate dependencies (l7-http)
    - Plugin identifier: plugin-l7-http
    - Owner crate: crates/kelir-plugins/plugin-l7-http
    - name() = "l7-http"
    - Tier 2 governance and protocol proxy (l7-http)
    - Depends on (l7-http): kelir-interface (binds to native Wire Plane L4)
    - Exposes: HttpPolicyService
    - VirtIO attachment (l7-http): see: 01-architecture.md#VirtIO-Device-Complex
    - Lifecycle attachment (l7-http): see: 08-lifecycle.md#Lifecycle-vetoes
  - Service exports (l7-http)
    - Exports HttpPolicyService via PipelineBinder for L7 verdicts
  - Invariants and policy (l7-http)
    - Owns all HTTP/1.1, HTTP/2, SSE, and JSON-RPC L7 parsing
    - Owns TLS interception via host keyring (plugin-scoped, not core)
    - Synthesizes policy-rejection receipts on the diverted L4 stream
      - HTTP 403 {"error":"E_POLICY_DENIED", reason, pid, binary}
    - SinkholeRule (methods, paths) is plugin-private config
    - Core wire plane stops at L4; every byte above is owned here
  - Error and return semantics (l7-http)
    - Hook signature (l7-http): see: artifacts/code/plugin-trait.rs
    - Return errno and rejection behavior (l7-http)
      - Policy deny → 403 with E_POLICY_DENIED JSON body
      - Upstream TLS failure → 502 with E_UPSTREAM_TLS
```

---

### `cans/plugins/egress-proxy.md` — NEW

```
- Plugin egress-proxy
  - Crate dependencies (egress-proxy)
    - Plugin identifier: plugin-egress-proxy
    - Owner crate: crates/kelir-plugins/plugin-egress-proxy
    - name() = "egress-proxy"
    - Tier 2 egress mediation (egress-proxy)
    - Depends on (egress-proxy): kelir-interface (L4 dialer trait)
    - Exposes: EgressProxyService
    - VirtIO attachment (egress-proxy): see: 01-architecture.md#VirtIO-Device-Complex
    - Lifecycle attachment (egress-proxy): see: 08-lifecycle.md#Lifecycle-vetoes
  - Service exports (egress-proxy)
    - Exports EgressProxyService via PipelineBinder for proxy chaining
  - Invariants and policy (egress-proxy)
    - Owns HTTP CONNECT and SOCKS5 negotiation end to end
    - Reads HTTP_PROXY, HTTPS_PROXY, ALL_PROXY, NO_PROXY from env
    - Core wire plane carries raw L4 splice only; no handshake parse
  - Error and return semantics (egress-proxy)
    - Hook signature (egress-proxy): see: artifacts/code/plugin-trait.rs
    - Return errno and rejection behavior (egress-proxy)
      - Upstream proxy auth failure → 407 passthrough to guest
      - SOCKS5 method rejection → ECONNREFUSED
```

---

**Deferred to later parts (called out, not forgotten):**
- Fixture keys `wire.l7_protocol_inspection`, `wire.outbox_deferral`, `wire.quarantine.imds_exception`, `wire.l4_diversion.upstream_dial` → **Part 7** (fixtures) and **Part 5** (as `plugins.vault` / `plugins.transaction` / `plugins.l7-http` / `plugins.egress-proxy` blocks).
- Sinkhole config migration `07 → l7-http` → **Part 7**.

Part 2/7 ready on request.


# Part 2/7 — Terminal Plane

Canonical post-extraction shape: core terminal keeps raw VT100 grid, byte-stream rings, `index.bin` replay log, and `write_console(port, bytes)`. All prompt matching, context-window clamping, and user-facing ANSI formatting move to plugins.

---

### `cans/04-terminal-plane.md` — node removals

```
@@ #Terminal-pipeline @@
-  - TerminalWiring hardcodes token_shield (maxLines / maxBytes scrollback clamp)
-  - TerminalWiring hardcodes prompts (regex / text prompt matcher)
-  - Fixed pipeline order: sanitize → token_shield → prompts
+  - Pipeline exposes ConsoleInterposer::on_console_output to plugins
+  - Plugin chain order is plugin-owned; core carries no fixed stages
+  - Core stages: raw VT100 grid, byte-stream ring, index.bin replay only

@@ #Disclosure @@
-  - Core constructs formatted ANSI notice boxes on Port 0
+  - Core exposes raw write_console(port, bytes) + EventBus emission
+  - ANSI notice-box rendering → plugins/terminal-guard.md
```

---

### `cans/plugins/token-shield.md` — NEW

```
- Plugin token-shield
  - Crate dependencies (token-shield)
    - Plugin identifier: plugin-token-shield
    - Owner crate: crates/kelir-plugins/plugin-token-shield
    - name() = "token-shield"
    - Tier 2 terminal output governance (token-shield)
    - Depends on (token-shield): kelir-interface (ConsoleInterposer)
    - Exposes: ScrollbackClampService
    - Lifecycle attachment (token-shield): see: 08-lifecycle.md#Lifecycle-vetoes
  - Service exports (token-shield)
    - Exports ScrollbackClampService via PipelineBinder for output clamping
  - Invariants and policy (token-shield)
    - Implements ConsoleInterposer::on_console_output
    - Clamps scrollback to protect model context window
      - max_bytes_total: per-execution byte ceiling
      - max_lines_total: per-execution line ceiling
      - clamp_strategy: head_and_tail | head_only | tail_only
    - Emits notice line on clamp (plugin-private template)
    - Context-window concept is plugin-owned; not a terminal primitive
    - Core terminal carries no maxLines / maxBytes config
    - see: artifacts/examples/fixtures/kelir.yaml#plugins.token_shield
  - Error and return semantics (token-shield)
    - Hook signature (token-shield): see: artifacts/code/plugin-trait.rs
    - Return errno and rejection behavior (token-shield)
      - Clamp is Pass with mutated byte buffer; no errno
      - Misconfiguration → EConfig at boot
```

---

### `cans/plugins/terminal-guard.md` — NEW

```
- Plugin terminal-guard
  - Crate dependencies (terminal-guard)
    - Plugin identifier: plugin-terminal-guard
    - Owner crate: crates/kelir-plugins/plugin-terminal-guard
    - name() = "terminal-guard"
    - Tier 2 interactive terminal governance (terminal-guard)
    - Depends on (terminal-guard): kelir-interface (ConsoleInterposer)
    - Exposes: PromptResponderService
    - Lifecycle attachment (terminal-guard): see: 08-lifecycle.md#Lifecycle-vetoes
  - Service exports (terminal-guard)
    - Exports PromptResponderService via PipelineBinder for prompt automation
  - Invariants and policy (terminal-guard)
    - Implements ConsoleInterposer::on_console_output
    - Matches interactive prompts against plugin-private regex set
      - Pattern, timeout_seconds, action bytes per rule
      - verify_foreground: gate on issuing process owning active pgid
    - Matches against shadow-stripped text; ANSI stripped before regex
    - Renders ANSI notice boxes on Port 0 for user-facing disclosures
      - Core exposes only raw write_console(port, bytes) + EventBus
    - All prompt patterns and notice templates are plugin-private config
    - Core terminal pipeline carries no prompt rules and no notice format
    - see: artifacts/examples/fixtures/kelir.yaml#plugins.terminal_guard
  - Error and return semantics (terminal-guard)
    - Hook signature (terminal-guard): see: artifacts/code/plugin-trait.rs
    - Return errno and rejection behavior (terminal-guard)
      - Prompt match → write action bytes to Port 0; Pass verdict
      - Timeout without match → no write; Pass verdict
      - Foreground check fails → no write; Pass verdict with reason tag
```

---

**Deferred to later parts:**

- Fixture key `terminal.sanitizer.token_shield` and `terminal.sanitizer.prompts` → **Part 7** (moved to `plugins.token_shield.*` and `plugins.terminal_guard.*`).
- Fixture key `terminal.sanitizer.notice` → **Part 7** (moved to `plugins.terminal_guard.notice_template`).
- `on_console_output` interposer trait shape → **Part 4** (protocol ring surface).

Part 3/7 ready on request.

# Part 3/7 — Checkpoint & Lifecycle

Canonical post-extraction shape: core `kelir-checkpoint` provides content-addressed RAM+disk DAG primitives only — `snapshot()`, `fork()`, `branch_from()`, `rollback(id)`, `compact()`. Orchestration, verdicts, signed reviewer attribution, quiescence heuristics, and `ExternalSuspendKind` variants move to plugins.

---

### `cans/08-lifecycle.md` — node removals

```
@@ #Native-checkpoint-plane @@
-  - StepSlot, StepEvaluator, StepVerdict, PostExecAction types
-  - Ed25519 reviewer attribution and separation-of-duties enforcement
-  - Tainted-step delta verification
-  - PTY idle quiescence heuristics
-  - Magic path /dev/kelir-gate sentinel
+  - Primitive surface: snapshot() | fork() | branch_from()
+  - Primitive surface: rollback(id) | compact()
+  - Content-addressed RAM+disk DAG owned by core
+  - Evaluation orchestration → plugins/step-eval.md

@@ #Hypervisor-primitives @@
-  - ExternalSuspendKind::{HumanAuthorizationHold, WireConnectionParking,
-      StepEvaluationYield}
+  - ExternalSuspendKind is opaque: External(String) | External(u32)
+  - Reason-tag ownership lives with each issuing plugin
```

---

### `cans/10-attestation.md` — node removal

```
@@ #Step-evaluation-loop @@
-  - Step loop lives in kelir-engine::checkpoint
-  - Commit / Rollback / YieldToHost verdict dispatch in core
+  - Step loop ownership → plugins/step-eval.md
+  - Core receipts expose raw facts only; no verdict field
```

---

### `cans/plugins/step-eval.md` — NEW

```
- Plugin step-eval
  - Crate dependencies (step-eval)
    - Plugin identifier: plugin-step-eval
    - Owner crate: crates/kelir-plugins/plugin-step-eval
    - name() = "step-eval"
    - Tier 2 governance and orchestration (step-eval)
    - Depends on (step-eval): kelir-interface (checkpoint DAG primitives)
    - Exposes: StepEvaluationService
    - Lifecycle attachment (step-eval): see: 08-lifecycle.md#Lifecycle-vetoes
  - Service exports (step-eval)
    - Exports StepEvaluationService via PipelineBinder for step verdicts
  - Invariants and policy (step-eval)
    - Owns StepSlot, StepEvaluator, StepVerdict, PostExecAction
    - Verdicts: Commit | Rollback | YieldToHost dispatched here, not core
    - Ed25519 reviewer attribution and separation-of-duties enforcement
    - Tainted-step delta verification (binary BLAKE3, dirty-byte, duration)
    - PTY idle quiescence heuristics for step boundaries
    - Owns /dev/kelir-gate sentinel path interpretation
    - Emits ExternalSuspendKind::External("step-evaluation-yield")
    - Core checkpoint offers snapshot/fork/rollback/compact only
    - see: artifacts/examples/fixtures/kelir.yaml#plugins.step_eval
  - Error and return semantics (step-eval)
    - Hook signature (step-eval): see: artifacts/code/plugin-trait.rs
    - Return errno and rejection behavior (step-eval)
      - Rejected step → Rollback verdict; host DAG unwinds to parent
      - Missing reviewer attribution → EPERM on commit attempt
      - Zero dirty-byte delta → ERANGE on commit when required
```

---

### `cans/plugins/tool-guard.md` — node additions

```
- Suspend reason ownership (tool-guard)
  - Emits ExternalSuspendKind::External("human-authorization-hold")
  - Approval-hold reason is plugin-private; core treats tag as opaque
  - Resumes only on RFC 6902 patch or explicit deny through SessionWakeHandle
```

---

### `cans/plugins/wire-ingress.md` — node additions

```
- Suspend reason ownership (wire-ingress)
  - Emits ExternalSuspendKind::External("wire-connection-parking")
  - Connection-park reason is plugin-private; core treats tag as opaque
  - Wakes target microVM on inbound frame arrival
```

---

**Deferred to later parts:**

- Fixture keys `checkpoint.step_dag`, `checkpoint.step_evaluation`, `checkpoint.boundaries.sentinel_path`, `checkpoint.boundaries.detect_pty_quiescence` → **Part 7** (moved to `plugins.step_eval.*`; sentinel removed or plugin-private).
- `StepVerdict` / `PostExecAction` type surface in `10-attestation.md` receipts → **Part 6** (typed-extensions cleanup alongside SQL accessor removal).

Part 4/7 ready on request.

# Part 4/7 — Trap Pipeline & Protocol

Canonical post-extraction shape: core `TrapFilter` accepts POSIX process attributes only (`binary`, `argv`, `cwd`, `env`, `uid`). Interpreter unwrapping, MCP schema validation, cross-ring taint, and `vault://` placeholder rejection move to plugins.

---

### `cans/02-plugin-model.md` — node removals

```
@@ #TrapFilter-grammar @@
-  - with_params: precompiled JSON schema validators against MCP tool args
+  - Core predicates: binary | argv | cwd | env | uid (POSIX only)
+  - MCP schema predicates → plugins/mcp.md + plugins/tool-guard.md

@@ #Segregated-ring-traits @@
-  - Dynamic byte-range taint tracking across FUSE → execve → wire egress
+  - Core exposes ring hooks and opaque per-session metadata store
+  - Taint tracking ownership → plugins/taint.md
```

---

### `cans/11-protocol.md` — node removals

```
@@ #Trap-fast-path @@
-  - Hardcoded interpreter unwrapping: python, node, bash, sh, perl, ruby
-  - Hardcoded wrappers: env, xargs, find -exec
-  - Recursive shebang / PT_INTERP parsing
-  - Inspection of -c / -e strings as code
+  - Core resolves raw exec facts: (st_dev, st_ino, binary_blake3,
      argv, envp, pt_interp)
+  - No interpreter knowledge in core; plugin-supply-chain consumes facts

@@ #Trap-decision-invariants @@
-  - Env injection limits reject vault:// placeholders with ESecurityViolation
+  - Core carries no vault:// awareness; placeholder rejection →
      plugins/vault.md via TrapInterposer
```

---

### `cans/plugins/supply-chain.md` — node additions

```diff
--- a/cans/plugins/supply-chain.md
+++ b/cans/plugins/supply-chain.md
@@ -24,6 +24,14 @@
     - Monitors package manager process trees
     - see: artifacts/examples/fixtures/kelir.yaml#plugins.supply_chain.package_guard.managers
     - Inspects binary_path and argv on TrapEvent for package managers (npm, pip)
     - Blocks execution of lifecycle scripts in dependency directories
+    - Interpreter unwrapping and payload inspection
+      - Owns the interpreter table: python, node, bash, sh, perl, ruby
+      - Owns wrapper unwrapping: env, xargs, find -exec
+      - Recursive shebang and PT_INTERP resolution is plugin-private
+      - Inspects -c / -e strings as code when interpreter is known
+      - Consumes core raw facts: (st_dev, st_ino, binary_blake3,
+        argv, envp, pt_interp); core resolves none of this
     - Entropy analysis
```

---

### `cans/plugins/mcp.md` — node additions

```diff
--- a/cans/plugins/mcp.md
+++ b/cans/plugins/mcp.md
@@ -17,6 +17,10 @@
     - Owns all JSON-RPC L7 parsing; core wire plane carries raw L4 only
     - MCP plugin: maps tools directly to virtio-console, virtio-fs, and traps
+    - Owns MCP tool-argument schema matching (with_params surface)
+      - Precompiled JSON schema validators are plugin-private
+      - Compiles against MCP tool manifest at transport bring-up
+      - Verdict dispatch through plugin-tool-guard when taint-bearing
+      - Core TrapFilter carries no schema awareness
     - MCP hook wiring
```

---

### `cans/plugins/tool-guard.md` — node additions

```
- MCP schema matching surface (tool-guard)
  - Accepts with_params predicates forwarded from plugin-mcp
  - Immutable schema lock: pins manifest at handshake; drift → EConfig
  - Semantic rug-pull detection: parameter-name and type diff on resume
  - Verdict from schema mismatch flows into taint escalation lattice
  - Core TrapFilter stays POSIX-only; schema never enters core
```

---

### `cans/plugins/taint.md` — NEW

```
- Plugin taint
  - Crate dependencies (taint)
    - Plugin identifier: plugin-taint
    - Owner crate: crates/kelir-plugins/plugin-taint
    - name() = "taint"
    - Tier 2 cross-ring security governance (taint)
    - Depends on (taint): kelir-interface (ring hooks + metadata store)
    - Exposes: TaintTrackingService
    - Lifecycle attachment (taint): see: 08-lifecycle.md#Lifecycle-vetoes
  - Service exports (taint)
    - Exports TaintTrackingService via PipelineBinder for byte-range taint
  - Invariants and policy (taint)
    - Owns byte-range taint tracking across ring boundaries
      - FUSE read → execve argv/envp → wire egress byte spans
    - Consumes core hooks: FsInterposer, TrapInterposer, WireInterposer
    - Uses opaque per-session metadata store from core for taint labels
    - Taint state is plugin-private; core holds no byte-range semantics
    - Escalation routes to plugin-tool-guard when mutating verbs match
    - see: artifacts/examples/fixtures/kelir.yaml#plugins.taint
  - Error and return semantics (taint)
    - Hook signature (taint): see: artifacts/code/plugin-trait.rs
    - Return errno and rejection behavior (taint)
      - Untrusted ingress byte range → taint label; no errno
      - Tainted mutating verb → delegates verdict to tool-guard
```

---

### `cans/plugins/vault.md` — node additions

```
- vault:// placeholder enforcement (vault)
  - TrapInterposer rejects vault:// in env with ESecurityViolation
  - Resolution path: placeholder → metadata-service fetch at exec
  - Unresolvable placeholder → ENODATA; out-of-scope → EPERM
  - Core Env injection limits carry no vault:// awareness
```

---

**Deferred to later parts:**

- Fixture keys `plugins.supply_chain.package_guard.*`, `plugins.tool_guard.taint_governance`, `plugins.vault.credentials.placeholder` → **Part 7** (already correctly grouped under `plugins.*`; only the prose removal of core-side mirroring lands in Part 7).
- `ExecutionReceipt` typed-extensions cleanup affecting trap facts → **Part 6**.

Part 5/7 ready on request.

# Part 5/7 — Storage Plane

Canonical post-extraction shape: core storage keeps `virtio-fs` + EROFS/DAX + sparse CoW upper broker + `openat2` containment + `flock` bridge + byte/inode quotas. Regex read masking, glob write snoops, and domain-specific synthetic inodes move to plugins via `FsInterposer`.

---

### `cans/05-storage-plane.md` — node removals

```
@@ #In-flight-read-masking @@
-  - Core storage config embeds mask_read.engine (FastRegex | StreamingAutomata)
-  - Core storage config embeds carryover_bytes sliding-window redaction
-  - Core carries Aho-Corasick read-redaction state per open file
+  - Core exposes FsInterposer::on_fs_read with byte buffer + path
+  - Read masking ownership → plugins/redaction.md
+  - Core carries no regex engine and no pattern vocabulary

@@ #Write-snoops @@
-  - Core SnoopRule globs alongside FsInterposer
-  - Core storage plane dispatches write-snoop verdicts
+  - Core exposes FsInterposer::on_fs_write with path + buffer
+  - Write-snoop rule ownership → plugins/redaction.md
+  - Core carries no glob matcher and no snoop table

@@ #Synthetic-FUSE-inodes @@
-  - Core storage config lists .env / .gitconfig synthetic entries
-  - Core domain knowledge of dotenv and git config file shapes
+  - Core provides virtual inode reply: on_fs_lookup -> Action::Patch(FsInode)
+  - Synthetic content ownership → plugins/vault.md / plugins/profile.md
  - Core carries no filename literals and no content templates
```

---

### `cans/plugins/redaction.md` — NEW

```
- Plugin redaction
  - Crate dependencies (redaction)
    - Plugin identifier: plugin-redaction
    - Owner crate: crates/kelir-plugins/plugin-redaction
    - name() = "redaction"
    - Tier 2 storage read/write governance (redaction)
    - Depends on (redaction): kelir-interface (FsInterposer)
    - Exposes: RedactionService
    - Lifecycle attachment (redaction): see: 08-lifecycle.md#Lifecycle-vetoes
  - Service exports (redaction)
    - Exports RedactionService via PipelineBinder for read masking
  - Invariants and policy (redaction)
    - Read masking (FsInterposer::on_fs_read)
      - Engine: FastRegex | StreamingAutomata (plugin-private)
      - carryover_bytes sliding window across 64KB read chunks
      - Rule shape: glob → ordered pattern list → replacement
      - Split-token safety: window overlap covers pattern max length
    - Write snoops (FsInterposer::on_fs_write)
      - Glob-keyed rule set; each rule emits verdict or mutation
      - Verdict flows through unified lattice; core carries no rule table
    - Pattern vocabulary and glob set are plugin-private config
    - Core storage plane delivers raw buffers; zero regex / zero glob
    - see: artifacts/examples/fixtures/kelir.yaml#plugins.redaction
  - Error and return semantics (redaction)
    - Hook signature (redaction): see: artifacts/code/plugin-trait.rs
    - Return errno and rejection behavior (redaction)
      - Mask hit → Pass with mutated byte buffer; no errno
      - Write snoop deny → Verdict::Deny(errno) from unified lattice
      - Engine misconfiguration → EConfig at boot
```

---

### `cans/plugins/vault.md` — node additions

```
- Synthetic inode ownership (vault)
  - Serves .env-style synthetic files through FsInterposer::on_fs_lookup
  - Content template and filename literal are plugin-private config
  - Mode 0o400 enforced plugin-side; core replies with FsInode only
  - In-flight read masking of credential patterns → plugins/redaction.md
  - Core storage carries no filename literals and no content templates
```

---

### `cans/plugins/profile.md` — node additions

```
- Synthetic inode ownership (profile)
  - Serves profile-scoped synthetic files through on_fs_lookup
  - .gitconfig-shape identity files are plugin-private templates
  - Content sourced from builder profile; never from core config
  - Core only replies with Action::Patch(FsInode); no domain shapes
```

---

**Deferred to later parts:**

- Fixture keys `storage.synthetic_files`, `storage.mask_read`, and `storage.*` snoop blocks → **Part 7** (moved to `plugins.vault.synthetic_files`, `plugins.profile.synthetic_files`, `plugins.redaction.mask_read`, `plugins.redaction.write_snoops`).
- `FsInterposer::on_fs_lookup -> Action::Patch(FsInode)` trait shape declaration → **Part 6** alongside `ExecutionReceipt` extension-trait cleanup, so the full interposer surface lands in one place.

Part 6/7 ready on request.


# Part 6/7 — Attestation & Observability

Canonical post-extraction shape: core exposes zero-alloc metric registry, EventBus, and flat `index.bin` logs. OTLP export, statistical feature extraction, surprisal escalation, and SQL typed accessors move to plugins. This part also lands the interposer trait surface deferred from Part 5.

---

### `cans/14-observability.md` — node removals

```
@@ #Metrics-pipeline @@
-  - Native gRPC and HTTP exporters via OpenTelemetry SDK in core
-  - OTLP is the default export surface in core observability
-  - W3C traceparent continuity injected by core
+  - Core exposes zero-alloc metric registry + EventBus + flat index.bin
+  - OTLP export ownership → plugins/ledger.md
+  - W3C traceparent continuity → plugins/ledger.md

@@ #Online-statistical-feature-extraction @@
-  - Core telemetry extracts normalized 16-dimensional feature vectors
-  - Core maintains 300-entry sliding windows of syscall rate / entropy
-  - Core triggers policy escalation on high-surprisal actions
+  - Core emits raw counters and event stream only
+  - Feature extraction and surprisal escalation → plugins/anomaly.md
  - PMU gauges remain core-emitted as raw counter values
```

---

### `cans/10-attestation.md` — node removal

```
@@ #Typed-extensions @@
-  - ExecutionReceipt defines receipt.net_l7_sql() -> Option<&NetL7SqlExt>
-  - Core receipts carry SQL-shaped extension accessors
+  - Core receipt exposes only ext_raw() -> &[u8] plus typed core fields
+  - Domain accessors → extension traits owned by plugins
  - Receipts carry zero knowledge of SQL, HTTP, MCP, or L7 shapes
```

---

### `cans/plugins/ledger.md` — node additions

```
- OTLP export ownership (ledger)
  - Native gRPC and HTTP exporters via OpenTelemetry SDK
  - OTLP is the default export surface when ledger is mounted
  - W3C traceparent continuity across guest-originated requests
  - Core observability carries no exporter and no traceparent injection
  - Core exposes zero-alloc metric registry + EventBus + index.bin only
```

---

### `cans/plugins/anomaly.md` — NEW

```
- Plugin anomaly
  - Crate dependencies (anomaly)
    - Plugin identifier: plugin-anomaly
    - Owner crate: crates/kelir-plugins/plugin-anomaly
    - name() = "anomaly"
    - Tier 3 cross-cutting telemetry aggregator (anomaly)
    - Depends on (anomaly): kelir-interface (EventBus + metric registry)
    - Exposes: AnomalyDetectionService
    - Lifecycle attachment (anomaly): see: 08-lifecycle.md#Lifecycle-vetoes
  - Service exports (anomaly)
    - Exports AnomalyDetectionService via PipelineBinder for escalation
  - Invariants and policy (anomaly)
    - Extracts normalized 16-dimensional feature vectors from telemetry
    - Sliding window (300 entries) over syscall rate, argv entropy, burstiness
    - Surprisal threshold breach triggers policy escalation
    - Computed and policy-bearing metrics are plugin-owned, not core
    - Consumes zero-alloc metric registry and EventBus from core
    - see: artifacts/examples/fixtures/kelir.yaml#plugins.anomaly
  - Error and return semantics (anomaly)
    - Hook signature (anomaly): see: artifacts/code/plugin-trait.rs
    - Return errno and rejection behavior (anomaly)
      - Surprisal breach → escalation event on EventBus; no errno
      - Feature extraction fault → EIO to host caller; window preserved
```

---

### `cans/plugins/db-proxy.md` — node additions

```
- SQL receipt extension (db-proxy)
  - Owns SqlReceiptExt::net_l7_sql(&receipt) extension trait
  - Reads receipt.ext_raw() bytes; core receipt carries no SQL shape
  - Extension trait lives in plugin crate; not in kelir-interface
  - Consumers opt in by importing plugin-db-proxy
```

---

### `cans/02-plugin-model.md` — interposer trait surface (deferred from Part 5)

```
@@ #Segregated-ring-traits @@
+  - FsInterposer::on_fs_read(path, buffer) -> mutated buffer
+  - FsInterposer::on_fs_write(path, buffer) -> Verdict
+  - FsInterposer::on_fs_lookup(path) -> Action::Patch(FsInode)
+  - TrapInterposer::on_trap(event) -> Verdict
+  - ConsoleInterposer::on_console_output(port, bytes) -> mutated bytes
+  - WireInterposer::on_net_outbound(frame) -> FilterAction
+  - Ring hooks are core; verdicts and mutations are plugin-owned
```

---

**Deferred to later parts:**

- Fixture keys `host.telemetry_ring.*` → **Part 7** (moved to `plugins.anomaly.sliding_window_depth`, `plugins.anomaly.export_feature_vector`).
- Fixture keys `plugins.ledger.otel.*` → **Part 7** (retained but re-parented under `plugins.ledger` exclusively; no core mirror).
- Fixture keys `attestation.*` receipt-shape entries → **Part 7** (retained under `attestation.*`; only `plugins.db_proxy.receipts` additions land).

Part 7/7 ready on request.


You're right — the example `see:` targets point at anchors being removed. Those are pointer-only edits (example code is unchanged; interposer surface is stable). Folding them into Part 7 so the split stays at 7.

---

# Part 7/7 — CLI, fixtures, catalog, example pointers

---

### `cans/15-operations.md` — node removal

```
@@ #Subcommands @@
-  - kelir setup hardcodes PATH shims: claude, pytest, python
-  - kelir setup hardcodes keyring migration
+  - kelir setup dispatches plugin setup hooks only
+  - Shim set and migration steps owned by adapter / plugin crates
  - Core CLI carries no tool names and no keyring paths
```

---

### `cans/00-overview.md` — catalog + plugin count

```diff
@@ #Spec-map @@
-    - Plugin model & catalog (13 optional domain plugins): see: 02-plugin-model.md
+    - Plugin model & catalog (20 optional domain plugins): see: 02-plugin-model.md
```

```diff
@@ #What-it-is-not @@
-    - Excludes CLI runners (no kelir run): see: 15-operations.md#Developer-CLI
+    - Excludes CLI runners (no kelir run): see: 15-operations.md#Developer-CLI
+    - Excludes L7, agent, and heuristics concepts: plugins own those
```

---

### `cans/02-plugin-model.md` — catalog

Add to plugin inventory (Tier 2 unless noted):

```
- anomaly        (Tier 3)  — feature extraction + surprisal escalation
- egress-proxy   (Tier 2)  — CONNECT / SOCKS5 handshake
- l7-http        (Tier 2)  — HTTP/SSE/JSON-RPC parse, TLS, 403 receipts
- redaction      (Tier 2)  — read masking + write snoops
- step-eval      (Tier 2)  — StepSlot / StepVerdict / reviewer attribution
- taint          (Tier 2)  — cross-ring byte-range taint
- terminal-guard (Tier 2)  — prompt responder + ANSI disclosures
- token-shield   (Tier 2)  — scrollback clamp
```

---

### `cans/artifacts/examples/fixtures/kelir.yaml` — key relocation

```diff
@@ storage.mask_read @@
-[storage.mask_read]
-  engine: FastRegex
-  carryover_bytes: 31
-  rules:
+[plugins.redaction.mask_read]
+  engine: FastRegex
+  carryover_bytes: 31
+  rules:

@@ storage.synthetic_files @@
-[storage.synthetic_files]
-  - path: "/workspace/repo/.env.local"
+[plugins.vault.synthetic_files]
+  - path: "/workspace/repo/.env.local"
   ...
+[plugins.profile.synthetic_files]
   - path: "/workspace/repo/.gitconfig"

@@ terminal.sanitizer.token_shield / prompts @@
-[terminal.sanitizer.token_shield]  →  [plugins.token_shield]
-[terminal.sanitizer.prompts]       →  [plugins.terminal_guard.prompts]
-[terminal.sanitizer.notice]        →  [plugins.terminal_guard.notice_template]

@@ wire.l7_protocol_inspection / outbox_deferral / quarantine.imds_exception @@
-[wire.l7_protocol_inspection]      →  removed (plugins.l7_http owns)
-[wire.outbox_deferral]             →  [plugins.transaction.outbox_deferral]
-[wire.quarantine.imds_exception]   →  removed (plugins.vault.boot_diversion owns)
-[wire.l4_diversion.upstream_dial]  →  [plugins.egress_proxy.upstream_dial]

@@ checkpoint.step_dag / step_evaluation / boundaries.sentinel_path @@
-[checkpoint.step_dag]              →  [plugins.step_eval.step_dag]
-[checkpoint.step_evaluation]       →  [plugins.step_eval.step_evaluation]
-[checkpoint.boundaries.sentinel_path] → [plugins.step_eval.sentinel_path]
-[checkpoint.boundaries.detect_pty_quiescence] → [plugins.step_eval.pty_quiescence]

@@ host.telemetry_ring @@
-[host.telemetry_ring]              →  [plugins.anomaly]
```

---

### `cans/artifacts/examples/fixtures/kelir.toml` — same key relocation

Mirror `yaml` above: `storage.mask_read`, `storage.synthetic_files`, `terminal.sanitizer.token_shield`, `terminal.sanitizer.prompts`, `wire.outbox_deferral`, `wire.quarantine.imds_exception`, `checkpoint.step_dag`, `checkpoint.step_evaluation`, `host.telemetry_ring` all relocate to `plugins.*`. No new top-level tables.

---

### `cans/artifacts/examples/git-gate.py` / `git-gate.ts` — pointer fix

```diff
--- a/cans/artifacts/examples/git-gate.py
+++ b/cans/artifacts/examples/git-gate.py
@@ -1,5 +1,5 @@
 # see: cans/02-plugin-model.md#Unified-interposition-pipeline
 # see: cans/05-storage-plane.md#Encapsulation-invariant
-# see: cans/05-storage-plane.md#In-flight-read-masking
+# see: cans/plugins/redaction.md#Plugin-redaction
```

Same replacement in `git-gate.ts`.

---

### `cans/artifacts/examples/hook-matrix-full.py` — pointer fix

```diff
--- a/cans/artifacts/examples/hook-matrix-full.py
+++ b/cans/artifacts/examples/hook-matrix-full.py
@@ -1,5 +1,5 @@
 # see: cans/02-plugin-model.md#Full-Verdict-lattice
 # see: cans/02-plugin-model.md#TrapFilter-predicates
-# see: cans/05-storage-plane.md#In-flight-read-masking
-# see: cans/04-terminal-plane.md#Terminal-pipeline
+# see: cans/plugins/redaction.md#Plugin-redaction
+# see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
```

---

### `cans/artifacts/examples/db-backed-runtime.ts` — pointer + accessor fix

```diff
--- a/cans/artifacts/examples/db-backed-runtime.ts
+++ b/cans/artifacts/examples/db-backed-runtime.ts
@@ -1,7 +1,8 @@
 // see: cans/plugins/db-proxy.md#Plugin-db-proxy
 // see: cans/plugins/ledger.md#Plugin-ledger
 // see: cans/plugins/transaction.md#Plugin-transaction
+// see: cans/plugins/db-proxy.md#SQL-receipt-extension
 ...
-// see: cans/10-attestation.md#Typed-extensions
+// receipt.net_l7_sql() moved to SqlReceiptExt::net_l7_sql(&receipt)
```

The call site itself is unchanged (`SqlReceiptExt` is a drop-in extension trait); only the `see:` and one comment line move.

---

### `cans/artifacts/examples/serverless-webhook.py` — pointer fix

```diff
--- a/cans/artifacts/examples/serverless-webhook.py
+++ b/cans/artifacts/examples/serverless-webhook.py
@@ -1,7 +1,8 @@
 # see: cans/08-lifecycle.md#State-machine
 # see: cans/plugins/wire-ingress.md#Plugin-wire-ingress
 # see: cans/plugins/mcp.md#Plugin-mcp
 # see: cans/plugins/tool-guard.md#Plugin-tool-guard
 # see: cans/plugins/ledger.md#Plugin-ledger
+# see: cans/plugins/step-eval.md#Plugin-step-eval
 ...
-# PostExecAction / PostExecAction types: 10-attestation.md
+# PostExecAction is plugin-step-eval-owned; core receipts carry no verdict
```

---

**Unchanged examples (no pointer drift):** `profile.{py,rs,ts}`, `replay.{py,ts}`, `container.{py,ts}`, `fluent-builder-chaos.{py,ts}`, `unified-builder-surface.{py,ts}`, `multi-vm-swarm.py`, `autonomous-swe-agent.*`, `policy-matrix.*`, `profile.*`, `replay.*`. Their `see:` targets (`02-plugin-model.md#Unified-builder`, `08-lifecycle.md#CoW-fork`, plugin leaves) remain valid.

---

## End of split

Total: 7 parts. Every extracted concept now has:
- A canonical plugin home (new or existing).
- A core node that stops at the primitive boundary.
- A fixture block under `plugins.*`.
- Example `see:` pointers retargeted without code change.

One residual: **`cans/artifacts/scorecard.md`** scores per-plane conformance; after these edits its "core purity" row should re-baseline. That is a numbers-only file — I'd treat it as a separate small pass once Part 1–7 land, not part of this diff set.

