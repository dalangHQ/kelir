# `work-order.md`

```markdown
# PARALLEL AGENT EXECUTION ORDER: KELIR DUAL-SPEC REFACTOR (PHASE 2)

## 0. EXECUTIVE DIRECTIVE & CONCURRENCY PROTOCOL
* **Objective:** Execute an atomic, two-repository specification refactor without scaffolding implementation code.
* **Architecture:** Split the monolithic corpus into `kelir` (Public Open-Core) and `kelir-cloud` (Private Commercial & Fleet).
* **Execution Model:** Designed for 4 concurrent, non-overlapping autonomous worker agents coordinated by a synchronization barrier.
* **Hard Constraints:**
  1. Spec-phase only: Generate ZERO `.rs`, `.ts`, or `.py` runtime implementations.
  2. Zero clobbering: Each agent owns an exclusive set of path mutations.
  3. Strict graph integrity: `cans check --refs-only --json` and `cans check --json` must return `ok: true, exitCode: 0` in both repos upon completion.
  4. Audit-slop zero tolerance: Banned terms (`pure epistemic|truth tier|fact-producer|ghost inode|GhostFile|cohesive state duality|token-to-bit|superpowers|teleport`) must never enter either corpus.

---

## 1. AGENT CONCURRENCY & RESPONSIBILITY MATRIX

```
                         [BARRIER 0: REPO SKELETON SETUP]
                                        │
           ┌────────────────────────────┼────────────────────────────┐
           ▼                            ▼                            ▼
  [AGENT 1: PUBLIC SPEC]      [AGENT 2: PUBLIC ARTIFACTS]  [AGENT 3: CLOUD SPEC & ARTIFACTS]
  • kelir/cans/*.md           • kelir/cans/artifacts/*     • kelir-cloud/cans/*.md
  • kelir/cans/_rules.yaml    • config/ (justfile, ci.yml) • kelir-cloud/cans/plugins/*
  • Licenses (FSL, MIT, CC)   • project-structure.txt      • kelir-cloud/cans/artifacts/*
           │                            │                            │
           └────────────────────────────┼────────────────────────────┘
                                        ▼
                         [BARRIER 1: INTEGRATION AUDIT]
                                        │
                             [AGENT 4: ORCHESTRATOR]
                             • Cross-repo cans check
                             • SPDX license compliance
                             • Audit-slop verification
```

| Agent ID | Exclusive Target Scope | Primary Responsibility |
|---|---|---|
| **Agent 0** | Workspace Root | Repository creation, file migrations, directory layouts |
| **Agent 1** | `kelir/cans/*.md`, `kelir/cans/_rules.yaml`, `kelir/LICENSE*` | Public normative spec nodes, licensing, clean-room boundary |
| **Agent 2** | `kelir/cans/artifacts/**/*` | Public justfile, ci.yml, project structure, community examples |
| **Agent 3** | `kelir-cloud/cans/**/*` | Private enterprise spec nodes, cloud architecture, SaaS metering |
| **Agent 4** | Both Repositories | Final barrier verification, JSON assertion of `cans check` |

---

## 2. BARRIER 0: PRE-REQUISITE SETUP (AGENT 0)

Execute before spawning Agents 1, 2, and 3:

```bash
# 1. Establish directory boundaries
mkdir -p kelir kelir-cloud

# 2. Populate public repo base from existing corpus
cp -r cans kelir/
cp -r .github kelir/ 2>/dev/null || true
cp justfile Cargo.toml package.json pnpm-workspace.yaml pyproject.toml kelir/ 2>/dev/null || true

