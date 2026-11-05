**Execution Philosophy:** Zero CPU burn. Sub-minute local cycles via `just`. Maximize GitHub Actions free tier (public repo = unlimited minutes, but macOS runners eat concurrency; private repo = 2,000 min ceiling).

SSOT: the justfile is `cans/artifacts/config/justfile`; the workflow is `cans/artifacts/config/ci.yml`. This doc carries no second copy of either — the embedded sketches drifted (a `driver-libkrun` that no longer exists, no cans-check job at all) and were replaced by pointers.

---

## 1. Local Velocity: `justfile`

Requires: `just`, `cargo-nextest`, `mold` (Linux), `zig` (for the static shim), `pnpm`, `uv`.

Local gates, all in the justfile: `check-spec` (spec gate, the only cans invocation; CI calls it, never raw flags), `check-spec-touched` (pull-request tier), `check-plugin-coverage`, `check-layers`, `check-graph`, `check-tiered` (`cargo check --workspace --all-targets`), `check-budgets`, `panic-hunt`, `parity`, `test-plugins` (one cell per community plugin), `test-drivers` (rustvmm, openvmm, bwrap), `check-drivers` (compile-only avf, proot), `pre-flight` (the union gate before push).

---

## 2. GitHub Actions Matrix (`cans/artifacts/config/ci.yml`)

*Strategy:* Public repos get free minutes, but macOS runners are scarce and drain private allotments at a **10x multiplier**. Run 90% of checks on Linux x86; run macOS Apple Silicon jobs strictly for the `driver-hvf` compilation gate.

Jobs: cans-check (spec gate via `just check-spec`, plus plugin coverage), lint-and-types, build-guest-shim (musl shim, <100KB size gate), test-core, panic-hunt, parity, package-check, test-plugins (13-cell matrix, fail-fast false), plugin-graph, plugin-audit, layer-check, verify-tiered-compilation, test-shim, test-drivers (plus compile-only avf/proot), perf-budgets, test-integration (KVM-detected, continue-on-error on hosted), transport-windows (SPSC rings + named pipes only), check-macos-hvf (compile-only driver-hvf).
