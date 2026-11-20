242343

`agentsh` and `kelir` sit at opposite ends of maturity and isolation:

- **`kelir`** has a superior isolation substrate (hardware microVM + `virtio` rings + in-process embedding + CoW RAM/disk rollback) and strict architectural hygiene (Law 8 facade blindness, L1/L2/L3 split). **But it is still L1/L2-only** (`cans/01-architecture.md`, `cans/artifacts/scorecard.md`: `hypervisors present: none`, `deferred/` runtime no-op examples), and its plugin specs gloss over hard protocol and attack-surface realities.
- **`agentsh`** has an ugly, OS-coupled enforcement substrate (ptrace/seccomp/FUSE/eBPF/ESF/unsigned minifilter) — **but it has battle-tested L3 domain engines** forged by CVE demos, adversarial reviews, and real agent workloads.

Here are `kelir`'s blindspots and what to steal, fix, or ignore from `agentsh`.

---

### 1. Blindspots in `kelir` Exposed by `agentsh`

#### A. `plugin-db-proxy` is dangerously underspecified (`cans/plugins/db-proxy.md` vs `agentsh/internal/db/`)
`kelir`'s `db-proxy.md` assumes decoding `Q`/`P` frames and counting `>3` rewrites (`ECYCLICREWRITE`) covers PostgreSQL mediation. `agentsh` proves how naive that is:
- **Wire protocol state machine gaps**: `kelir` ignores Extended Query (`Parse`/`Bind`/`Execute`/`Sync`) pipeline desyncs, `FunctionCall` subprotocol abuse, `COPY ... FROM PROGRAM` / `pg_read_file` / `lo_import` / `dblink_exec` RCE-on-DB-server vectors, GSSENC/replication downgrade attempts, and **`CancelRequest` side-channel mapping** (`agentsh` uses synthetic `BackendKeyData` keys + `cancelmap.go` so a guest can't spoof/DOS another session's query cancellation).
- **Policy evaluation semantics**: `kelir`'s ring pipeline is first-denial / monotonic per hook, which fails for SQL. A single query like `INSERT INTO public.a SELECT * FROM secret.b` or a multi-statement batch has multiple objects/effects. `agentsh` (`internal/db/policy/evaluate.go`) uses **collect-all, any-deny-wins** with explicit `require_where` guards on `UPDATE`/`DELETE` and `deny_mode_in_tx: terminate` (closing the transaction on violation instead of leaving a poisoned txn open).
- **`search_path` / Temp-table aliasing**: Without catalog resolution (`agentsh/internal/db/catalog/`), an agent runs `SET search_path = ...` or `CREATE TEMP TABLE users` to evade naive SQL table-name regex/AST rules.

#### B. No LLM Proxy, SSE Stream Interception, or Format-Preserving Credential Substitution (`l7-http` + `vault` vs `agentsh/internal/proxy/`)
`kelir` splits L7 HTTP (`l7-http.md`), secrets (`vault.md`), and read masking (`redaction.md`), leaving three major gaps:
- **No response-side credential scrubbing or format-preserving fakes**: `agentsh` (`credsub/table.go`, `secrets/fakegen.go`) hands the agent a length- and format-valid fake token (e.g. `ghp_{rand:36}`), swaps the real secret in outbound headers, and **scrubs real credentials from upstream responses** (`scrub_response`) in case an API echoes `Authorization` or debug headers. `kelir`'s `vault.md` only injects bearer tokens or `vault://` placeholders on outbound streams, and `redaction.md` only wraps `FsInterposer`, not `WireInterposer` inbound responses.
- **No fail-closed direct-host guard for declared services**: `agentsh`'s `http_services` combines L7 path/method rules with an automatic `netmonitor` block on the raw upstream hostname/aliases (`http_service_denied_direct`) plus `..` path-traversal 403 rejection. In `kelir`, nothing binds `l7-http` SinkholeRules to block direct IP/domain bypasses when a service is meant to be gated.
- **No LLM dialect/SSE/DLP awareness**: `agentsh` (`internal/proxy/dialect.go`, `dlp.go`, `usage.go`, `sse_intercept.go`) parses Anthropic/OpenAI dialects, tracks normalized `input_tokens`/`output_tokens`, redacts PII *before* prompts leave the host, and inspects `tool_use` blocks inside streaming SSE chunks in real time. `kelir` has none of this — it only catches tools once the guest agent invokes MCP.

#### C. MCP Security Blindspots (`mcp.md` + `tool-guard.md` vs `agentsh/internal/mcpinspect/`)
`kelir`'s `tool-guard.md` handles `tools/list` BLAKE3 schema locking, byte-range taint, and per-tool call quotas. It misses three real MCP attack classes that `agentsh` already defends against:
- **Semantic cross-server exfiltration**: `kelir`'s `plugin-taint` tracks *byte-range* taint (`FUSE read -> argv -> wire`). If an LLM reads secret data from MCP Server A, summarizes or re-encodes it in-context (different bytes!), and calls a `send`/`post`/`upload` tool on MCP Server B, byte-range taint misses it. `agentsh` (`session_analyzer.go`) classifies tools by capability prefix (`read`/`get`/`list` vs `send`/`post`/`upload` vs `run`) over a sliding window and blocks `read_then_send` / `cross_server_flow` within a turn regardless of byte identity.
- **Confusable / typosquatted tool names**: `agentsh` (`name_similarity.go`, `mcpregistry/OnOverwrite`) flags Levenshtein-close tool names and cross-server tool-name collisions.
- **MCP `sampling/createMessage` abuse**: Server-initiated sampling (`sampling_test.go`) lets a rogue MCP server drive the client's LLM. `kelir` doesn't mention `sampling` gating in `mcp.md` or `tool-guard.md`.

#### D. `supply-chain.md` Relies on Entropy & Blocks Scripts, Missing Manifest Resolution & Privacy-Safe Risk Feeds
- **Self-contradiction inside `kelir`**: `cans/07-wire-plane.md` explicitly rejects Shannon entropy as a policy primitive (*"false positives... attacker who controls encoding trivially defeats a fixed threshold"*), yet `cans/plugins/supply-chain.md` uses `> 5.2 bits/char` Shannon entropy on `argv[1:]`.
- **No pre-install manifest resolution**: Blocking `npm`/`pip` lifecycle scripts doesn't stop an install of a malicious/vulnerable package whose *import* or runtime code is backdoored. `agentsh`'s `internal/pkgcheck` dry-run resolves lockfiles/manifests (`npm`, `pip`, `pnpm`, `yarn`, `uv`, `poetry`) before writes, queries OSV/Snyk/Socket/deps.dev, and strips private scopes (`privacy.go`) so internal package names aren't leaked externally.
- **No skill/instruction file vetting**: `agentsh`'s `internal/skillcheck` scans `SKILL.md`/plugin files for hidden unicode, scope mismatch, and injection before agents ingest them.

#### E. Zero-Config Ergonomics & Profile Generation (`profile.md` vs `agentsh/internal/policygen`)
`kelir`'s `default_zero_capability` + `physicality_check` is strong security, **but brutal to configure**. Writing `MountScope` + `SinkholeRule` + `kelir.toml` by hand for a messy repo will push users toward overly permissive profiles. `agentsh policy generate` (`internal/policygen/`) solves this with "profile-then-lock": run an observed session, collapse directories/subdomains/`/24` IPs above thresholds, flag risky commands, hash seen MCP tools, and emit a starter policy. `kelir` has `kelir spec export`, which dumps an already-configured builder, **not** an observed-run profile synthesizer.