# 3. Initialize private repo specification skeleton
mkdir -p kelir-cloud/cans/artifacts/{config,diagrams,examples}
mkdir -p kelir-cloud/cans/plugins
```

---

## 3. AGENT 1: PUBLIC SPECIFICATION WORKSTREAM

**Target Files (Strictly Exclusive):**
* `kelir/cans/00-overview.md`
* `kelir/cans/02-plugin-model.md`
* `kelir/cans/03-sdk-architecture.md`
* `kelir/cans/07-lifecycle.md`
* `kelir/cans/10-adapters.md`
* `kelir/cans/11-operations.md`
* `kelir/cans/_rules.yaml`
* `kelir/LICENSE-FSL.md`, `kelir/LICENSE-MIT`, `kelir/LICENSE-APACHE`, `kelir/LICENSE-SPEC.md`

### Task 1.1: Licensing Invariant Injection
In `kelir/cans/00-overview.md`, insert under `- Product`:
```markdown
  - Legal and licensing architecture
    - Tri-license topology
      - Spec: Creative Commons Attribution 4.0 International (CC-BY-4.0)
      - Core engine, drivers, and shim: Functional Source License (FSL-1.1-Apache-2.0)
      - Interfaces, SDKs, and testkit: MIT OR Apache-2.0 dual license
      - Enterprise extensions (EE): Commercial proprietary license
    - Two-repo distribution invariant
      - Public repo (kelir): core microkernel, local virtualization, community plugins
      - Private repo (kelir-cloud): fleet compaction, S3 snapshot streaming, enterprise shims
    - Enterprise distribution channels
      - Dynamic SDKs: Precompiled native binaries via npm (@kelir-cloud) and PyPI (kelir-cloud)
      - Rust on-prem: Authenticated private Cargo registry with commercial license tokens
      - Managed platform: Kelir Cloud SaaS execution over HTTPS/gRPC
```

### Task 1.2: Clean-Room Guarantee Injection
In `kelir/cans/02-plugin-model.md`, append under `- Taxonomic coordinate contract`:
```markdown
    - Clean-room boundary invariant
      - kelir-interface is strictly dual-licensed (MIT OR Apache-2.0)
      - Implementing KelirPlugin creates zero copyleft or viral licensing obligations
      - Third-party authors retain 100% intellectual property rights over their plugins
      - Commercial plugins in kelir-cloud link against kelir-interface without licensing conflict
```

### Task 1.3: Catalog Tiering Definition
In `kelir/cans/02-plugin-model.md`, replace `- First-class plugins` catalog entries with:
```markdown
    - Catalog tiering
      - Community plugins (hosted in core repo):
        - pty, vfs, wire, chaos, ledger, mcp, db-proxy, mesh, lock-broker, supply-chain, wire-ingress
      - Commercial plugins (SSOT hosted in kelir-cloud):
        - hyper-density: see: kelir-cloud://cans/plugins/hyper-density.md
        - cas-snapshot: see: kelir-cloud://cans/plugins/cas-snapshot.md
        - oci-translator: see: kelir-cloud://cans/plugins/oci-translator.md
        - openshell-coordinator: see: kelir-cloud://cans/plugins/openshell-coordinator.md
    - Coverage matrix (community plugins -> demonstrating example)
      - pty, vfs, wire, chaos, ledger: autonomous-swe-agent.{rs,ts,py}
      - mesh, lock-broker, supply-chain: multi-vm-swarm.rs
      - db-proxy: db-backed-runtime.py
      - wire-ingress, mcp: serverless-webhook.ts
```

### Task 1.4: Tri-Tier SDK Distribution Invariant
In `kelir/cans/03-sdk-architecture.md`, append under `- N-native design`:
```markdown
      - Three-tier binary distribution model
        - Dynamic runtimes (TS/Python): Zero Rust compilation by end user; ships precompiled
          NAPI-RS .node and PyO3 .whl native shared libraries
        - Pure Rust enterprise: Gated private Cargo registry (crates.kelir.dev) requiring license
          tokens for on-premise compilation ($100k/yr commercial tier)
        - Kelir Cloud SaaS: Managed execution over HTTP/gRPC; internal hyper-density and cas-snapshot
          optimization applied transparently on host servers
```

### Task 1.5: Ingestion Matrix Tiering
In `kelir/cans/10-adapters.md`, append under `- Ingestion matrix`:
```markdown
    - Ingestion tiering and licensing
      - Community adapters: mcp-server (stdio wire), e2b-compat (thin client SDK)
      - Commercial adapters: containerd-shim-v2 (K8s), openshell-grpc (NVIDIA CDI)
      - Commercial adapter SSOT: see: kelir-cloud://cans/01-fleet-orchestration.md
```

### Task 1.6: Public Operations & Decoupling Scope
In `kelir/cans/11-operations.md`:
1. Under `- Matrix job: test-plugins`, set plugin list to exactly:
   `[pty, vfs, wire, chaos, ledger, mcp, db-proxy, mesh, lock-broker, supply-chain, wire-ingress]`.
2. Under `- Six crate families`, replace the plugin crate family description:
   `kelir-plugins: community plugin family (11 plugins; 4 enterprise plugins hosted in kelir-cloud)`.
3. Under `- Decoupling proof`, set audit scope to the 11 community plugins.

### Task 1.7: Public Rules Configuration
In `kelir/cans/_rules.yaml`, update `references:`:
```yaml
references:
  mode: pointer
  back_pointers: false
  max_hops: 1
  orphan_check: true
  duplicate_home_check: true
  external_roots:
    kelir-cloud: "../kelir-cloud/cans"
```

### Task 1.8: Generate License Files
In `kelir/`:
* `LICENSE-SPEC.md`: Creative Commons Attribution 4.0 International.
* `LICENSE-FSL.md`: Functional Source License 1.1 with Apache-2.0 Change License.
* `LICENSE-MIT`: Standard MIT License text.
* `LICENSE-APACHE`: Apache License Version 2.0 text.

---

## 4. AGENT 2: PUBLIC ARTIFACTS WORKSTREAM

**Target Files (Strictly Exclusive):**
* `kelir/cans/artifacts/config/justfile`
* `kelir/cans/artifacts/config/ci.yml`
* `kelir/cans/artifacts/config/project-structure.txt`
* `kelir/cans/artifacts/examples/multi-vm-swarm.rs`
* `kelir/cans/artifacts/examples/serverless-webhook.ts`
* `kelir/cans/artifacts/examples/db-backed-runtime.py`

### Task 2.1: Update `justfile` Plugin Targets
In `kelir/cans/artifacts/config/justfile`:
1. Modify `test-plugins`:
```make
test-plugins:
    cargo nextest run -p plugin-pty -p plugin-vfs -p plugin-wire -p plugin-chaos -p plugin-ledger -p plugin-mcp -p plugin-db-proxy -p plugin-mesh -p plugin-lock-broker -p plugin-supply-chain -p plugin-wire-ingress
```
2. Modify `plugin-check-all` and `test-isolation-all`:
```bash
    for p in pty vfs wire chaos ledger mcp db-proxy mesh lock-broker supply-chain wire-ingress; do
```

### Task 2.2: Update `ci.yml` Matrix Jobs
In `kelir/cans/artifacts/config/ci.yml`:
Update `matrix.plugin` across `test-plugins`, `plugin-audit`, and `plugin-isolation`:
```yaml
matrix:
  plugin: [pty, vfs, wire, chaos, ledger, mcp, db-proxy, mesh, lock-broker, supply-chain, wire-ingress]