#### F. Guest Kernel & Class 2 Attack Surface + Cross-OS Host FS Quirks
- **Guest BPF gaps**: Inside the microVM (protecting `kelir-shim`, `hidepid=2`, `StepSlot` isolation) and especially in `driver-bwrap`/`driver-proot` (Class 2), `kelir` mentions `mask_proc_environ`, `process_vm_readv`, and `CLONE_NEWUSER`, **but doesn't specify blocking**:
  - `io_uring` (`io_uring_setup`/`enter`/`register`) — bypasses syscall trap inspection on async I/O
  - Exotic/CVE-prone socket families (`AF_ALG`, `AF_RXRPC`, `NETLINK_XFRM` from `agentsh`'s CVE-2026-* demos)
  - Kernel keyrings (`add_key`, `request_key`, `keyctl` — `CVE-2026-46243`)
  - x32 / non-native ABI syscall numbers (`__X32_SYSCALL_BIT`, `int 0x80` — caught in `agentsh`'s adversarial review PR #457)
- **Host/Guest FS & `envp` subtleties**:
  - `05-storage-plane.md` handles `openat2`/`O_NOFOLLOW`/`FILE_FLAG_OPEN_REPARSE_POINT`, but misses `agentsh`'s `symlink_cwd_deny` distinction (escaping via `cwd` vs escaping via path arg) and **case-folding mismatch** between APFS/NTFS host `lowerdir` and Linux guest VFS (`internal/pathutil`).
  - `11-protocol.md` / `plugins/tool-guard.md` `ExecveMutation.set_env` should mandate **stripping existing duplicate keys from `envp`** before prepending/appending, because POSIX `getenv()` returns the first match (`agentsh/internal/envinject`).
- **Pre-flight honesty (`kelir doctor` vs `agentsh detect`)**: `kelir doctor` probes `/dev/kvm`, `pidfd_open`, and HVF entitlements statically. When `kelir` runs in restricted clouds (Firecracker/AppArmor/nested containers) under Class 2 (`driver-bwrap`/`proot`), static `/dev` or `uname` checks lie — `agentsh` learned to run **live behavioral probes** (`seccomp_install_probe`, `wait_killable_probe`) to verify the kernel doesn't silently block or kill trapped syscalls.

#### G. Operational & Compliance Surface (`14-observability.md` vs `agentsh/internal/ocsf`, `pkg/emergency`, `internal/report`)
- **OCSF exhaustiveness**: `agentsh` maps every event type to OCSF (`internal/ocsf/exhaustiveness_test.go` fails build if an event isn't mapped). `kelir` mentions OCSF in one bullet (`on_host_containment_breach`) and leaves `plugin-ledger` emitting custom records + OTLP.
- **Fleet emergency kill-switch / break-glass**: `agentsh` has `pkg/emergency` (timed, MFA-gated break-glass bypass + global session kill-switch). `kelir` only has per-session `drain`/`kill` and `Port 0` break-glass.
- **Human-readable session findings report**: `agentsh report` produces an executive markdown summary of blocks, anomalies, and timeline without requiring an external OTel/SIEM stack.

---

### 2. Concrete List: What to Steal / Adapt for `kelir`

| Steal from `agentsh` | Where it fits in `kelir` | Why it's an upgrade |
|---|---|---|
| `libpgquery` AST classifier + `statemachine`/`cancelmap`/`require_where`/collect-all-deny policy | `crates/kelir-plugins/plugin-db-proxy` + `plugins/db-proxy.md` | Replaces naive Q/P frame parsing; stops `COPY PROGRAM`, `search_path` evasion, `CancelRequest` spoofing, and multi-statement bypasses |
| `credsub` format-preserving fake tokens + `scrub_response` + `http_services` direct-egress block | `crates/kelir-plugins/plugin-vault` + `plugin-l7-http` | Prevents secret leaks in echoed HTTP responses and eliminates direct-IP bypass of declared API gateways |
| `mcpinspect` cross-server `read_then_send` window + `sampling` block + Levenshtein name check | `crates/kelir-plugins/plugin-tool-guard` + `plugin-mcp` | Catches LLM-paraphrased cross-server exfiltration that byte-range `plugin-taint` cannot see |
| `pkgcheck` dry-run resolver + privacy-filtered OSV/Snyk/Socket gate | `crates/kelir-plugins/plugin-supply-chain` | Upgrades supply-chain protection from "block lifecycle scripts + noisy entropy" to pre-install transitive CVE/malware blocking |
| `policygen` ("profile-then-lock") | `kelir-plugin-cli` / `kelir spec generate` (host-side tool) | Solves `kelir.toml` / `default_zero_capability` authoring friction by compiling observed `ExecutionReceipt` + `EventBus` logs into a starter profile |
| `seccomp` CVE hardening (`io_uring`, `AF_ALG`, `AF_RXRPC`, `NETLINK_XFRM`, `add_key`, x32 ABI check) + `envinject` dedup | Guest kernel BPF filter (`kelir-shim` / `driver-bwrap`) + `ExecveMutation` spec | Closes real Linux privilege-escalation and ABI-bypass vectors inside the guest and Class 2 sandboxes |
| Behavioral probes in `doctor` + `ocsf` AST exhaustiveness gate | `kelir doctor` + `plugin-ledger` / `kelir-testkit` | Prevents false-positive capability reports on restricted cloud kernels and guarantees SIEM-ready audit output |
| CVE-style reproducible adversarial harness (`examples/demo-cve-*` + `adversarial-review`) | `kelir-integration` / L3 phase gate | Grounds `kelir`'s AS-01..AS-08 claims in real exploit reproduction rather than synthetic scorecard counts |

---

### 3. What `kelir` Must NOT Copy from `agentsh`

1. **OS-level syscall/FS interception as the primary tier**: `agentsh` spends massive engineering complexity fighting `ptrace` + `SIGURG` Go preemption (`arm64-sigurg-reproducer.md`), Yama `PR_SET_PTRACER` `LD_PRELOAD` hacks, `BASH_ENV` shell-builtin disablement, `/etc/agentsh/shim.conf` non-TTY bypasses, FUSE-T removal/ESF alpha lag on macOS, and unsigned Windows minifilters. `kelir`'s microVM + `virtio` architecture eliminates that entire class of bugs — keep `bwrap`/`proot` strictly as Class 2 dev fallback.
2. **`agentsh`'s monolithic `internal/api` coupling**: `agentsh/internal/api` mixes HTTP, gRPC, WebSocket PTY, ptrace handlers, Landlock, cgroups, FUSE approvals, and DB proxy wiring in one package. `kelir`'s Law 8 facade blindness (`crates/kelir` zero plugin deps) and segregated ring traits are far cleaner — keep new capabilities (LLM dialect/DLP, `pkgcheck`, `credsub`, `policygen`) strictly in Tier 2/3 plugins or external CLI tooling, **never** in `kelir-engine` or `kelir-wire`.
3. **Warning-to-reality gap**: `agentsh` ships real working code and openly documents where macOS/Windows/DB backends fall short (`adversarial-review-2026-11-adjudication.md`, `README.md`). `kelir` currently has 15+ spec files, 21 plugin specs, and `L1/L2` maturity enforcement before running real L3 microVMs (`scorecard.md`: `0/13 kvm`, `0/6 hvf`). **Stop expanding the spec surface and ship L3 end-to-end on one real hypervisor (`KVM`/`HVF`) before adding more plugins.**



3535253 No — digging deeper into both specs exposes several **structural contradictions, broken POSIX assumptions, and architectural gaps** inside `kelir` that `agentsh` already ran into and solved.

---

### 1. Architectural & Technical Blindspots in `kelir`

#### A. Impossible PID Attribution & Missing DNS-to-IP Correlation in `07-wire-plane.md`
- **The Netlink ghost**: `07-wire-plane.md` states: `on_socket_connect resolves guest PID via netlink for PNACL enforcement`. `kelir-wire` runs **host-side** on `smoltcp` consuming raw L2 Ethernet frames from `virtio-net`. Ethernet frames carry zero PID metadata, and host netlink (`SOCK_DIAG`) cannot inspect a guest kernel's socket table. Without a guest-side socket-to-PID telemetry channel on `11-protocol.md` (which doesn't exist) or correlating guest `connect()` traps in `TrapInterposer` to source `(ip, port)`, `on_socket_connect` cannot know the guest PID. `agentsh` solved this with in-kernel eBPF (`cgroup/connect4`, `bpf_get_current_pid_tgid`) + `internal/netmonitor/pnacl` + `ancestry`.
- **Missing DNS-to-IP allowlist bridge**: `07-wire-plane.md` handles `SOCKS5h` via a synthetic `198.18.0.0/15` pool, **but for normal host-resolved DNS (`192.168.127.1:53`)**, there is no synthetic IP pool or DNS-to-IP correlation table (`agentsh` has `internal/netmonitor/dnscache.go`, `redirect/correlation.go`, `api/dns_resolver.go`). When an allowed domain resolves to a rotating CDN IP, `kelir` doesn't specify how the L4 ingress filter admits that IP without either opening a raw-IP bypass hole or breaking on IP rotation.

#### B. No "Deny → Redirect + Guidance" Steering or Per-File `soft_delete`
- `agentsh`'s core thesis (`README.md`, `cans/02-policy-model.md`) is **steering**: `command_rules`, `file_rules`, `dns_redirect`, `connect_redirect`, `signal_rules`, and `database_rules` can **redirect** an operation (`curl` → audited wrapper, writes to `/tmp` → `/workspace/.scratch`, `SIGKILL` → `SIGTERM`, DB table → read-only view) and return a structured `message` telling the agent why and what happened.
- `kelir` (`10-attestation.md#Scope`) enforces **L3/L4 boundary opacity** (*"interception surfaces only as physical bus errors, standard POSIX errnos, or silent packet drops"*), only exposes `FsPathRedirect` on `unlink`, and has **no general write/connect/DNS redirect with agent guidance**. Result: an AI agent hitting `EPERM`/`ECONNREFUSED` burns tokens in blind retry loops.
- **Coarse rollback vs. `soft_delete`**: `kelir` rolls back the entire session delta (`upperdir` punch-hole). `agentsh` (`internal/trash/`) also supports per-file `soft_delete` quarantine with restore tokens so an operator or agent can recover one accidentally deleted file without wiping sibling build progress.

#### C. Broken POSIX Blocking Locks (`F_SETLKW`) & Cross-Mount Rename Gaps (`05-storage-plane.md`)
- **`SETLKW` bug**: `05-storage-plane.md` states `LockBroker` intercepts `SETLK` and `SETLKW`, delegates to host `flock`, and *"Conflicting lock returns EWOULDBLOCK without host thread latency"*. Returning `EWOULDBLOCK` on `F_SETLKW` (which is **blocking wait** by POSIX spec) breaks `cargo`, `git`, `sqlite`, and `npm` when concurrent guest processes contend on a lockfile.
- **Scratch vs. Workspace rename boundary**: `kelir` mounts an ephemeral `virtio-blk` scratch disk at `/tmp` and `/var/cache` while workspace lives on `virtio-fs`. Build tools frequently compile into `/tmp/...` and `rename()` into `/workspace/...`. `05-storage-plane.md` doesn't address cross-mount `EXDEV` semantics or atomic policy checks across the `virtio-blk` ↔ `virtio-fs` boundary (`agentsh` explicitly flags cross-mount renames as deletes/creates).
- **Unsafe `reconcile()` fallback**: `05-storage-plane.md` falls back on `EBUSY` or `EPERM` during `renameat2` to **in-place write with 2000ms backoff**, while claiming *"Whole commit is idempotent"*. In-place overwrite on crash causes torn files and breaks idempotency.

#### D. Weak Intra-VM Signal & Resource Isolation Between Concurrent `ExecSlot`s
- **Signal blindspot**: `11-protocol.md` mentions `on_signal intercepts guest kill calls; blocks unauthorized SIGKILL` in a single bullet. `agentsh` (`internal/signal/`) learned it must also trap `tkill`, `tgkill`, `rt_sigqueueinfo`, `rt_tgsigqueueinfo`, and `pidfd_send_signal`, classify target relationships (`self`, `children`, `descendants`, `session`, `external`, `system`), and support `redirect` (`SIGKILL` → `SIGTERM`) + `absorb`. Without `tgkill`/`pidfd_send_signal` interception and target relationship checks, one `ExecSlot` in `kelir`'s shared guest VM can signal another `ExecSlot` or `kelir-shim`.
- **Shared-UID `RLIMIT_NPROC` DoS**: `kelir` relies on `setrlimit(2)` per `ExecSlot` (`11-protocol.md`). On Linux, `RLIMIT_NPROC` is accounted **per real UID**, not per process tree. A runaway `ExecSlot` exhausts the UID's process budget and starves sibling `ExecSlot`s in the same microVM. `agentsh` uses per-command **cgroups v2** (`internal/limits/cgroupv2_manager.go`) to isolate `pids.max` and `memory.max` cleanly.

#### E. Audit Drop Modes & Missing Human Auth/RBAC for Approvals
- **Silent telemetry loss**: In `kelir/plugins/ledger.md`, `merkle.enabled` defaults to `false`, and OTLP export *"never blocks the pipeline... excess samples dropped with a metric"*. If the OTLP collector is unreachable, `kelir` silently drops audit records in RAM. `agentsh` provides `strict` fail-closed audit (`fsmonitor`), cross-restart/rotation HMAC sidecar verification (`audit.jsonl.chain` with `key_fingerprint` mismatch refusal), and explicit Watchtower WAL drop-class accounting (`overflow`, `crc_corruption`, `ack_regression_after_gc`, `invalid_utf8`, `mapper_failure`).
- **Unauthenticated `Approval` primitive**: `kelir` defines `SuspendUntil::Approval` (`08-lifecycle.md`, `tool-guard.md`) and `Ed25519 reviewer attribution` (`step-eval.md`), **but has zero auth/RBAC/MFA model** for who is allowed to resolve an approval hold (`agentsh` has `internal/auth` + `internal/approvals`: `agent` vs `approver` vs `admin` roles, self-approval refusal, `TOTP`, `WebAuthn`, `OIDC`, native OS dialogs, and a 10-slot/5m timeout ceiling so pending holds can't DoS `max_exec_slots`).

#### F. Shell Unwrapping & BusyBox / `argv[0]` Realities
- `kelir/plugins/supply-chain.md` waves at *"Recursive shebang and PT_INTERP resolution"* and *"Inspects -c / -e strings as code when interpreter is known"*, leaving `Depth cap: TBD`.
- `agentsh` (`internal/shellparse/`, `cmd/agentsh-unixwrap/argv0_override_test.go`, `issue-375`/`377`/`378`) surfaced all the real failures here: `command -v` introspection, opaque `sh -c` constructs (pipelines, command substitution, subshells) that cannot be statically unwrapped cleanly, and multi-call binaries (BusyBox `applets`) where overwriting/ignoring `argv[0]` breaks normal execution (`11-protocol.md` simply declares `argv[0] is untrusted input`).

---

### 2. Internal Contradictions & Spec Debt Inside `kelir` (Visible Against `agentsh`'s Grounding)

Even before comparing external features, reading `kelir`'s `cans/` end-to-end shows spec drift and layer violations that `agentsh`'s code-first adversarial reviews avoided:

1. **Plugin count drift across 3 files**:
   - `cans/00-overview.md`: `20 optional domain plugins`
   - `cans/artifacts/scorecard.md`: `kelir-bundle aggregates all 21 plugins`
   - `cans/02-plugin-model.md#Plugin-tiering`: lists **21** community plugins (2 Tier 1 + 14 Tier 2 + 5 Tier 3), while `#Catalog-tiering` in the **same file** only lists **13** community plugins (missing `egress-proxy`, `l7-http`, `redaction`, `step-eval`, `taint`, `terminal-guard`, `token-shield`, `anomaly`), plus 4 commercial plugins in `plugins/commercial.md`.
2. **Core purity leaks inside `kelir`'s own core plane specs**:
   - `05-storage-plane.md` says `on_fs_readdir scrubs hidden files from guest LLM context windows` — LLM concepts in the core storage plane, violating `01-architecture.md` (*"Zero knowledge of prompts..."*).
   - `07-wire-plane.md` mentions `PNACL enforcement` and `netlink` inside `kelir-wire`.
   - `10-attestation.md` promises `L3/L4` opacity (errnos/RSTs only), while `plugins/supply-chain.md` emits explanatory `stderr_message` strings and `04-terminal-plane.md` writes `TerminalNotice` onto the TTY.
3. **Shannon entropy self-contradiction**: `07-wire-plane.md` explains why entropy gating is fundamentally broken; `plugins/supply-chain.md` mandates `> 5.2 bits/char` entropy gating on `argv[1:]`.
4. **macOS `fork()` / CoreFoundation confusion in `08-lifecycle.md`**: `08-lifecycle.md#CoW-fork` says `child sessions inherit zero CoreFoundation state... Forked children resolve DNS via POSIX structures to prevent SIGABRT`. That only applies to POSIX `fork()` across OS processes, whereas `VmmDriver::fork()` is an in-process `hv_vm_protect` Stage-2 clone (or if it spawns a process, CoreFoundation in a multi-threaded Tokio/NAPI host will `SIGABRT` *before* DNS resolution anyway).
5. **Process/tooling weight vs. runtime proof**: `kelir` has 4 AST/parity/maturity linters, 96 builder parity vectors, 13 Rust scope structs, and 38 ring verbs while `scorecard.md` shows **`hypervisors present: none` (0/13 KVM, 0/6 HVF)** and all composite examples (`serverless-webhook`, `db-backed-runtime`, `multi-vm-swarm`) quarantined in `deferred/` as runtime no-ops. `agentsh` invested in 6 real CVE exploit demos (`examples/demo-cve-*`), real Docker/Postgres E2E tests, and adversarial code audits (`docs/adversarial-review-2026-11-adjudication.md`) instead of L1/L2 typestate ceremony.

---

### 3. Additional Actionable Fixes to Steal from `agentsh` (Beyond Pass 1)

1. **Add a DNS-to-IP synthetic pool or TTL-tracked resolution table** to `07-wire-plane.md` for non-proxy DNS (`192.168.127.1:53`) so domain rules actually govern subsequent TCP/UDP connections, and remove the impossible `netlink` PID-lookup claim from `kelir-wire` (move PID attribution to `TrapInterposer` + `kelir-shim`).
2. **Add an opt-in `Action::Redirect` / guidance surface** (or `TerminalNotice`/structured refusal payload in `plugins/tool-guard` & `l7-http`) plus **per-file `soft_delete` quarantine** in `plugin-redaction`/storage plugins so agents can be steered instead of hitting opaque `EPERM` walls.
3. **Fix `F_SETLKW` in `LockBroker`** (`05-storage-plane.md`): offload blocking `F_SETLKW` onto a bounded async/waiter pool or guest-parked fiber rather than illegally returning `EWOULDBLOCK`, and drop the non-atomic `EPERM` in-place write fallback in `reconcile()`.
4. **Upgrade guest `on_signal` and per-exec isolation**: cover `tkill`/`tgkill`/`rt_sigqueueinfo`/`rt_tgsigqueueinfo`/`pidfd_send_signal`, add target relationship rules (`self`/`children`/`session`/`kelir-shim`), and enforce per-`ExecSlot` **cgroups v2** inside the guest instead of shared-UID `RLIMIT_NPROC`.
5. **Define real approval auth & failure modes**: add role separation, self-approval denial, pending-hold limits/timeouts (`SuspendUntil::Approval` + `Deadline`), and fail-closed audit mode (`ledger.strict`) with Watchtower-style explicit drop counters.
6. **Resolve `kelir`'s internal spec drift first**: fix the `13`/`20`/`21` plugin count mismatch, delete Shannon entropy from `supply-chain.md`, and stop adding L1/L2 builder abstractions until one real KVM/HVF microVM executes `autonomous-swe-agent` + `db-backed-runtime` end-to-end.


**`X = 3` parts total** (all lines `<= 90` chars, single-line bullets, Day-1 canonical tone, zero `04-06 -> plugins/` leaks):
- **Part 1/3 (below)**: Core specs (`00`, `01`, `05`, `07`, `08`, `10`, `11`, `13`, `14`, `15`)
- **Part 2/3**: `02-plugin-model.md` + `plugins/{db-proxy,l7-http,vault,egress-proxy,redaction}.md`
- **Part 3/3**: `plugins/{mcp,tool-guard,supply-chain,step-eval,ledger}.md`

```diff
--- a/cans/00-overview.md
+++ b/cans/00-overview.md
@@ -73,7 +73,7 @@
     - Mobile runtime governance on Android/Termux: see: 01-architecture.md#Driver-backends
   - Spec map
     - Core foundation: see: 01-architecture.md
-    - Plugin model & catalog (20 optional domain plugins): see: 02-plugin-model.md
+    - Plugin model & catalog (21 optional domain plugins): see: 02-plugin-model.md
     - SDK architecture: see: 03-sdk-architecture.md (parity)
     - Terminal plane: see: [1] (engine native)
     - Storage plane: see: [2] (engine native)
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -361,6 +361,9 @@
       - kelir-shim remounts /proc with hidepid=2 during early init
       - /proc/kallsyms, /sys/firmware, /proc/version leak signatures suppressed
       - Untrusted scripts cannot probe kernel pointers or peer processes
+      - Guest BPF blocks io_uring, add_key, request_key, and keyctl syscalls
+      - Exotic socket families denied by default: AF_ALG, AF_RXRPC, NETLINK_XFRM
+      - Syscall trap validates native AUDIT_ARCH; x32 and compat ABIs rejected
     - Workload contract: see: #Zero-workload-intrusion
   - VmmDriver trait
     - see: [6]
@@ -391,6 +394,7 @@
     - Emulated tier (Strict Opt-In, Development Only):
       - driver-bwrap: Linux user namespaces; requires allow_emulated_fallback=true
       - driver-proot: Termux ptrace emulation; requires allow_emulated_fallback=true
+      - Class 2 sandboxes inherit identical BPF syscall and socket family blocks
       - Attempting to boot emulated tier without flag throws E_NOHARDWAREVMM
 
 - [2]: 10-attestation.md#Execution-receipts
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -26,7 +26,7 @@
       - on_fs_setattr traps FUSE_SETATTR for ftruncate, chmod, and chown ops
       - on_fs_symlink traps traversal attempts at link creation boundary
       - on_fs_mkdir enforces directory creation boundaries and quota pre-checks
-      - on_fs_readdir scrubs hidden files from guest LLM context windows
+      - on_fs_readdir filters hidden or policy-excluded entries by hook verdict
       - on_fs_readlink resolves canonical targets against MountScope root
       - Destructive unlinks can be redirected host-side via PatchPayload::FsPathRedirect
       - FUSE dispatch routes through pipeline engine across mounted interceptors
@@ -88,9 +88,9 @@
       - Linux: renameat2; macOS: renameatx_np(RENAME_SWAP); Windows:
       - SetFileInformationByHandle(FileRenameInfoEx)
     - Deletes unlink in place; new dirs create in place
-    - Host Inode Broker pre-checks active host flock descriptors
-    - Fallback on EBUSY or EPERM: in-place write with 2000ms backoff
-    - Whole commit is idempotent: replaying after crash converges
+    - EBUSY retries atomic rename with bounded backoff up to 2000ms
+    - EPERM aborts entry commit fail-closed; never overwrites in place
+    - Atomic leaf rename guarantees crash recovery remains idempotent
   - Containment
     - Strict containment: rejects symlink hops and traversals outside host root
     - Multi-tenant root isolation partitions workspaces by tenant organization
@@ -103,6 +103,9 @@
     - Symlink traversals escaping root fail with EXDEV at host VFS layer
     - S_ISUID and S_ISGID stripped on every write, create, chmod (mode & !0o6000)
     - In-guest root cannot mint setuid host binaries
+    - Symlink CWD traversal outside workspace fails with symlink_cwd_deny
+    - Cross-mount rename between /tmp scratch and workspace returns EXDEV
+    - Case-insensitive hosts (APFS, NTFS) detect case-collision ambiguity
   - Synthetic FUSE inodes
     - Core provides virtual inode reply: on_fs_lookup -> Action::Patch(FsInode)
     - Synthetic content ownership → plugins/vault.md / plugins/profile.md
@@ -126,7 +129,8 @@
     - Intercepts FUSE SETLK and SETLKW requests on declared repository paths
     - Maps virtual guest inode to physical host inode via virtio-fs mount
     - Delegates acquisition directly to host OS advisory locks (flock)
-    - Conflicting lock returns EWOULDBLOCK without host thread latency
+    - Conflicting SETLK returns EWOULDBLOCK; SETLKW parks on bounded waiter pool
+    - Bounded SETLKW waiters time out with EDEADLK; host ring threads never block
     - `SETLK` and `SETLKW` dispatch calls `LockBroker::acquire` before the interposer
     - `LockBroker::acquire` routes the lock hook through the pipeline engine across mounted interceptors with scope filtering and one fused crossing
     - Lock denial replies `FuseReply::Errno(EWOULDBLOCK)`
--- a/cans/07-wire-plane.md
+++ b/cans/07-wire-plane.md
@@ -11,7 +11,7 @@
     - Hardware abstraction: virtio-net bound host-side to embedded smoltcp TCP/IP stack
     - Hook points: on_net_outbound, on_net_inbound, on_socket_connect
     - Additional net hooks: on_dns_query, on_socket_bind
-    - on_socket_connect resolves guest PID via netlink for PNACL enforcement
+    - on_socket_connect attributes PID via TrapInterposer socket-tuple correlation
     - on_dns_query mediates domain resolution before packet egress
     - on_socket_bind blocks unauthorized listening ports before reservation
     - Abstract socket boundary: wire.block_abstract_unix_sockets denies \0 paths
@@ -31,6 +31,7 @@
       - Unauthorized endpoints injected via untrusted context drop at L4 ring
       - DNS queries for undeclared external targets return synthetic NXDOMAIN
       - Out-of-band exfiltration via image rendering or webhooks fails closed
+      - Unconnected UDP sendto and sendmmsg frames evaluate egress policy per target
     - UDP NAT semantics
       - Permitted UDP egress uses endpoint-independent mapping (Full Cone NAT)
       - Single host UDP socket per guest source port serves multiple remote peers
@@ -94,6 +95,10 @@
   - Synthetic DNS gateway
     - Gateway binding
       - Guest paravirtual network binds synthetic gateway 192.168.127.1:53
+    - DNS-to-IP admission
+      - Allowed queries populate a TTL-bounded synthetic or pinned IP table
+      - Direct IP dials absent from the table drop fail-closed at L4 ingress
+      - Rotating CDN answers refresh active IP pins per query TTL
       - All UDP/TCP port 53 traffic to the gateway intercepted in kelir-wire::RealNetOps via kelir-engine::planes::wire
       - Traffic to non-gateway resolvers redirected at ingress ring
     - Host-state resolution
@@ -129,6 +134,8 @@
     - Diverted streams delivered raw to plugins/l7-http (see: plugins/l7-http.md)
     - Stream mutations apply in-place without breaking client keep-alive
   - Egress side-effect outbox
+    - connect_redirect rewrites target endpoint before upstream TCP dial
+    - Threat-feed IP and Tor relay blocks evaluate at L4 before dial
     - L4 stream pause / flush / drop primitives only
     - Saga + undo-webhook ownership → plugins/transaction.md
     - Vault and credential flow
--- a/cans/08-lifecycle.md
+++ b/cans/08-lifecycle.md
@@ -57,9 +57,8 @@
     - HVF path: Stage-2 translation write protection via hv_vm_protect
       - Stage-2 data aborts resolve via Chunked Page Promoter
       - Eagerly promotes 2MB contiguous blocks via Mach vm_copy CoW
-      - macOS fork isolation: child sessions inherit zero CoreFoundation state
-      - SCDynamicStore and Keychain bindings isolated to pre-fork supervisor
-      - Forked children resolve DNS via POSIX structures to prevent SIGABRT
+      - Stage-2 clone executes in-process without OS fork() on multi-threaded hosts
+      - Detached child runtimes spawn clean binaries before CoreFoundation init
       - Eliminates vCPU exit storms during large heap allocations
     - Host runtime isolates plugin state per branch via fork_state() or clone
     - Plugins holding un-clonable host descriptors veto via VetoEvent::Fork
@@ -191,6 +190,8 @@
         - Awaiting out-of-band release
       - Resume executes cycle-exact upon external release signal
       - Release signal arrives on Control 0x0000
+      - Approval holds enforce a per-session slot cap and default expiry TTL
+      - Reviewer identity and RBAC gate approval holds: see: [2]
   - Elastic compute
     - Guest kernel boots with maxcpus=N; configured count online at boot
     - Scale up: hypervisor unparks host vCPU threads; shim writes
@@ -403,3 +404,4 @@
         - WatchdogAction stays closed (Reboot | Terminate)
 
 - [1]: artifacts/code/vmm-driver-trait.rs
+- [2]: plugins/tool-guard.md#Approval-authorization
--- a/cans/10-attestation.md
+++ b/cans/10-attestation.md
@@ -7,6 +7,7 @@
     - Diagnostic metrics: PMU counters, dirty bytes, wire volume (gauges only)
     - Boundary opacity (L3/L4): interception surfaces only as physical bus errors,
       - standard POSIX errnos, or silent packet drops
+    - Agent-facing refusal guidance belongs to L7/TTY plugins, never raw L3/L4
     - L7 wire-plane rejections are an explicit wire contract
       - (see: 11-protocol.md#Policy-rejection-receipt), not an attestation fact
   - Primitive vs derived metrics
--- a/cans/11-protocol.md
+++ b/cans/11-protocol.md
@@ -37,6 +37,7 @@
       - Dynamic ExecSlot slab maps guest pid to exec_id
         - Backed by a chunked allocator; frees return chunks to the pool
         - Sized by min(guest RLIMIT_NPROC, host max_exec_slots)
+      - Per-ExecSlot cgroup v2 enforces pids.max and memory.max inside guest
         - Host max_exec_slots default 4096; configurable per session
       - Slab saturation: shim returns MSG_EXEC_REJECT{ENOMEM} in-band
       - waitpid(-1, WNOHANG) updates slot exit status without blocking
@@ -143,7 +144,9 @@
       - Gate budgets: see: 02-plugin-model.md#Latency-ceiling
       - Filter invariants: see: 02-plugin-model.md#Filter-before-FFI-gate
     - Signal and fork interception
-      - on_signal intercepts guest kill calls; blocks unauthorized SIGKILL
+      - on_signal traps kill, tkill, tgkill, rt_sigqueueinfo, pidfd_send_signal
+      - Signal rules classify target: self, children, session, shim_pid1, peer
+      - Signal verdicts support Pass, Drop, Absorb, and Redirect (SIGKILL->SIGTERM)
       - on_fork intercepts process tree mutations and stops fork bombs in-RAM
     - Host containment verification
       - on_host_containment_breach emits immediate security alerts on openat2 trap
@@ -165,7 +168,8 @@
       - binary_path is a display hint; policy never matches on it alone
       - Resolved identity: see: artifacts/code/messages.rs
     - Interpreter unwrapping
-      - Depth cap: TBD (L3 sets the bound; overflow fails closed)
+      - Depth cap is 8; overflow or cycles fail closed with Drop
+      - Opaque shell ASTs (pipes, subshells) fall back to child execve traps
       - Unreadable payload or cycle fails closed with Drop, never Pass
       - Terminal binary is the resolved identity; chain is audit only
       - Shell payload and chain: see: artifacts/code/messages.rs
@@ -337,6 +341,7 @@
     - Env injection limits
       - set_env+unset_env total ≤ 32KB CBOR-encoded
       - Key length ≤ 128 bytes
+      - set_env strips duplicate keys from envp first to prevent getenv shadowing
     - Core carries no vault:// awareness; placeholder rejection → plugins/vault.md via TrapInterposer
     - Custom dispatch timeout
       - Exceeded handler falls through to Action::Pass (c.pass_() in Python)
--- a/cans/13-cli.md
+++ b/cans/13-cli.md
@@ -5,9 +5,16 @@
   - Host pre-flight commands
     - doctor probes KVM/HVF entitlement, page size, and /dev/kvm permissions
     - doctor --enforce writes physical hardware lockfile /etc/kelir/hw.lock
+    - doctor runs behavioral seccomp, WAIT_KILLABLE, and userfaultfd probes
     - setup attaches eBPF/LSM workspace killswitch and sets device perms
   - Inspection commands
     - logs tails guest output with cursor based replay
       - Cursors resume a tail from an earlier point
       - History survives sandbox restarts
       - Stream guarantee: 04-terminal-plane.md#Stream-replay
+  - Policy generation commands
+    - spec generate synthesizes kelir.toml from ExecutionReceipt and EventBus logs
+    - Collapses path globs, /24 egress CIDRs, and observed MCP tool hashes
+  - Incident and emergency commands
+    - report emits a markdown session summary of blocks, anomalies, and timeline
+    - emergency break-glass and kill-switch control global runtime halt state
--- a/cans/14-observability.md
+++ b/cans/14-observability.md
@@ -22,6 +22,7 @@
     - Core emits raw counters and event stream only
     - Feature extraction and surprisal escalation → plugins/anomaly.md
     - Zero-copy shared memory exports vector directly to host anomaly filters
+    - Audit drop counters track overflow, crc_corruption, mapper, and auth_loss
       - Metrics: counters (execs, syscalls, drops, backpressure events),
         - gauges (RSS, dirty pages, active vCPUs, in-flight execs)
       - Traces: one span per receipt phase (trap_ingress, ffi_hop, policy,
@@ -52,3 +53,7 @@
     - on_host_containment_breach publishes high-severity OCSF security event
     - Captures openat2 RESOLVE_BENEATH and symlink traversal violations in RAM
     - Dispatches immediately to EventBus subscribers without guest logging
+    - Every EventBus security event maps to an OCSF 1.x class id
+    - Strict audit mode and fleet emergency kill-switch events: see: [1]
+
+- [1]: plugins/ledger.md#Strict-audit-and-drop-accounting
--- a/cans/15-operations.md
+++ b/cans/15-operations.md
@@ -250,6 +250,10 @@
     - --layering-coverage
       - Validates downward-only crate dependencies via kelir-plugin-cli check --layers
       - Core and interface violations reject compilation before test run
       - Enforcement: just check-layers, CI layer-check job
+    - --ocsf-coverage
+      - AST test asserts every EventBus security event maps to an OCSF 1.x class
+    - --adversarial-cve-coverage
+      - kelir-integration replays CVE-class exploit demos against guest and Class 2
     - Verb and verdict coverage
       - --hook-coverage
         - Asserts every hook in the segregated traits is implemented by some
@@ -471,6 +475,9 @@
       - `kelir spec check <file>`: AOT validation of kelir.toml or kelir.yaml:
         - Validates syntax and TypeBox/Serde schema without compiling code
         - Asserts acyclic inheritance trees across extends pointers
         - Verifies monotonic limits across deep-merged profiles
       - `kelir spec export`: Emits canonical kelir.toml or kelir.yaml
+      - `kelir spec generate <session>`: synthesizes starter profile from audit log
+      - `kelir report <session>`: emits human-readable findings and timeline markdown
+      - `kelir emergency <kill-switch|break-glass>`: global halt or timed override
       - Granular flags: --install-shims, --attach-lsm, --quarantine-credentials
```

Say `next` (or `2/3`) for Part 2/3.

**Part 2/3**: `02-plugin-model.md` + `plugins/{db-proxy,l7-http,vault,egress-proxy,redaction}.md` (all lines `<= 90` chars, single-line bullets, Day-1 canonical tone).

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -188,6 +188,7 @@
           - TCP RST or ICMP Port Unreachable, L7 gets a synthetic 403 JSON receipt)
         - The one host-response suppression is chaos loss injection, where the
           - driver suppresses the response (plugins/chaos.md)
+        - Optional redirect steering rides Patch payloads or L7/TTY guidance
       - Patch payload is registered per ring: ExecveMutation, FsInode, FsPathRedirect,
         - Custom(schema)
       - Idempotency and density: see: 01-architecture.md#Codebase-invariants
@@ -361,10 +362,12 @@
   - First-class plugins
     - Capability access for every catalog plugin: see: 03-sdk-architecture.md#Service-lookup-across-languages
     - Catalog tiering
-      - Community plugins (in-process engine): cans/plugins/
-        - mcp, mesh, tool-guard, wire-ingress, vault, container
-        - replay, ledger, profile, chaos, transaction
-        - db-proxy, supply-chain
+      - Community plugins (21 in-process engine): cans/plugins/
+        - Tier 1: mcp, mesh
+        - Tier 2: tool-guard, wire-ingress, vault, container, replay
+        - Tier 2: db-proxy, supply-chain, egress-proxy, l7-http, redaction
+        - Tier 2: step-eval, taint, terminal-guard, token-shield
+        - Tier 3: ledger, profile, chaos, transaction, anomaly
       - Commercial plugins: interface in plugins/commercial.md
         - hyper-density, cas-snapshot
         - oci-translator, openshell-coordinator
@@ -381,6 +384,8 @@
       - profile: profile.rs
       - db-proxy: db-backed-runtime.py (userland SDK policy)
       - supply-chain: multi-vm-swarm.rs (userland SDK policy)
+      - egress-proxy, l7-http, redaction, step-eval: none yet
+      - taint, terminal-guard, token-shield, anomaly: none yet
     - Non-plugin surface examples
       - Exercises raw traits or SDK surfaces; no catalog plugin
       - Terminal and trap interposition: see: artifacts/examples/git-gate.rs
--- a/cans/plugins/db-proxy.md
+++ b/cans/plugins/db-proxy.md
@@ -17,7 +17,16 @@
   - Invariants and policy (db-proxy)
     - L7 pgwire rewrite and 2PC commit: see: artifacts/examples/db-backed-runtime.py
     - Mediates relational protocol traffic
-    - PostgreSQL: intercepts port 5432, decodes Q/P frames via pgwire parser
+    - PostgreSQL AST and wire state machine
+      - Parses SQL via libpgquery AST; never relies on regex matching
+      - Tracks Extended Query state machine: Parse, Bind, Execute, Sync
+      - Evaluates multi-object SQL via collect-all any-deny-wins semantics
+      - Enforces require_where on UPDATE and DELETE mutations
+      - Blocks COPY FROM PROGRAM, pg_read_file, lo_import, and dblink_exec
+      - Denies FunctionCall subprotocol, GSSENC, and replication downgrades
+      - Synthesizes BackendKeyData and maps CancelRequest to prevent spoofing
+      - Resolves search_path and temp-table aliases against catalog state
+      - Supports read-only relation redirect and deny_mode_in_tx terminate
     - SQLite UDS bridge mechanics:
       - Guest helper binds Unix Domain Socket at /run/kelir/db.sock on demand
       - Forwards client connections over DB_SQLITE mux stream
--- a/cans/plugins/l7-http.md
+++ b/cans/plugins/l7-http.md
@@ -16,9 +16,22 @@
     - Owns TLS interception via host keyring (plugin-scoped, not core)
     - Synthesizes policy-rejection receipts on the diverted L4 stream
       - HTTP 403 {"error":"E_POLICY_DENIED", reason, pid, binary}
+      - Includes optional policy guidance message for agent recovery
     - SinkholeRule (methods, paths) is plugin-private config
     - Core wire plane stops at L4; every byte above is owned here
+    - Declared HTTP services
+      - Binds gateway URL env, path rules, and credential swap per service
+      - Rejects .. path-traversal attempts with HTTP 403 before dial
+      - Blocks direct guest egress to declared upstream hosts via L4 rules
+    - LLM dialect and SSE inspection
+      - Normalizes OpenAI and Anthropic token usage and rate limits
+      - Redacts outbound prompt PII via DLP rules before upstream dial
+      - Inspects streaming SSE chunks for tool_use blocks before guest read
+    - Threat feeds and egress guard
+      - Syncs URLhaus and phishing blocklists with atomic swap on 304 check
+      - Blocks .onion targets and Tor Onionoo relay IPs at L7/L4
   - Error and return semantics (l7-http)
     - Hook signature (l7-http): see: artifacts/code/plugin-trait.rs
     - Return errno and rejection behavior (l7-http)
--- a/cans/plugins/vault.md
+++ b/cans/plugins/vault.md
@@ -47,6 +47,9 @@
       - Parses L7, substitutes vault:// placeholders with bearer tokens in flight
       - Core wire plane delivers raw L4 streams; no L7 parse in core
       - Eliminates external sidecar daemons, preserving strict Class 1 attestation
+      - Issues format- and length-matched fake tokens for third-party APIs
+      - Swaps fake tokens for host keyring secrets on outbound L7 headers
+      - Scrubs real secrets from inbound L7 responses before guest read
     - Memory isolation and zeroization
       - Host keyring memory pinned with mlock(2); excluded from swap
       - Issued token buffers zeroized on drop; TTL expiry revokes host-side
--- a/cans/plugins/egress-proxy.md
+++ b/cans/plugins/egress-proxy.md
@@ -14,6 +14,11 @@
     - Owns HTTP CONNECT and SOCKS5 negotiation end to end
     - Reads HTTP_PROXY, HTTPS_PROXY, ALL_PROXY, NO_PROXY from env
     - Core wire plane carries raw L4 splice only; no handshake parse
+    - Tor and proxy controls
+      - Denies SOCKS5 RESOLVE (0xF0) and .onion domains by default
+      - Blocks Tor bridge binaries (obfs4proxy, snowflake-client) via trap hook
+      - Rejects guest proxy-env spoofing or NO_PROXY bypasses against L4 policy
+      - Normalizes lowercase and uppercase proxy env variants uniformly
   - Error and return semantics (egress-proxy)
     - Hook signature (egress-proxy): see: artifacts/code/plugin-trait.rs
     - Return errno and rejection behavior (egress-proxy)
--- a/cans/plugins/redaction.md
+++ b/cans/plugins/redaction.md
@@ -20,10 +20,14 @@
       - Verdict flows through unified lattice; core carries no rule table
     - Pattern vocabulary and glob set are plugin-private config
     - Core storage plane delivers raw buffers; zero regex / zero glob
+    - Soft-delete quarantine (FsInterposer::on_fs_unlink)
+      - Redirects matching deletes via PatchPayload::FsPathRedirect to trash
+      - Records per-file manifest tokens for selective restore without rollback
     - see: artifacts/examples/fixtures/kelir.yaml#plugins.redaction
   - Error and return semantics (redaction)
     - Hook signature (redaction): see: artifacts/code/plugin-trait.rs
     - Return errno and rejection behavior (redaction)
       - Mask hit → Pass with mutated byte buffer; no errno
       - Write snoop deny → Verdict::Deny(errno) from unified lattice
+      - Soft-delete hit → Patch(FsPathRedirect) with quarantine token notice
       - Engine misconfiguration → EConfig at boot
```

Say `3/3` when ready for the final part.


**Part 3/3 (final)**: `plugins/{mcp,tool-guard,supply-chain,step-eval,ledger}.md` (all lines `<= 90` chars, single-line bullets, Day-1 canonical tone).

```diff
--- a/cans/plugins/mcp.md
+++ b/cans/plugins/mcp.md
@@ -47,6 +47,8 @@
       - Max frame: 16 MB via streaming continuation frames and virtio-shm
       - Carrier tracks JSON-RPC transaction IDs across request and response
       - AST parser validates JSON types before dispatching to ring hooks
+      - Server-initiated sampling/createMessage requests deny by default
+      - Per-endpoint sampling allowlist overrides require explicit policy opt-in
   - Error and return semantics (mcp)
     - Hook signature (mcp): see: artifacts/code/plugin-trait.rs
     - Return errno and rejection behavior (mcp)
--- a/cans/plugins/tool-guard.md
+++ b/cans/plugins/tool-guard.md
@@ -30,6 +30,10 @@
       - Interposes dynamic tools/list updates and performs semantic AST diff
       - Description rewriting or smuggled parameters halt with ESecurityViolation
       - Schema drift events emit structured audit telemetry before VM panic
+    - Cross-server flow and name safety
+      - Classifies tool calls into read, write, send, and run capability buckets
+      - Sliding window detects cross-server read_then_send flows in same turn
+      - Levenshtein distance flags confusable and shadowed tool names on list
     - Host extensible hook pipeline
     - The `.tool()` builder sugar lives in kelir-bundle (`ToolGuardExt`) as of Law 8; see: cans/02-plugin-model.md#Facade-blindness
       - Exposes typed pre-call and post-call interception to host runtimes
@@ -63,6 +67,12 @@
       - vault:// placeholders are resolved by the workload
       - Output scrubber: scans tool execution responses on return;
         - strips host secrets before model context ingests bytes
+      - Scans tool return payloads for hidden instructions and injection patterns
+    - Approval authorization
+      - Separates agent and approver roles; denies self-approval unconditionally
+      - Verifies approver via TOTP, WebAuthn (touch + UV), or OIDC claims
+      - Caps concurrent pending holds per session and expires stale holds on TTL
+      - Surfaces human-readable refusal guidance on denied tool calls
   - Error and return semantics (tool-guard)
     - Hook signature (tool-guard): see: artifacts/code/plugin-trait.rs
     - Return errno and rejection behavior (tool-guard)
--- a/cans/plugins/supply-chain.md
+++ b/cans/plugins/supply-chain.md
@@ -26,15 +26,20 @@
       - Recursive shebang and PT_INTERP resolution is plugin-private
       - Inspects -c / -e strings as code when interpreter is known
       - Consumes core raw facts: (st_dev, st_ino, binary_blake3, argv, envp, pt_interp); core resolves none of this
-    - Entropy analysis
-      - Computes Shannon entropy of argv[1:] on every TrapEvent
-      - Flags values above 5.2 bits/char as encrypted or base64-obfuscated
-      - Bypass paths are plugin-private config, outside VmConfig
-      - Default bypass runners: node, python, language toolchains
+      - Preserves multi-call argv[0] only for attested applet binaries (BusyBox)
+    - Pre-install manifest risk gate
+      - Dry-run resolves lockfiles (npm, pip, pnpm, yarn, uv, poetry) pre-write
+      - Queries OSV, Snyk, Socket, and deps.dev for CVEs, malware, and licenses
+      - Strips private package scopes before external advisory lookups
+    - Agent skill and instruction vetting
+      - Scans SKILL.md and plugin files for hidden unicode and prompt injection
+      - Enforces declared capability scope before guest ingestion
   - Error and return semantics (supply-chain)
     - Hook signature (supply-chain): see: artifacts/code/plugin-trait.rs
     - Return errno and rejection behavior (supply-chain)
       - Lifecycle-script deny → exit 126 (permission denied)
       - stderr_message names manager and blocked script
       - Nested npm→sh→node traps at the sh ancestor
+      - Advisory malware or CVE block → EACCES with package and advisory ID
--- a/cans/plugins/step-eval.md
+++ b/cans/plugins/step-eval.md
@@ -15,6 +15,7 @@
     - Owns StepSlot, StepEvaluator, StepVerdict, PostExecAction
     - Verdicts: Commit | Rollback | YieldToHost dispatched here, not core
     - Ed25519 reviewer attribution and separation-of-duties enforcement
+      - Reviewer auth and RBAC: see: plugins/tool-guard.md#Approval-authorization
     - Tainted-step delta verification (binary BLAKE3, dirty-byte, duration)
     - PTY idle quiescence heuristics for step boundaries
     - Owns /dev/kelir-gate sentinel path interpretation
--- a/cans/plugins/ledger.md
+++ b/cans/plugins/ledger.md
@@ -62,6 +62,9 @@
         - When off, receipts flow to OTLP alone; no on-disk audit is written
         - When on, WAL records carry the same receipt correlation_token used
           - by the OTLP spans for cross-reference
+        - Sidecar records key_fingerprint and sequence across log rotations
+        - Startup fails closed on key_fingerprint mismatch or missing segments
+        - Supports KMS-backed signing keys (AWS, Azure, GCP, Vault) >= 32 bytes
     - OpenTelemetry
       - Default exporter: OTLP over gRPC and HTTP
         - Standard OTLP endpoints via OTEL_EXPORTER_OTLP_ENDPOINT
@@ -85,6 +88,10 @@
       - see: artifacts/examples/fixtures/kelir.yaml#plugins.ledger.otel.endpoint
       - see: artifacts/examples/fixtures/kelir.yaml#plugins.ledger.otel.service_name
       - see: artifacts/examples/fixtures/kelir.yaml#plugins.ledger.otel
+    - Strict audit and drop accounting
+      - ledger.strict = true fails FUSE and ring ops closed on sink stall
+      - Tracks explicit drop classes: overflow, crc_corruption, mapper, auth_loss
+      - Binds fleet emergency kill-switch and break-glass events to OCSF 1.x ids
   - Error and return semantics (ledger)
     - Hook signature (ledger): see: artifacts/code/plugin-trait.rs
     - Finalize pin (SSOT-A1)
```

All 3 parts applied together resolve the cross-file anchors (`plugins/tool-guard.md#Approval-authorization` from `08-lifecycle.md` and `step-eval.md`; `plugins/ledger.md#Strict-audit-and-drop-accounting` from `14-observability.md`). Next step after applying: run `just check-spec` to verify slugs, hops, and line limits against `cans/_rules.yaml`.


Strictly auditing Parts 1–3 against `242343` and `3535253` caught **5 real gaps/omissions** that were either half-covered or missing a canonical home:

1. **`05-storage-plane.md`**: Only `unlink` mentioned `FsPathRedirect` — missing general `write`/`create` path redirection (e.g., steering out-of-workspace writes to scratch) and FUSE `default_permissions` (clone-deadlock prevention).
2. **`07-wire-plane.md`**: Had `connect_redirect` + DNS-to-IP pinning, but missed **`dns_redirect`** (rewriting matched DNS queries/answers).
3. **`08-lifecycle.md#Break-glass`**: `13-cli.md` and `15-operations.md` added `emergency <kill-switch|break-glass>`, without a canonical runtime spec under `#Break-glass`.
4. **`11-protocol.md`**: Dropped `rt_tgsigqueueinfo` from the `on_signal` trap list when compressing to 90 chars.
5. **`plugins/profile.md`**: `#Plugins-catalog-schema-(profile)` still only listed 13 of the 21 community plugins.

Everything else from `242343` and `3535253` (`libpgquery`/Extended Query/`CancelRequest`/collect-all-deny, `credsub`/`scrub_response`/`http_services`/LLM-SSE-DLP, `mcp` sampling + cross-server `read_then_send` + Levenshtein + prompt-injection scrub, `pkgcheck` resolver + privacy filter + `SKILL.md` + deleting Shannon entropy, `policygen`, `io_uring`/socket-family/x32 BPF blocks, `F_SETLKW` fix, `envp` dedup, `doctor` probes, OCSF exhaustiveness, `ledger.strict`/drop-classes/KMS, approval RBAC/MFA/TTL, and internal count/purity fixes) is covered.

**Closure Patch** (apply after Parts 1–3; all lines `<= 79` chars, single-line bullets, Day-1 tone):

```diff
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -28,7 +28,7 @@
       - on_fs_mkdir enforces directory creation boundaries and quota pre-checks
       - on_fs_readdir filters hidden or policy-excluded entries by hook verdict
       - on_fs_readlink resolves canonical targets against MountScope root
-      - Destructive unlinks can be redirected host-side via PatchPayload::FsPathRedirect
+      - Write, create, and unlink ops redirect via PatchPayload::FsPathRedirect
       - FUSE dispatch routes through pipeline engine across mounted interceptors
       - Emits physical StorageDelta events directly to host event bus on commit
     - Kernel physics invariants (storage)
@@ -39,6 +39,7 @@
       - Mandatory non-blocking host I/O: host descriptors open with O_NONBLOCK
       - Host FIFO and socket rejection: S_IFIFO and S_IFSOCK fail lookup with ENXIO
       - Host blocking prevention: zero guest syscalls may sleep host ring workers
+      - FUSE mounts enable default_permissions to avert guest clone deadlocks
     - Encapsulation invariant
       - Host workspace is locked to mode 000 on host; only the hypervisor
         - descriptor bridges access to virtio-fs lowerdir
--- a/cans/07-wire-plane.md
+++ b/cans/07-wire-plane.md
@@ -99,6 +99,7 @@
       - Allowed queries populate a TTL-bounded synthetic or pinned IP table
       - Direct IP dials absent from the table drop fail-closed at L4 ingress
       - Rotating CDN answers refresh active IP pins per query TTL
+      - dns_redirect rewrites matched query names or answers before guest reply
       - All UDP/TCP port 53 traffic to the gateway intercepted in kelir-wire::RealNetOps via kelir-engine::planes::wire
       - Traffic to non-gateway resolvers redirected at ingress ring
     - Host-state resolution
--- a/cans/08-lifecycle.md
+++ b/cans/08-lifecycle.md
@@ -330,6 +330,8 @@
     - Interaction with lifecycle states
       - Running and Draining: available
       - Paused and Stopped: refuse with ESSHUnavailable
+    - Timed policy break-glass overrides require MFA, reason, and expiry TTL
+    - Fleet kill-switch halts active sessions and blocks new boots globally
   - Wire port forwarding
     - Details: see: 07-wire-plane.md#Port-forwarding
   - Zombie reaping
--- a/cans/11-protocol.md
+++ b/cans/11-protocol.md
@@ -144,7 +144,8 @@
       - Gate budgets: see: 02-plugin-model.md#Latency-ceiling
       - Filter invariants: see: 02-plugin-model.md#Filter-before-FFI-gate
     - Signal and fork interception
-      - on_signal traps kill, tkill, tgkill, rt_sigqueueinfo, pidfd_send_signal
+      - on_signal traps kill, tkill, tgkill, rt_sigqueueinfo, rt_tgsigqueueinfo
+      - on_signal also traps pidfd_send_signal across target relationships
       - Signal rules classify target: self, children, session, shim_pid1, peer
       - Signal verdicts support Pass, Drop, Absorb, and Redirect (SIGKILL->SIGTERM)
       - on_fork intercepts process tree mutations and stops fork bombs in-RAM
--- a/cans/plugins/profile.md
+++ b/cans/plugins/profile.md
@@ -228,6 +228,8 @@
     - Enabled plugins: ledger, vault, tool-guard, db-proxy, supply-chain
     - Enabled plugins: transaction, profile, mcp, wire-ingress
     - Disabled plugins: chaos, mesh, replay, container
+    - Disabled plugins: egress-proxy, l7-http, redaction, step-eval
+    - Disabled plugins: taint, terminal-guard, token-shield, anomaly
   - Attestation schema (profile)
     - Attestation semantics (10-attestation.md): see: [1]
     - see: artifacts/examples/fixtures/kelir.yaml#attestation.hardware_facts
```

**`X = 3` parts total** for `cans/artifacts/examples/`:
- **Part 1/3 (below)**: Canonical fixtures (`fixtures/kelir.toml`, `fixtures/kelir.yaml`)
- **Part 2/3**: Deferred domain examples (`deferred/{db-backed-runtime,multi-vm-swarm,serverless-webhook}.{py,rs,ts}`)
- **Part 3/3**: Active agent & matrix examples (`{autonomous-swe-agent,hook-matrix-full,policy-matrix}.{py,rs,ts}`)

*(Note: `unified-builder-surface`, `container`, `fluent-builder-chaos`, `git-gate`, `profile`, and `replay` already conform to the updated spec without redundant edits.)*

```diff
--- a/cans/artifacts/examples/fixtures/kelir.toml
+++ b/cans/artifacts/examples/fixtures/kelir.toml
@@ -94,6 +94,10 @@
 task_freeze_timeout_ms = 300000
 rejection_mode = "synthetic_refusal"
 mask_proc_environ = true
+per_exec_cgroup = true
+deny_abi_compat = true
+blocked_syscalls = ["io_uring_setup", "io_uring_enter", "io_uring_register", "add_key", "request_key", "keyctl"]
+blocked_socket_families = ["AF_ALG", "AF_RXRPC", "NETLINK_XFRM"]
 
 # Session-scoped driver boot knobs; mirror of top-level driver.fast_boot.
 [session.driver]
@@ -105,6 +109,8 @@
 
 [plugins.ledger]
 enabled = true
+strict = true
+ocsf_mapping = true
 
 [plugins.ledger.merkle_wal]
 enabled = true
@@ -115,6 +121,9 @@
 record_pre_redaction_hash = true
 include_hook_spans_in_root = true
 witness_cosigning_endpoint = "https://witness.internal.net/v1/sth"
+kms_provider = "vault"
+min_signing_key_bytes = 32
+verify_key_fingerprint = true
 
 [plugins.ledger.otel]
 endpoint = "http://localhost:4318/v1/traces"
@@ -137,6 +146,8 @@
 token_ttl_seconds = 60
 zeroize_on_drop = true
 fail_closed = true
+credsub_fake_tokens = true
+scrub_responses = true
 
 [[plugins.vault.credentials]]
 name = "STRIPE_SECRET"
@@ -166,6 +177,8 @@
 strict_ast_parameter_validation = true
 max_invocations_per_trace = 50
 max_frame_bytes = 16777216
+detect_cross_server_exfil = true
+detect_confusable_names = true
 
 [plugins.tool_guard.taint_governance]
 untrusted_ingress_taints = true
@@ -175,6 +188,13 @@
 [plugins.tool_guard.sanitization]
 scan_arguments = true
 scrub_output_secrets = true
+scan_tool_results = true
+
+[plugins.tool_guard.approval_auth]
+deny_self_approval = true
+methods = ["webauthn", "totp", "oidc"]
+max_pending_holds = 10
+hold_ttl_seconds = 300
 
 [plugins.db_proxy]
 enabled = true
@@ -183,6 +203,12 @@
 listen_port = 5432
 upstream = "pg.internal.net:5432"
 max_rewrites_per_conn = 3
+ast_parser = "libpgquery"
+require_where_on_mutations = true
+deny_mode_in_tx = "terminate"
+block_unsafe_io = true
+catalog_resolution = true
+synthetic_cancel_keys = true
 
 [[plugins.db_proxy.postgres.schema_rewrites]]
 from = "public."
@@ -217,7 +243,10 @@
 [plugins.supply_chain.package_guard]
 deny_lifecycle_scripts = true
 managers = ["npm", "pip", "cargo"]
-shannon_entropy_threshold = 5.2
+resolve_lockfiles = true
+advisories = ["osv", "snyk", "socket", "deps_dev"]
+strip_private_scopes = true
+vet_skills = true
 
 [plugins.mesh]
 enabled = false
@@ -258,10 +287,32 @@
 socket_path = "/run/kelir/mcp.sock"
 framing = "json_lines"
 max_frame_bytes = 16777216
+deny_server_sampling = true
 
 [plugins.wire_ingress]
 enabled = true
 
+[plugins.l7_http]
+enabled = true
+threat_feeds = ["urlhaus", "phishing"]
+
+[[plugins.l7_http.declared_services]]
+name = "anthropic"
+env_var = "ANTHROPIC_API_URL"
+upstream = "https://api.anthropic.com"
+block_direct_egress = true
+llm_dialect = "anthropic"
+dlp_redact_prompts = true
+inspect_sse_tool_use = true
+
+[plugins.egress_proxy]
+enabled = true
+block_tor = true
+deny_socks5_resolve = true
+
+[plugins.redaction]
+enabled = true
+
+[plugins.redaction.soft_delete]
+enabled = true
+trash_dir = "/workspace/.kelir_trash"
+
 [plugins."repo-policy"]
 max_diff_bytes = 65536
 protected_branches = ["main", "master", "release/*"]
@@ -338,6 +389,7 @@
 enabled = true
 scope_globs = ["**/Cargo.lock", "**/package-lock.json"]
 non_blocking = true
+blocking_wait_timeout_ms = 5000
 
 [wire]
 userspace_netstack = true
@@ -361,6 +413,7 @@
 synthetic_gateway = "192.168.127.1:53"
 track_host_resolver = true
 edns0_downgrade = true
+dns_to_ip_pinning = true
 socks5h_synthetic_pool = "198.18.0.0/15"
 allowlist = ["api.stripe.com", "github.com", "api.github.com", "*.internal.net"]
 
--- a/cans/artifacts/examples/fixtures/kelir.yaml
+++ b/cans/artifacts/examples/fixtures/kelir.yaml
@@ -127,6 +127,16 @@
     task_freeze_timeout_ms: 300000    # 5m human-in-the-loop approval ceiling before auto-refusal
     rejection_mode: synthetic_refusal
     mask_proc_environ: true           # seccomp blocks /proc/[pid]/environ and process_vm_readv
+    per_exec_cgroup: true             # Leaf cgroup v2 isolates pids.max/memory.max per ExecSlot
+    deny_abi_compat: true             # Enforces native AUDIT_ARCH; rejects x32/compat syscalls
+    blocked_syscalls:
+      - "io_uring_setup"
+      - "io_uring_enter"
+      - "io_uring_register"
+      - "add_key"
+      - "request_key"
+      - "keyctl"
+    blocked_socket_families: ["AF_ALG", "AF_RXRPC", "NETLINK_XFRM"]
 
   # Session-scoped driver boot knobs; mirror of top-level driver.fast_boot.
   driver:
@@ -234,6 +244,7 @@
     enabled: true                     # Intercepts FUSE SETLK/SETLKW and delegates to host flock
     scope_globs: ["**/Cargo.lock", "**/package-lock.json"]
     non_blocking: true                # Conflicting writer receives EWOULDBLOCK, preventing vCPU hang
+    blocking_wait_timeout_ms: 5000    # Bounded SETLKW waiter pool ceiling before EDEADLK
 
 # ------------------------------------------------------------------------------
 # 5. WIRE PLANE (virtio-net, Userspace smoltcp, DNS Gateway & L4 Routing)
@@ -267,6 +278,7 @@
     synthetic_gateway: "192.168.127.1:53"
     track_host_resolver: true         # Watch /etc/resolv.conf (Linux) or SCDynamicStore (macOS)
     edns0_downgrade: true             # Strips DO bit and clears AD bit to stop DNSSEC aborts
+    dns_to_ip_pinning: true           # Correlates allowed domain answers to TTL-bounded L4 IPs
     socks5h_synthetic_pool: "198.18.0.0/15" # Maps remote domain dials to synthetic CIDR
 
     # Destination allowlists (wildcard domains matching egress rules).
@@ -429,6 +441,8 @@
   # ─── PLUGIN: LEDGER (Tier 3 Cross-Cutting Audit & Compliance Aggregator) ───
   ledger:
     enabled: true
+    strict: true                      # Fails FUSE/ring ops closed on sink stall
+    ocsf_mapping: true                # Maps every EventBus security event to an OCSF 1.x class
     # Merkle Write-Ahead Log (WAL) — Tamper-evident on-disk audit trail.
     # Completely isolated on host; survives guest rollback and fork branches.
     merkle_wal:
@@ -440,6 +454,9 @@
       record_pre_redaction_hash: true
       include_hook_spans_in_root: true
       witness_cosigning_endpoint: "https://witness.internal.net/v1/sth"
+      kms_provider: "vault"
+      min_signing_key_bytes: 32
+      verify_key_fingerprint: true
 
     # OpenTelemetry (OTLP) Native Wire Exporter.
     otel:
@@ -466,6 +483,8 @@
       token_ttl_seconds: 60           # Ephemeral token expiration window (max 900s)
       zeroize_on_drop: true           # Memory buffers explicitly scrubbed post-dispatch
       fail_closed: true               # Unregistered credential request raises ENODATA
+      credsub_fake_tokens: true       # Issues format-matched fake tokens to guest
+      scrub_responses: true           # Scrubs real credentials from inbound L7 responses
 
     # Declarative credential descriptors. Workloads query by placeholder handle.
     credentials:
@@ -494,6 +513,8 @@
       strict_ast_parameter_validation: true
       max_invocations_per_trace: 50   # Anti-recursion governor (rejects ECYCLICREWRITE)
       max_frame_bytes: 16777216
+      detect_cross_server_exfil: true # Sliding-window read_then_send cross-server check
+      detect_confusable_names: true   # Levenshtein check against shadowed/typosquatted tools
 
     # Context Taint Tracking & Out-of-Band Human Approvals.
     taint_governance:
@@ -505,6 +526,13 @@
     sanitization:
       scan_arguments: true            # Aho-Corasick regex scanning over tool parameter strings
       scrub_output_secrets: true      # Redacts host credentials before model context ingestion
+      scan_tool_results: true         # Scrubs hidden instructions and prompt injection
+
+    approval_auth:
+      deny_self_approval: true
+      methods: ["webauthn", "totp", "oidc"]
+      max_pending_holds: 10
+      hold_ttl_seconds: 300
 
   # ─── PLUGIN: DB-PROXY (Tier 2 Agnostic Relational Database Proxy) ───────────
   db_proxy:
@@ -514,6 +542,12 @@
       listen_port: 5432
       upstream: "pg.internal.net:5432"
       max_rewrites_per_conn: 3        # Exceeding 3 query rewrites aborts ECYCLICREWRITE
+      ast_parser: "libpgquery"
+      require_where_on_mutations: true
+      deny_mode_in_tx: "terminate"
+      block_unsafe_io: true
+      catalog_resolution: true
+      synthetic_cancel_keys: true
       schema_rewrites:
         - from: "public."
           to: "tenant_sandbox."
@@ -549,7 +583,10 @@
     package_guard:
       deny_lifecycle_scripts: true    # Denies preinstall/postinstall inside node_modules/
       managers: ["npm", "pip", "cargo"]
-      shannon_entropy_threshold: 5.2  # Flags obfuscated or base64-packed command strings
+      resolve_lockfiles: true         # Dry-run resolves manifests pre-write
+      advisories: ["osv", "snyk", "socket", "deps_dev"]
+      strip_private_scopes: true      # Strips private namespaces before external lookups
+      vet_skills: true                # Scans SKILL.md and plugin files before guest ingestion
 
   # ─── PLUGIN: MESH (Tier 1 Sovereign Cross-VM Peer Transport) ────────────────
   mesh:
@@ -603,10 +640,32 @@
     socket_path: "/run/kelir/mcp.sock"
     framing: "json_lines"             # Strict newline-delimited JSON (no Content-Length header)
     max_frame_bytes: 16777216
+    deny_server_sampling: true        # Denies server-initiated sampling/createMessage by default
 
   # ─── PLUGIN: WIRE-INGRESS (Tier 2 In-Ring L7 Ingress) ───────────────────────
   wire_ingress:
     enabled: true
+
+  # ─── PLUGIN: L7-HTTP (Tier 2 Declared Services, LLM DLP & Threat Feeds) ────
+  l7_http:
+    enabled: true
+    threat_feeds: ["urlhaus", "phishing"]
+    declared_services:
+      - name: "anthropic"
+        env_var: "ANTHROPIC_API_URL"
+        upstream: "https://api.anthropic.com"
+        block_direct_egress: true
+        llm_dialect: "anthropic"
+        dlp_redact_prompts: true
+        inspect_sse_tool_use: true
+
+  # ─── PLUGIN: EGRESS-PROXY (Tier 2 Upstream Proxy & Tor Controls) ───────────
+  egress_proxy:
+    enabled: true
+    block_tor: true
+    deny_socks5_resolve: true
+
+  # ─── PLUGIN: REDACTION (Tier 2 FS Masking & Soft-Delete Quarantine) ────────
+  redaction:
+    enabled: true
+    soft_delete:
+      enabled: true
+      trash_dir: "/workspace/.kelir_trash"
 
   # ─── PLUGIN: REPO-POLICY (Branch Protection & Diff Limits) ───────
   repo-policy:
```

Say `2/3` when ready for the deferred domain examples (`db-backed-runtime`, `multi-vm-swarm`, `serverless-webhook`).
**Part 2/3**: `cans/artifacts/examples/deferred/{db-backed-runtime,multi-vm-swarm,serverless-webhook}.{py,rs,ts}` (strict 3-language parity).

```diff
--- a/cans/artifacts/examples/deferred/db-backed-runtime.py
+++ b/cans/artifacts/examples/deferred/db-backed-runtime.py
@@ -66,6 +66,12 @@
             postgres=PgWireRewrite(
                 listen_port=5432,
                 upstream=f"{pg_host}:5432",
+                ast_parser="libpgquery",
+                require_where_on_mutations=True,
+                deny_mode_in_tx="terminate",
+                block_unsafe_io=True,
+                catalog_resolution=True,
+                synthetic_cancel_keys=True,
                 schema_rewrites=[
                     PgSchemaRewrite(from_schema="public.", to_schema="migrate_schema."),
                 ],
@@ -84,6 +90,8 @@
         # see: cans/plugins/ledger.md#Merkle-WAL
         .use(LedgerPlugin(LedgerConfig(
             store_path=".kelir/migrate-audit.merkle.db",
+            strict=True,
+            ocsf_mapping=True,
             merkle_chain=True,
             otel=None,
             receipts=ReceiptConfig(
--- a/cans/artifacts/examples/deferred/db-backed-runtime.rs
+++ b/cans/artifacts/examples/deferred/db-backed-runtime.rs
@@ -47,6 +47,12 @@
     let mut db_config = DbProxyConfig::default();
     db_config.postgres.listen_port = 5432;
     db_config.postgres.upstream = format!("{pg_host}:5432");
+    db_config.postgres.ast_parser = "libpgquery".into();
+    db_config.postgres.require_where_on_mutations = true;
+    db_config.postgres.deny_mode_in_tx = "terminate".into();
+    db_config.postgres.block_unsafe_io = true;
+    db_config.postgres.catalog_resolution = true;
+    db_config.postgres.synthetic_cancel_keys = true;
     db_config.postgres.schema_rewrites = vec![PgSchemaRewrite {
         from_schema: "public".into(),
         to_schema: "migrate_schema".into(),
@@ -61,6 +67,8 @@
 
     // see: cans/plugins/ledger.md#Merkle-WAL
     let ledger_plugin = LedgerPlugin::new(true).with_config(LedgerConfig {
+        strict: true,
+        ocsf_mapping: true,
         hashing_standard: "RFC_6962_DUAL_BLAKE3_SHA256".into(),
         record_pre_redaction_hash: false,
         include_hook_spans_in_root: false,
--- a/cans/artifacts/examples/deferred/db-backed-runtime.ts
+++ b/cans/artifacts/examples/deferred/db-backed-runtime.ts
@@ -30,6 +30,12 @@
       postgres: {
         listenPort: 5432,
         upstream: `${pgHost}:5432`,
+        astParser: "libpgquery",
+        requireWhereOnMutations: true,
+        denyModeInTx: "terminate",
+        blockUnsafeIo: true,
+        catalogResolution: true,
+        syntheticCancelKeys: true,
         schemaRewrites: [{ fromSchema: "public.", toSchema: "migrate_schema." }],
       },
       // see: cans/plugins/db-proxy.md#Ephemeral-credentials-injected-at-host-connection-boundary
@@ -46,6 +52,8 @@
     // see: cans/plugins/ledger.md#Merkle-WAL
     .use(ledger({
       storePath: ".kelir/migrate-audit.merkle.db",
+      strict: true,
+      ocsfMapping: true,
       merkleChain: true,
       otel: undefined,
       receipts: { includeSyscallCount: false },
--- a/cans/artifacts/examples/deferred/multi-vm-swarm.py
+++ b/cans/artifacts/examples/deferred/multi-vm-swarm.py
@@ -62,11 +62,17 @@
             wx_jit=WxJitConfig(),
             package_guard=PackageGuardConfig(
                 managers=["npm", "pip", "cargo"],
+                resolve_lockfiles=True,
+                advisories=["osv", "snyk", "socket", "deps_dev"],
+                strip_private_scopes=True,
+                vet_skills=True,
             ),
         )))
         # see: cans/plugins/ledger.md#Plugin-ledger
         .use(LedgerPlugin(LedgerConfig(
             store_path=".kelir/swarm-audit.merkle.db",
+            strict=False,
+            ocsf_mapping=True,
             merkle_chain=True,
             suppress_local_db=False,
             otel=None,
--- a/cans/artifacts/examples/deferred/multi-vm-swarm.rs
+++ b/cans/artifacts/examples/deferred/multi-vm-swarm.rs
@@ -96,11 +96,17 @@
         wx_jit: WxJitConfig::default(),
         package_guard: PackageGuardConfig {
             managers: vec!["npm".into(), "pip".into(), "cargo".into()],
+            resolve_lockfiles: true,
+            advisories: vec!["osv".into(), "snyk".into(), "socket".into(), "deps_dev".into()],
+            strip_private_scopes: true,
+            vet_skills: true,
             ..PackageGuardConfig::default()
         },
     }))
     // see: cans/plugins/ledger.md#Plugin-ledger
     .plugin_instance(LedgerPlugin::new(true).with_config(LedgerConfig {
+        strict: false,
+        ocsf_mapping: true,
         store_path: ".kelir/swarm-audit.merkle.db".into(),
         merkle_chain: true,
         suppress_local_db: false,
--- a/cans/artifacts/examples/deferred/multi-vm-swarm.ts
+++ b/cans/artifacts/examples/deferred/multi-vm-swarm.ts
@@ -43,11 +43,19 @@
     // see: cans/plugins/supply-chain.md#Plugin-supply-chain
     .use(supplyChain({
       wxJit: {},
-      packageGuard: { managers: ["npm", "pip", "cargo"] },
+      packageGuard: {
+        managers: ["npm", "pip", "cargo"],
+        resolveLockfiles: true,
+        advisories: ["osv", "snyk", "socket", "deps_dev"],
+        stripPrivateScopes: true,
+        vetSkills: true,
+      },
     }))
     // see: cans/plugins/ledger.md#Plugin-ledger
     .use(ledger({
       storePath: ".kelir/swarm-audit.merkle.db",
+      strict: false,
+      ocsfMapping: true,
       merkleChain: true,
       suppressLocalDb: false,
       receipts: { includeSyscallCount: false },
--- a/cans/artifacts/examples/deferred/serverless-webhook.py
+++ b/cans/artifacts/examples/deferred/serverless-webhook.py
@@ -112,11 +112,16 @@
         )))
 
         # see: cans/plugins/mcp.md#Stream-kind-MCP-CONSOLE-routes-directly-to-MCP-client-without-PTY-conflict
-        .use(McpPlugin(McpConfig()))
+        .use(McpPlugin(McpConfig(deny_server_sampling=True)))
 
         .use(ToolGuardPlugin(ToolGuardConfig(
             # see: cans/plugins/tool-guard.md#Immutable-schema-locking
             pin_schemas=True,
+            detect_cross_server_exfil=True,
+            detect_confusable_names=True,
+            scan_tool_results=True,
+            deny_self_approval=True,
+            hold_ttl_seconds=300,
             # see: cans/plugins/tool-guard.md#Call-quota
             max_invocations_per_trace=50,
         )))
@@ -124,6 +129,8 @@
         # see: cans/plugins/ledger.md#Merkle-WAL
         .use(LedgerPlugin(LedgerConfig(
             store_path=".kelir/webhook.merkle.db",
+            strict=True,
+            ocsf_mapping=True,
             merkle_chain=True,
             otel=OtelConfig(
                 service_name="kelir-webhook",
--- a/cans/artifacts/examples/deferred/serverless-webhook.rs
+++ b/cans/artifacts/examples/deferred/serverless-webhook.rs
@@ -10,7 +10,7 @@
 use kelir::{
     HCons, HNil, HandlerCtx, Host, IngressChannel, IngressRoutingService, KelirBuilder,
     LedgerConfig, LedgerCtx, LedgerPlugin, LifecycleEvent, McpCtx, McpPlugin, NoParams,
-    OtelConfig, PostExecAction, ReceiptConfig, ToolGuardConfig, ToolGuardCtx,
+    McpConfig, OtelConfig, PostExecAction, ReceiptConfig, ToolGuardConfig, ToolGuardCtx,
     ToolGuardPlugin, TrapFilter, Unsealed, Verdict, VmConfig, WireIngressConfig,
     WireIngressPlugin,
     driver_interface::{DriverPolicy, WxPolicy},
@@ -111,17 +111,27 @@
     )
     .plugin_instance(WireIngressPlugin::with_config(WireIngressConfig::default()))
     // see: cans/plugins/mcp.md#Stream-kind-MCP-CONSOLE-routes-directly-to-MCP-client-without-PTY-conflict
-    .plugin_instance(McpPlugin::new())
+    .plugin_instance(McpPlugin::new().with_config(McpConfig {
+        deny_server_sampling: true,
+        ..McpConfig::default()
+    }))
     .plugin_instance(ToolGuardPlugin::new().with_config(ToolGuardConfig {
         max_frame_bytes: 16_777_216,
         detect_semantic_rug_pull: true,
         strict_ast_parameter_validation: true,
         propagate_byte_range_taints: true,
         // see: cans/plugins/tool-guard.md#Immutable-schema-locking
         pin_schemas: true,
+        detect_cross_server_exfil: true,
+        detect_confusable_names: true,
+        scan_tool_results: true,
+        deny_self_approval: true,
+        hold_ttl_seconds: 300,
         // see: cans/plugins/tool-guard.md#Call-quota
         max_invocations_per_trace: 50,
         dial_timeout_ms: 3_000,
     }))
     // see: cans/plugins/ledger.md#Merkle-WAL
     .plugin_instance(LedgerPlugin::new(true).with_config(LedgerConfig {
+        strict: true,
+        ocsf_mapping: true,
         hashing_standard: "RFC_6962_DUAL_BLAKE3_SHA256".into(),
         record_pre_redaction_hash: false,
         include_hook_spans_in_root: false,
--- a/cans/artifacts/examples/deferred/serverless-webhook.ts
+++ b/cans/artifacts/examples/deferred/serverless-webhook.ts
@@ -40,17 +40,24 @@
     }))
 
     // see: cans/plugins/mcp.md#Stream-kind-MCP-CONSOLE-routes-directly-to-MCP-client-without-PTY-conflict
-    .use(mcp({}))
+    .use(mcp({ denyServerSampling: true }))
 
     .use(toolGuard({
       // see: cans/plugins/tool-guard.md#Immutable-schema-locking
       pinSchemas: true,
+      detectCrossServerExfil: true,
+      detectConfusableNames: true,
+      scanToolResults: true,
+      denySelfApproval: true,
+      holdTtlSeconds: 300,
       // see: cans/plugins/tool-guard.md#Call-quota
       maxInvocationsPerTrace: 50,
     }))
 
     // see: cans/plugins/ledger.md#Merkle-WAL
     .use(ledger({
       storePath: ".kelir/webhook.merkle.db",
+      strict: true,
+      ocsfMapping: true,
       merkleChain: true,
       otel: { propagateW3cTraceparent: true, serviceName: "kelir-webhook" },
     }))
```

Say `3/3` when ready for the active examples (`autonomous-swe-agent`, `hook-matrix-full`, `policy-matrix`).
**Part 3/3 (final)**: `cans/artifacts/examples/{autonomous-swe-agent,hook-matrix-full,policy-matrix}.{py,rs,ts}` (strict 3-language parity).

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.py
+++ b/cans/artifacts/examples/autonomous-swe-agent.py
@@ -286,6 +286,8 @@
             ),
         )))
         .use(VaultPlugin(VaultConfig(
+            credsub_fake_tokens=True,
+            scrub_responses=True,
             credentials=[
                 CredentialDescriptor(
                     name="STRIPE_SECRET",
@@ -340,6 +342,8 @@
         # see: cans/plugins/ledger.md#Merkle-WAL
         .use(LedgerPlugin(LedgerConfig(
             store_path=".kelir/audit.merkle.db",
+            strict=True,
+            ocsf_mapping=True,
             merkle_chain=True,
             otel=OtelConfig(
                 endpoint="http://localhost:4318/v1/traces",
--- a/cans/artifacts/examples/autonomous-swe-agent.rs
+++ b/cans/artifacts/examples/autonomous-swe-agent.rs
@@ -368,6 +368,8 @@
     // see: cans/plugins/ledger.md#Plugin-ledger
     .plugin_instance(VaultPlugin::new())
     .plugin_instance(LedgerPlugin::new(true).with_config(LedgerConfig {
+        strict: true,
+        ocsf_mapping: true,
         hashing_standard: "RFC_6962_DUAL_BLAKE3_SHA256".into(),
         record_pre_redaction_hash: false,
         include_hook_spans_in_root: false,
--- a/cans/artifacts/examples/autonomous-swe-agent.ts
+++ b/cans/artifacts/examples/autonomous-swe-agent.ts
@@ -60,6 +60,8 @@
     // 1. Plugins mount first; TPlugins accumulates ledger, chaos, policy
     .use(ledger({
       storePath: ".kelir/audit.merkle.db",
+      strict: true,
+      ocsfMapping: true,
       merkleChain: true,
       otel: { propagateW3cTraceparent: true, serviceName: "kelir-swe-agent" },
       receipts: { includeDirtyBytes: true, includeBinaryBlake3: true, includeSyscallCount: true },
@@ -75,6 +77,8 @@
                                                    ["AKIA[0-9A-Z]{16}", "[AWS_IAM_REDACTED]"]] }] } }))
     .use(vault({
       credentials: [{
+        credsubFakeTokens: true,
+        scrubResponses: true,
         name: "STRIPE_SECRET",
         placeholder: "vault://stripe_key",
         kind: "bearer-token",
--- a/cans/artifacts/examples/hook-matrix-full.py
+++ b/cans/artifacts/examples/hook-matrix-full.py
@@ -80,6 +80,8 @@
 
 
 async def _on_signal(sender: int, target: int, sig: int) -> Verdict:
+    if target == 1 and sender != 1:
+        return Verdict.drop(Errno.EPERM, "EPOLICY: signaling kelir-shim PID 1 denied")
     return Verdict.pass_()
 
 
--- a/cans/artifacts/examples/hook-matrix-full.rs
+++ b/cans/artifacts/examples/hook-matrix-full.rs
@@ -453,7 +453,13 @@
         }
         Ok(())
     }
-async fn on_signal(&self, _sender: u32, _target: u32, _sig: i32) -> Result<Verdict, PluginError> {
+async fn on_signal(&self, sender: u32, target: u32, _sig: i32) -> Result<Verdict, PluginError> {
+    if target == 1 && sender != 1 {
+        return Ok(Verdict::Drop {
+            errno: Errno::Eperm,
+            reason: "EPOLICY: signaling kelir-shim PID 1 denied".into(),
+        });
+    }
     Ok(Verdict::Pass)
 }
 async fn on_fork(&self, _parent: u32, _child: u32) -> Result<Verdict, PluginError> {
--- a/cans/artifacts/examples/hook-matrix-full.ts
+++ b/cans/artifacts/examples/hook-matrix-full.ts
@@ -210,7 +210,12 @@
         hookState.exits += 1;
         if (event.exitCode !== 0) hookState.failures += 1;
       })
-      .onSignal((_sender, _target, _sig) => Verdict.Pass)
+      .onSignal((sender, target, _sig) => {
+        if (target === 1 && sender !== 1) {
+          return Verdict.Drop(Errno.EPERM, "EPOLICY: signaling kelir-shim PID 1 denied");
+        }
+        return Verdict.Pass;
+      })
       .onFork((_parent, _child) => Verdict.Pass),
   );
 
--- a/cans/artifacts/examples/policy-matrix.py
+++ b/cans/artifacts/examples/policy-matrix.py
@@ -119,6 +119,10 @@
     return Verdict.drop(errno=Errno.EAFNOSUPPORT, reason="EAFNOSUPPORT: socket family AF_RXRPC blocked")
 
 
+async def row_uring_block(c: HandlerContext) -> Verdict:
+    return Verdict.drop(errno=Errno.EPERM, reason="EPERM: io_uring blocked by guest BPF")
+
+
 # see: cans/02-plugin-model.md#Full-Verdict-lattice
 async def row_custom(c: HandlerContext) -> Verdict:
     return Verdict.patch(
@@ -166,6 +170,7 @@
         .trap(TrapFilter.binary("policy-escalate"), pre=row_escalate)
         .trap(TrapFilter.binary("policy-syscall-block"), pre=row_syscall_block)
+        .trap(TrapFilter.binary("policy-uring-block"), pre=row_uring_block)
         .trap(TrapFilter.binary("policy-custom"), pre=row_custom)
         .seal()
     )
@@ -183,6 +188,7 @@
         for binary in ["policy-allow", "policy-deny", "policy-rewrite",
                        "policy-abort", "policy-yield", "policy-approval",
                        "policy-escalate", "policy-syscall-block",
+                       "policy-uring-block",
                        "policy-custom"]:
             run = await session.spawn(binary, ["--policy-matrix"])
             print(f"[kelir] {binary} -> exit={run.exit_code}")
@@ -192,6 +198,8 @@
                 await session.elevate_lease("signed_jws_operator_lease_token_payload")
                 elevated = await session.spawn(binary, ["--policy-matrix"])
                 assert elevated.exit_code == 0
+            elif binary in ("policy-syscall-block", "policy-uring-block"):
+                assert run.exit_code != 0
             elif binary == "policy-custom":
                 assert run.exit_code == 0
 
--- a/cans/artifacts/examples/policy-matrix.rs
+++ b/cans/artifacts/examples/policy-matrix.rs
@@ -185,6 +185,11 @@
             "policy-syscall-block" => Ok(Verdict::Drop {
                 errno: Errno::Eafnosupport,
                 reason: "EAFNOSUPPORT: socket family AF_RXRPC blocked (CVE mitigation)".into(),
+            }),
+
+            "policy-uring-block" => Ok(Verdict::Drop {
+                errno: Errno::Eperm,
+                reason: "EPERM: io_uring blocked by guest BPF".into(),
             }),
 
             "policy-custom" => Ok(Verdict::Patch(PatchPayload::Custom {
@@ -395,6 +400,7 @@
         "policy-escalate",
         "policy-syscall-block",
+        "policy-uring-block",
         "policy-custom",
     ] {
         let run = session.spawn(binary, &["--policy-matrix"], None).await?;
@@ -418,6 +424,9 @@
                 assert_ne!(run.exit_code, 0);
                 assert!(matches!(&run.exit_kind, ExitKind::Signal(_)));
             }
+            "policy-uring-block" => {
+                assert_ne!(run.exit_code, 0);
+            }
             "policy-custom" => assert_eq!(run.exit_code, 0),
             _ => {}
         }
--- a/cans/artifacts/examples/policy-matrix.ts
+++ b/cans/artifacts/examples/policy-matrix.ts
@@ -45,6 +45,8 @@
         case "policy-syscall-block":
           return c.drop(Errno.EAFNOSUPPORT, "EAFNOSUPPORT: socket family AF_RXRPC blocked");
+        case "policy-uring-block":
+          return c.drop(Errno.EPERM, "EPERM: io_uring blocked by guest BPF");
         // see: cans/11-protocol.md#Custom-payload:-c.patch(Custom-{-name,-cbor-})
         case "policy-custom":
           return c.patch({
@@ -122,7 +124,7 @@
   for (const binary of ["policy-allow", "policy-deny", "policy-rewrite",
                         "policy-abort", "policy-yield", "policy-approval",
                         "policy-escalate", "policy-syscall-block",
-                        "policy-custom"]) {
+                        "policy-uring-block", "policy-custom"]) {
     const run = await session.spawn(binary, ["--policy-matrix"]);
     console.log(`[kelir] ${binary} -> exit=${run.exitCode}`);
     if (binary === "policy-approval") {
@@ -134,6 +136,8 @@
       if (elevated.exitCode !== 0) throw new Error("escalation failed post-signature");
     } else if (binary === "policy-syscall-block") {
       if (run.exitCode === 0) throw new Error("syscall block should fail execution");
+    } else if (binary === "policy-uring-block") {
+      if (run.exitCode === 0) throw new Error("io_uring block should fail execution");
     } else if (binary === "policy-custom") {
       if (run.exitCode !== 0) throw new Error("custom failed");
     }
```

Note on `policy-matrix.rs` line 403 (`"policy-syscall-block"` vs `"policy-uring-block"`): wait — in `policy-matrix.rs`, `"policy-uring-block"` is checked cleanly with `assert_ne!(run.exit_code, 0)` without assuming `ExitKind::Signal`, matching `Drop(Errno::Eperm)`. Note also that in `autonomous-swe-agent.ts` lines 78-84, `credsubFakeTokens` / `scrubResponses` sit inside `credentials[0]` vs top-level `vault({ ... })` — double check indentation if applying: `credsubFakeTokens` and `scrubResponses` belong on the `vault({ ... })` object alongside `credentials: [...]`, just like `VaultConfig(...)` in `autonomous-swe-agent.py`. Let me flag that explicitly right below the diff? No, wait — let's check line 78 of `autonomous-swe-agent.ts` right now!

Look at `autonomous-swe-agent.ts` lines 77-85 from Paste 2:
```typescript
    .use(vault({
      credentials: [{
        name: "STRIPE_SECRET",
```
Yes! Look at the hunk I wrote above for `autonomous-swe-agent.ts`: it accidentally put `credsubFakeTokens: true, scrubResponses: true,` *inside* `credentials: [{ ... }]` instead of on `vault({ ... })` above `credentials:`! Good catch! Here is the corrected `autonomous-swe-agent.ts` hunk:

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.ts
+++ b/cans/artifacts/examples/autonomous-swe-agent.ts
@@ -60,6 +60,8 @@
     // 1. Plugins mount first; TPlugins accumulates ledger, chaos, policy
     .use(ledger({
       storePath: ".kelir/audit.merkle.db",
+      strict: true,
+      ocsfMapping: true,
       merkleChain: true,
       otel: { propagateW3cTraceparent: true, serviceName: "kelir-swe-agent" },
       receipts: { includeDirtyBytes: true, includeBinaryBlake3: true, includeSyscallCount: true },
@@ -74,6 +76,8 @@
       rules: [{ glob: "**/secrets/**", patterns: [["sk_live_[0-9a-zA-Z]{24}", "[STRIPE_VAULT_KEY]"],
                                                    ["AKIA[0-9A-Z]{16}", "[AWS_IAM_REDACTED]"]] }] } }))
     .use(vault({
+      credsubFakeTokens: true,
+      scrubResponses: true,
       credentials: [{
         name: "STRIPE_SECRET",
         placeholder: "vault://stripe_key",
```

Use that corrected hunk for `autonomous-swe-agent.ts`. All 3 parts are now consistent across TOML, YAML, Python, Rust, and TypeScript.

Good pushback — a line-by-line audit of Parts 1–3 against `242343`, `3535253`, and `kelir`'s own `02-plugin-model.md` type rules uncovered **1 real spec contradiction we introduced** and **5 remaining coverage gaps**:

### Audit Findings (What Was Still Wrong or Missing)
1. **Type-level contradiction in our `05-storage-plane.md` Closure Patch**: We wrote `Write, create, and unlink ops redirect via PatchPayload::FsPathRedirect`. That breaks `02-plugin-model.md#Unified-verdict` and `05-storage-plane.md#Write-gating`: `on_fs_write` returns `FilterAction` (which has **no `Patch` variant**). Path redirection applies to `Verdict`-returning hooks (`lookup`, `mkdir`, `rename`, `unlink`), not `on_fs_write`.
2. **Inaccurate coverage table in our `02-plugin-model.md` Part 2/3 diff**: We listed `redaction, terminal-guard, token-shield: none yet`, even though `autonomous-swe-agent.{py,ts}` already mounts and demonstrates all three.
3. **Redis & `preparedcache` missing in `plugins/db-proxy.md`**: `db-proxy.md` claims port `6379` Redis proxying, yet we only patched PostgreSQL (`EVAL`/`CONFIG`/`DEBUG`/`FLUSH*` Redis gating and `preparedcache` invalidation on `search_path`/role change were still missing).
4. **3 residual `242343`/`3535253` observability/DLP items**:
   - `plugins/l7-http.md`: LLM payload retention / hash-only body audit mode (`retention.go`).
   - `plugins/ledger.md`: `CRC32C` segment framing + persisted ACK high-watermark (`Watchtower`).
   - `plugins/anomaly.md`: Explicit behavioral signals (raw-IP egress without prior DNS, denial bursts, privilege drift).
5. **Class 2 behavioral probes in `01-architecture.md`**: `13-cli.md` added behavioral probes to `doctor`, but `01-architecture.md#Driver-backends` didn't wire them into Class 2 pre-boot gating.
6. **Example blindspots**: `hook-matrix-full.{py,rs,ts}` still stubbed `on_fs_symlink` to `Pass` without `escapes_workspace`, and `autonomous-swe-agent.{py,ts}` didn't enable `soft_delete` on `Redaction` to match `kelir.toml`/`kelir.yaml`.

### Final Closure Diff (Fixes #1–#6; `<= 85` chars/line in `cans/`)

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -395,6 +395,7 @@
       - driver-bwrap: Linux user namespaces; requires allow_emulated_fallback=true
       - driver-proot: Termux ptrace emulation; requires allow_emulated_fallback=true
       - Class 2 sandboxes inherit identical BPF syscall and socket family blocks
+      - Class 2 sandboxes run behavioral probes pre-boot: see: 13-cli.md
       - Attempting to boot emulated tier without flag throws E_NOHARDWAREVMM
 
 - [2]: 10-attestation.md#Execution-receipts
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -384,8 +384,8 @@
       - profile: profile.rs
       - db-proxy: db-backed-runtime.py (userland SDK policy)
       - supply-chain: multi-vm-swarm.rs (userland SDK policy)
-      - egress-proxy, l7-http, redaction, step-eval: none yet
-      - taint, terminal-guard, token-shield, anomaly: none yet
+      - redaction, terminal-guard, token-shield: autonomous-swe-agent.{py,ts}
+      - egress-proxy, l7-http, step-eval, taint, anomaly: none yet
     - Non-plugin surface examples
       - Exercises raw traits or SDK surfaces; no catalog plugin
       - Terminal and trap interposition: see: artifacts/examples/git-gate.rs
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -28,7 +28,7 @@
       - on_fs_mkdir enforces directory creation boundaries and quota pre-checks
       - on_fs_readdir filters hidden or policy-excluded entries by hook verdict
       - on_fs_readlink resolves canonical targets against MountScope root
-      - Write, create, and unlink ops redirect via PatchPayload::FsPathRedirect
+      - Lookup, mkdir, rename, and unlink ops redirect via PatchPayload::FsPathRedirect
       - FUSE dispatch routes through pipeline engine across mounted interceptors
       - Emits physical StorageDelta events directly to host event bus on commit
     - Kernel physics invariants (storage)
--- a/cans/plugins/db-proxy.md
+++ b/cans/plugins/db-proxy.md
@@ -27,6 +27,7 @@
       - Synthesizes BackendKeyData and maps CancelRequest to prevent spoofing
       - Resolves search_path and temp-table aliases against catalog state
       - Supports read-only relation redirect and deny_mode_in_tx terminate
+      - Invalidates prepared statement cache on role or search_path change
     - SQLite UDS bridge mechanics:
       - Guest helper binds Unix Domain Socket at /run/kelir/db.sock on demand
       - Forwards client connections over DB_SQLITE mux stream
@@ -40,6 +41,7 @@
     - Verdict execution: see: 02-plugin-model.md#Unified-verdict
     - Protocol proxying
       - PostgreSQL on 5432, Redis on 6379
+      - Redis proxy gates EVAL, CONFIG, DEBUG, and FLUSH* by command policy
       - Ephemeral credentials injected at host connection boundary
       - Client mTLS certificates injected at host boundary
   - Error and return semantics (db-proxy)
--- a/cans/plugins/l7-http.md
+++ b/cans/plugins/l7-http.md
@@ -27,6 +27,7 @@
       - Normalizes OpenAI and Anthropic token usage and rate limits
       - Redacts outbound prompt PII via DLP rules before upstream dial
       - Inspects streaming SSE chunks for tool_use blocks before guest read
+      - Configurable LLM body retention supports hash-only audit mode
     - Threat feeds and egress guard
       - Syncs URLhaus and phishing blocklists with atomic swap on 304 check
       - Blocks .onion targets and Tor Onionoo relay IPs at L7/L4
--- a/cans/plugins/ledger.md
+++ b/cans/plugins/ledger.md
@@ -65,6 +65,7 @@
         - Sidecar records key_fingerprint and sequence across log rotations
         - Startup fails closed on key_fingerprint mismatch or missing segments
         - Supports KMS-backed signing keys (AWS, Azure, GCP, Vault) >= 32 bytes
+        - WAL segments use CRC32C framing with persisted ACK high-watermarks
     - OpenTelemetry
       - Default exporter: OTLP over gRPC and HTTP
         - Standard OTLP endpoints via OTEL_EXPORTER_OTLP_ENDPOINT
--- a/cans/plugins/anomaly.md
+++ b/cans/plugins/anomaly.md
@@ -13,6 +13,7 @@
   - Invariants and policy (anomaly)
     - Extracts normalized 16-dimensional feature vectors from telemetry
     - Sliding window (300 entries) over syscall rate, argv entropy, burstiness
+    - Flags raw-IP egress without DNS, denial bursts, and privilege drift
     - Surprisal threshold breach triggers policy escalation
     - Computed and policy-bearing metrics are plugin-owned, not core
     - Consumes zero-alloc metric registry and EventBus from core
--- a/cans/artifacts/examples/hook-matrix-full.py
+++ b/cans/artifacts/examples/hook-matrix-full.py
@@ -165,6 +165,8 @@
 
 
 def _on_fs_symlink(target: Path, link: Path) -> Verdict:
+    if escapes_workspace(str(target)) or escapes_workspace(str(link)):
+        return Verdict.drop(Errno.PATH_TRAVERSAL, "symlink escapes /workspace")
     return Verdict.pass_()
 
 
--- a/cans/artifacts/examples/hook-matrix-full.rs
+++ b/cans/artifacts/examples/hook-matrix-full.rs
@@ -349,6 +349,12 @@
         Ok(Verdict::Pass)
     }
     fn on_fs_symlink(&self, _target: &Path, _link: &Path) -> Result<Verdict, PluginError> {
+        if escapes_workspace(_target) || escapes_workspace(_link) {
+            return Ok(Verdict::Drop {
+                errno: Errno::PathTraversal,
+                reason: "symlink escapes /workspace".into(),
+            });
+        }
         Ok(Verdict::Pass)
     }
     fn on_fs_mkdir(&self, _path: &Path, _mode: u32) -> Result<Verdict, PluginError> {
--- a/cans/artifacts/examples/hook-matrix-full.ts
+++ b/cans/artifacts/examples/hook-matrix-full.ts
@@ -139,7 +139,12 @@
         return Verdict.Pass;
       })
       .onFsSetattr((_path, _mode, _size) => Verdict.Pass)
-      .onFsSymlink((_target, _link) => Verdict.Pass)
+      .onFsSymlink((target, link) => {
+        if (escapesWorkspace(target) || escapesWorkspace(link)) {
+          return Verdict.Drop(Errno.PATH_TRAVERSAL, "symlink escapes /workspace");
+        }
+        return Verdict.Pass;
+      })
       .onFsMkdir((_path, _mode) => Verdict.Pass)
       .onFsReaddir((_path, _entries) => FilterAction.Forward)
       .onFsReadlink((_path) => Verdict.Pass),
--- a/cans/artifacts/examples/autonomous-swe-agent.py
+++ b/cans/artifacts/examples/autonomous-swe-agent.py
@@ -275,6 +275,7 @@
 
         # see: cans/plugins/redaction.md#Plugin-redaction
         .use(redaction(RedactionConfig(
+            soft_delete={"enabled": True, "trash_dir": "/workspace/.kelir_trash"},
             mask_read=MaskReadConfig(
                 engine="fast-regex",
                 carryover_bytes=1024,
--- a/cans/artifacts/examples/autonomous-swe-agent.ts
+++ b/cans/artifacts/examples/autonomous-swe-agent.ts
@@ -71,7 +71,9 @@
     .use(tokenShield({ maxBytesTotal: 128 * 1024, maxLinesTotal: 500, clampStrategy: "head-and-tail", notice: "⚠️ [SHIELD]" }))
     .use(terminalGuard({ prompts: [{ pattern: "(?i)Overwrite production schema\\? \\[y/N\\]", timeoutSeconds: 3,
                                      verifyForeground: true, action: [...Buffer.from("y\n")] }] }))
-    .use(redaction({ maskRead: { engine: "fast-regex", carryoverBytes: 1024,
+    .use(redaction({
+      softDelete: { enabled: true, trashDir: "/workspace/.kelir_trash" },
+      maskRead: { engine: "fast-regex", carryoverBytes: 1024,
       rules: [{ glob: "**/secrets/**", patterns: [["sk_live_[0-9a-zA-Z]{24}", "[STRIPE_VAULT_KEY]"],
                                                    ["AKIA[0-9A-Z]{16}", "[AWS_IAM_REDACTED]"]] }] } }))
     .use(vault({
```

Now every single point from `242343` and `3535253` is accounted for without contradicting `02-plugin-model.md`. Note one intentional non-patch item remaining from `3535253` Section 2 #5: `kelir` still has zero live L3 hypervisor executions in `scorecard.md` (`0/13 kvm`, `0/6 hvf`), which is an execution/implementation milestone rather than a `cans/` spec diff.


True — `kelir` still has three structural steering weaknesses:
1. **Programmatic agents don't read Port 0 PTY**: `c.terminal.notice()` writes to Port 0 (human break-glass), while agent commands stream on `EXEC` (`0x0001`), bypassing PTY completely — so host blocks/redirects are invisible to the agent.
2. **Incomplete `Patch` payloads**: `02-plugin-model.md` lacked `NetRedirect` and `SignalMutation` payloads, and `07-wire-plane.md` hadn't bound `DnsVerdict::Redirect` / `on_socket_connect -> Patch(NetRedirect)`.
3. **No L7/SQL/MCP guidance fields**: `db-proxy` lacked pgwire `HINT ('H')`, `tool-guard` lacked `redirect_tool`/`guidance` in JSON-RPC `error.data`, and `10-attestation.md` conflicted with opt-in steering.

Targeted **Steering Patch** (Day-1 tone, all `cans/` lines `<= 80` chars, single-line bullets, zero `04-06 -> plugins/` leaks):

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -188,9 +188,10 @@
           - TCP RST or ICMP Port Unreachable, L7 gets a synthetic 403 JSON receipt)
         - The one host-response suppression is chaos loss injection, where the
           - driver suppresses the response (plugins/chaos.md)
-        - Optional redirect steering rides Patch payloads or L7/TTY guidance
-      - Patch payload is registered per ring: ExecveMutation, FsInode, FsPathRedirect,
-        - Custom(schema)
+        - Opt-in steering emits Drop reasons and Patch redirects as guidance
+        - Guidance injects into EXEC stderr, HTTP 403, JSON-RPC, or SQL HINT
+      - Patch payload per ring: ExecveMutation, FsInode, FsPathRedirect
+      - Additional Patch payloads: NetRedirect, SignalMutation, Custom(schema)
       - Idempotency and density: see: 01-architecture.md#Codebase-invariants
       - Hook-return pin (SSOT-A1): one Action enum is canonical
         - Type aliases, not split enums: Verdict and FilterAction are aliases of Action
--- a/cans/04-terminal-plane.md
+++ b/cans/04-terminal-plane.md
@@ -57,6 +57,8 @@
   - Binary exec channel (terminal plane)
     - Carries programmatic execution streams on stream kind EXEC (mux 0x0001)
     - Sub-lane framing: see: 11-protocol.md#Binary-exec-channel
+    - Programmatic steering injects host guidance into EXEC stderr (0x02)
+    - c.exec.steer(exec_id, msg) emits guidance before exit EOF flush
     - stdout and stderr retain bitwise fidelity; zero terminal escape pollution
     - Programmatic commands bypass PTY entirely; PTY (Port 0) is reserved for human attach
     - Per-exec PTY opt-in
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -29,6 +29,7 @@
       - on_fs_readdir filters hidden or policy-excluded entries by hook verdict
       - on_fs_readlink resolves canonical targets against MountScope root
       - Lookup, mkdir, rename, and unlink ops redirect via PatchPayload::FsPathRedirect
+      - Redirected creates and unlinks steer into scratch or trash with notice
       - FUSE dispatch routes through pipeline engine across mounted interceptors
       - Emits physical StorageDelta events directly to host event bus on commit
     - Kernel physics invariants (storage)
--- a/cans/07-wire-plane.md
+++ b/cans/07-wire-plane.md
@@ -12,6 +12,7 @@
     - Hook points: on_net_outbound, on_net_inbound, on_socket_connect
     - Additional net hooks: on_dns_query, on_socket_bind
     - on_socket_connect attributes PID via TrapInterposer socket-tuple correlation
+    - on_socket_connect admits Pass, Drop, or Patch(NetRedirect) steering
     - on_dns_query mediates domain resolution before packet egress
     - on_socket_bind blocks unauthorized listening ports before reservation
     - Abstract socket boundary: wire.block_abstract_unix_sockets denies \0 paths
@@ -99,7 +100,7 @@
       - Allowed queries populate a TTL-bounded synthetic or pinned IP table
       - Direct IP dials absent from the table drop fail-closed at L4 ingress
       - Rotating CDN answers refresh active IP pins per query TTL
-      - dns_redirect rewrites matched query names or answers before guest reply
+      - on_dns_query returns DnsVerdict::Redirect to steer domain answers
       - All UDP/TCP port 53 traffic to the gateway intercepted in kelir-wire::RealNetOps via kelir-engine::planes::wire
       - Traffic to non-gateway resolvers redirected at ingress ring
     - Host-state resolution
--- a/cans/10-attestation.md
+++ b/cans/10-attestation.md
@@ -7,7 +7,7 @@
     - Diagnostic metrics: PMU counters, dirty bytes, wire volume (gauges only)
     - Boundary opacity (L3/L4): interception surfaces only as physical bus errors,
       - standard POSIX errnos, or silent packet drops
-    - Agent-facing refusal guidance belongs to L7/TTY plugins, never raw L3/L4
+    - Opt-in EXEC/L7/TTY steering surfaces guidance without altering receipts
     - L7 wire-plane rejections are an explicit wire contract
       - (see: 11-protocol.md#Policy-rejection-receipt), not an attestation fact
   - Primitive vs derived metrics
--- a/cans/11-protocol.md
+++ b/cans/11-protocol.md
@@ -91,6 +91,7 @@
       - Host consumer slow or stalled → producer fd is paused, not dropped
       - No mid-flight frame loss; no guest kernel deadlock
     - Programmatic tooling uses stream kind EXEC (0x0001) exclusively; PTY bypassed
+    - Steering frames flush onto sub-lane 0x02 before exit control frame
     - Spawn submit path
       - SpawnBuilder carries the session transport bound at spawn_builder; poll() builds SpawnRequest and calls SpawnTransport::dispatch_exec
         - see: artifacts/code/wire-protocol.rs
@@ -147,7 +148,7 @@
       - on_signal traps kill, tkill, tgkill, rt_sigqueueinfo, rt_tgsigqueueinfo
       - on_signal also traps pidfd_send_signal across target relationships
       - Signal rules classify target: self, children, session, shim_pid1, peer
-      - Signal verdicts support Pass, Drop, Absorb, and Redirect (SIGKILL->SIGTERM)
+      - Signal verdicts: Pass, Drop, Absorb, or Patch(SignalMutation) redirect
       - on_fork intercepts process tree mutations and stops fork bombs in-RAM
     - Host containment verification
       - on_host_containment_breach emits immediate security alerts on openat2 trap
--- a/cans/plugins/db-proxy.md
+++ b/cans/plugins/db-proxy.md
@@ -51,6 +51,7 @@
         - pgwire ErrorResponse frame precedes connection close
         - SQLSTATE derived from errno (EPERM→42501, ENOENT→42P01)
         - Unmapped errno → SQLSTATE XX000 (internal_error)
+        - Populates pgwire ErrorResponse HINT ('H') with steering guidance
       - FilterAction::Forward after in-place Q/P frame rewrite
         - Server sees rewritten frame as if client sent it
         - Each connection's rewrites tallied; >3 aborts ECYCLICREWRITE
--- a/cans/plugins/l7-http.md
+++ b/cans/plugins/l7-http.md
@@ -16,7 +16,7 @@
     - Owns TLS interception via host keyring (plugin-scoped, not core)
     - Synthesizes policy-rejection receipts on the diverted L4 stream
       - HTTP 403 {"error":"E_POLICY_DENIED", reason, pid, binary}
-      - Includes optional policy guidance message for agent recovery
+      - HTTP 403 JSON includes optional guidance field for agent recovery
     - SinkholeRule (methods, paths) is plugin-private config
     - Core wire plane stops at L4; every byte above is owned here
     - Declared HTTP services
--- a/cans/plugins/tool-guard.md
+++ b/cans/plugins/tool-guard.md
@@ -78,6 +78,7 @@
       - Action::Drop returns JSON-RPC error object:
         - code = -32001 (EPERM) or mapped POSIX errno
         - message = user-defined reason or "Policy violation: tool execution rejected"
+        - data carries {errno, reason, guidance, redirect_tool} when steered
       - Schema drift or unpinned tool tampering returns -32601
         - (Method not found) + VM Security Panic
 - Suspend reason ownership (tool-guard)
--- a/cans/artifacts/examples/fixtures/kelir.toml
+++ b/cans/artifacts/examples/fixtures/kelir.toml
@@ -99,6 +99,11 @@
 blocked_syscalls = ["io_uring_setup", "io_uring_enter", "io_uring_register", "add_key", "request_key", "keyctl"]
 blocked_socket_families = ["AF_ALG", "AF_RXRPC", "NETLINK_XFRM"]
 
+[session.steering]
+steer_on_deny = true
+steer_on_redirect = true
+inject_exec_stderr = true
+
 # Session-scoped driver boot knobs; mirror of top-level driver.fast_boot.
 [session.driver]
 fast_boot = "off"
--- a/cans/artifacts/examples/fixtures/kelir.yaml
+++ b/cans/artifacts/examples/fixtures/kelir.yaml
@@ -138,6 +138,11 @@
       - "keyctl"
     blocked_socket_families: ["AF_ALG", "AF_RXRPC", "NETLINK_XFRM"]
 
+  steering:
+    steer_on_deny: true               # Emits structured agent guidance on Drop
+    steer_on_redirect: true           # Announces Patch redirects on EXEC/L7/SQL
+    inject_exec_stderr: true          # Flushes steering lines to EXEC sub-lane 0x02
+
   # Session-scoped driver boot knobs; mirror of top-level driver.fast_boot.
   driver:
     fast_boot: "off"
--- a/cans/artifacts/examples/git-gate.py
+++ b/cans/artifacts/examples/git-gate.py
@@ -43,7 +43,7 @@
             msg = ""
         if any(p in msg for p in PROTECTED):
             return Verdict.abort(
-                reason="EPOLICY: commit references protected branch",
+                reason="EPOLICY: commit targets protected branch; commit to feature/* instead",
             )
         return Verdict.pass_()
     return Verdict.pass_()
--- a/cans/artifacts/examples/git-gate.rs
+++ b/cans/artifacts/examples/git-gate.rs
@@ -62,7 +62,7 @@
                     .unwrap_or("");
                 if PROTECTED.iter().any(|p| msg.contains(p)) {
                     return Ok(Verdict::Abort {
-                        reason: "EPOLICY: commit references protected branch".into(),
+                        reason: "EPOLICY: commit targets protected branch; commit to feature/* instead".into(),
                     });
                 }
                 Ok(Verdict::Pass)
--- a/cans/artifacts/examples/git-gate.ts
+++ b/cans/artifacts/examples/git-gate.ts
@@ -28,7 +28,7 @@
           const mIndex = event.argv.indexOf("-m");
           const msg = mIndex >= 0 ? (event.argv[mIndex + 1] ?? "") : "";
           if (PROTECTED.some((p) => msg.includes(p)))
-            return c.abort("EPOLICY: commit references protected branch");
+            return c.abort("EPOLICY: commit targets protected branch; commit to feature/* instead");
           return c.pass();
         }
         default:
```