```

### Task 2.3: Update `project-structure.txt`
In `kelir/cans/artifacts/config/project-structure.txt`:
1. Under `crates/kelir-plugins/`, remove:
   * `plugin-hyper-density/`
   * `plugin-cas-snapshot/`
   * `plugin-oci-translator/`
   * `plugin-openshell-coordinator/`
2. Under `crates/kelir-adapters/`, remove:
   * `containerd-shim/`
   * `openshell-grpc/`
3. Add a footer:
```text
── KELIR-CLOUD (PRIVATE REPO) ──
crates/kelir-ee-plugins/ (hyper-density, cas-snapshot, oci-translator, openshell-coordinator)
crates/kelir-ee-adapters/ (containerd-shim, openshell-grpc)
services/ (control-plane, api-gateway, snapshot-relay, billing)
```

### Task 2.4: Prune Commercial Invocations from Public Examples
Sanitize public examples so they demonstrate community plugins without depending on commercial plugins:
1. `multi-vm-swarm.rs`: Mount `mesh`, `lock-broker`, `supply-chain`. Replace `openshell_coordinator` with standard session veto API.
2. `serverless-webhook.ts`: Mount `wire-ingress`, `mcp`, `ledger`. Replace `hyperDensity()` with standard `c.vm.adjust_balloon()`.
3. `db-backed-runtime.py`: Mount `db-proxy`. Replace `OciTranslatorPlugin` and `CasSnapshotPlugin` with local EROFS fixtures.

---

## 5. AGENT 3: CLOUD SPECIFICATION & ARTIFACTS WORKSTREAM

**Target Files (Strictly Exclusive):**
* `kelir-cloud/cans/00-cloud-overview.md`
* `kelir-cloud/cans/01-fleet-orchestration.md`
* `kelir-cloud/cans/02-commercial-licensing.md`
* `kelir-cloud/cans/_rules.yaml`
* `kelir-cloud/cans/plugins/hyper-density.md`
* `kelir-cloud/cans/plugins/cas-snapshot.md`
* `kelir-cloud/cans/plugins/oci-translator.md`
* `kelir-cloud/cans/plugins/openshell-coordinator.md`
* `kelir-cloud/cans/artifacts/config/ee-ci.yml`
* `kelir-cloud/cans/artifacts/config/cloud-structure.txt`
* `kelir-cloud/cans/artifacts/examples/` (Full enterprise versions of examples)
* `kelir-cloud/LICENSE`

### Task 3.1: Initialize `kelir-cloud/cans/_rules.yaml`
Write `kelir-cloud/cans/_rules.yaml`:
```yaml
slug:
  algorithm: dash-normalized
  collapse: separator-runs-to-one-hyphen
  case: preserved
structure:
  node_length: { min: 3, max: 90 }
  siblings: { min: 1, max: 20 }
  depth: { min: 2, max: 10 }
  single_child_collapse: false
  empty_nodes: false
content:
  tbd_allowed: true
  max_tbd_per_file: 10
references:
  mode: pointer
  max_hops: 1
  orphan_check: true
  duplicate_home_check: true
  external_roots:
    kelir: "../kelir/cans"
```

### Task 3.2: Create `00-cloud-overview.md`
Write `kelir-cloud/cans/00-cloud-overview.md`:
```markdown
- Cloud Overview
  - Product
    - Name: kelir-cloud
    - Commercial model: SaaS execution platform and Enterprise VPC distribution
    - Boundary rule: see: kelir://cans/00-overview.md#Axiomatic-foundations
  - SaaS Architecture
    - Zero-RAM idle execution: Powered by plugin-hyper-density
    - Content-addressed block streaming: Powered by plugin-cas-snapshot
    - Managed endpoints: REST / JSON-RPC gateway at https://api.kelir.dev
    - Multi-tenant boundary: Per-tenant hardware microVM isolation, zero cgroup bleeding
```

### Task 3.3: Create `01-fleet-orchestration.md`
Write `kelir-cloud/cans/01-fleet-orchestration.md`:
```markdown
- Fleet Orchestration
  - Kubernetes bare-metal runtime
    - containerd-shim-kelir-v2 integrates microVMs directly into K8s Pod lifecycle
    - Translates OCI layers via plugin-oci-translator: see: plugins/oci-translator.md
  - NVIDIA OpenShell integration
    - openshell-grpc mediates compute_driver.proto via UDS
    - Binds GPU devices via CDI: see: plugins/openshell-coordinator.md
```

### Task 3.4: Create `02-commercial-licensing.md`
Write `kelir-cloud/cans/02-commercial-licensing.md`:
```markdown
- Commercial Licensing
  - Distribution channels
    - Precompiled NAPI / PyO3 packages: Delivered via npm/PyPI under commercial EULA
    - Private Cargo Registry: Authenticated access to crates.kelir.dev for pure Rust consumers
    - License enforcement: Cryptographic host entitlement token verification at on_vm_init
```

### Task 3.5: Migrate & Patch Enterprise Plugin Specs
Relocate the 4 plugin files from `kelir/cans/plugins/` to `kelir-cloud/cans/plugins/`:
* `hyper-density.md`
* `cas-snapshot.md`
* `oci-translator.md`
* `openshell-coordinator.md`

In each migrated file, rewrite internal core references to use cross-repo pointers:
* `see: 01-architecture.md#VirtIO-Device-Complex` -> `see: kelir://cans/01-architecture.md#VirtIO-Device-Complex`
* `see: 07-lifecycle.md#Lifecycle-vetoes` -> `see: kelir://cans/07-lifecycle.md#Lifecycle-vetoes`
* `see: 02-plugin-model.md#Core-context-keys` -> `see: kelir://cans/02-plugin-model.md#Core-context-keys`
* `see: artifacts/code/plugin-trait.rs` -> `see: kelir://cans/artifacts/code/plugin-trait.rs`

### Task 3.6: Relocate Enterprise Artifacts
1. Copy original full-scale examples (`multi-vm-swarm.rs`, `serverless-webhook.ts`, `db-backed-runtime.py`) into `kelir-cloud/cans/artifacts/examples/`.
2. Add `kelir-cloud/LICENSE`: Commercial Proprietary text (All Rights Reserved).

---

## 6. BARRIER 1: INTEGRATION VERIFICATION (AGENT 4)

Agent 4 runs only after Agents 1, 2, and 3 report completion.

### Task 4.1: Public Repo Graph & Slop Audit
```bash
cd kelir

# 1. References integrity (<2s)
cans check --refs-only --json | jq -e '.ok and .exitCode == 0'

# 2. Full spec integrity
cans check --json | jq -e '.ok and .exitCode == 0'

# 3. Slop audit (MUST return 0 matches)
git grep -i -E "pure epistemic|truth tier|fact-producer|ghost inode|GhostFile|cohesive state duality|token-to-bit|superpowers|teleport" -- cans/ || true
```

### Task 4.2: Private Repo Graph Audit
```bash
cd ../kelir-cloud

# 1. References integrity across external root kelir://
cans check --refs-only --json | jq -e '.ok and .exitCode == 0'

# 2. Full spec integrity
cans check --json | jq -e '.ok and .exitCode == 0'
```

### Task 4.3: Structural Leak Audit
Verify that NO enterprise code or specs remain in the public repository:
```bash
cd ../kelir
if ls cans/plugins/hyper-density.md cans/plugins/cas-snapshot.md cans/plugins/oci-translator.md cans/plugins/openshell-coordinator.md 2>/dev/null; then
    echo "ERROR: Enterprise plugins leaked into public spec!" >&2
    exit 1
fi
echo "PARALLEL REFACTOR VERIFIED: PASS"
```

---

## 7. DEFINITION OF DONE
1. Both `kelir` and `kelir-cloud` exist as independent directory structures.
2. `cans check --json` outputs `{"ok": true, "exitCode": 0}` in both repositories.
3. Cross-repo URI links (`kelir://` and `kelir-cloud://`) resolve with zero broken pointers.
4. Exactly zero lines of implementation code (`src/*.rs`, `*.ts`, `*.py`) were written.
5. All 11 community plugins pass public spec validation; all 4 commercial plugins pass private spec validation.
```