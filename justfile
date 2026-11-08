# kelir development orchestrator
set shell := ["bash", "-uc"]

default:
    @just --list

# ── Toolchain & setup ──
bootstrap:
    cargo install cargo-nextest sccache --locked
    rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl
    pnpm install --frozen-lockfile
    uv venv && uv pip install -e "packages/py[dev]"

# ── Compilation ──
# Compiles host core using mold/lld linker and sccache
build-host:
    RUSTC_WRAPPER=sccache cargo build --workspace --exclude kelir-shim

# Compiles static 50KB in-guest musl init binary
build-shim target="aarch64-unknown-linux-musl":
    cargo zigbuild -p kelir-shim --release --target {{target}}
    @ls -lh target/{{target}}/release/kelir-shim

# Builds all TS packages: @kelir/core plus every @kelir/plugin-* binding
build-ts:
    pnpm -r build

# ── Audit & validation ──
# check-spec is the spec gate only (refs + structure + redundancy).
# Coverage gates live elsewhere; see 15-operations.md#Coverage-gates.
# Flag string lives here: CI calls just check-spec, never raw cans flags.
check-spec:
    @cans check --refs-only --json | jq -e '.ok and .exitCode == 0'
    @cans check --json          | jq -e '.ok and .exitCode == 0'

# Pull-request tier: same flags as check-spec, restricted to the touched
# file list CI writes to /tmp/touched.
check-spec-touched:
    @cans check --touched-files /tmp/touched --json | jq -e '.ok and .exitCode == 0'

# --plugin-coverage: every community plugin names a demonstrating example.
check-plugin-coverage:
    ./scripts/check-plugin-coverage.sh

# Codebase inbound refs (RFC #66, local checker): every embedded see:
# pointer in code resolves; the self-test proves each failure mode.
check-codebase-refs:
    python3 scripts/check-codebase-refs.py check
    ./scripts/check-codebase-refs-selftest.sh

# Compile-only gate for drivers with no CI test runner (Android AVF,
# Termux proot); test execution needs hardware CI does not have.
check-drivers:
    cargo check -p driver-avf -p driver-proot

check-layers:
    cargo run -p kelir-plugin-cli -- check --layers

check-budgets:
    cargo run -p kelir-plugin-cli -- check --budgets perf/budgets.toml

panic-hunt:
    cargo nextest run -p kelir-core --test panic_hunt

fetch-fixtures:
    cargo run -p kelir-plugin-cli -- fixtures --fetch

check-hygiene:
    #!/usr/bin/env bash
    set -euo pipefail
    oversized=$(find crates -name "*.rs" -exec wc -l {} + | awk '$1 > 1000 && $2 != "total" {print $2 " (" $1 " lines)"}')
    if [ -n "$oversized" ]; then
        echo "FAIL: Files > 1,000 LOC detected:" && echo "$oversized" && exit 1
    fi
    inline_tests=$(grep -rn --include="*.rs" '#\[cfg(test)\]' crates/*/src/ || true)
    if [ -n "$inline_tests" ]; then
        echo "FAIL: Inline #[cfg(test)] forbidden in src/. Move to tests/:"
        echo "$inline_tests" && exit 1
    fi
    unsafe_violations=$(grep -rn --include="*.rs" 'unsafe ' crates/ | grep -v 'crates/kelir-drivers/' | grep -v 'crates/kelir-transport/' | grep -v 'crates/kelir-shim/' || true)
    if [ -n "$unsafe_violations" ]; then
        echo "FAIL: Unsafe code detected outside driver/transport/shim enclaves:"
        echo "$unsafe_violations" && exit 1
    fi
    echo "OK: All files <= 1000 LOC; zero inline tests in src/."

check-tiered:
    cargo check --workspace --all-targets

check-graph:
    cargo run -p kelir-plugin-cli -- check --graph

check-plugin plugin:
    cargo run -p kelir-plugin-cli -- check -p plugin-{{plugin}}

check-plugin-all:
    #!/usr/bin/env bash
    set -euo pipefail
    for p in chaos container db-proxy ledger mcp mesh profile replay supply-chain tool-guard transaction vault wire-ingress; do
        just check-plugin "$p"
    done

# ── Lint & typecheck ──
lint:
    cargo fmt --all -- --check
    cargo clippy --workspace --exclude kelir-shim -- -D warnings \
        -D clippy::unwrap_used \
        -D clippy::expect_used \
        -D clippy::panic \
        -D clippy::unimplemented \
        -D clippy::todo \
        -D clippy::unreachable
    pnpm -r exec eslint .

typecheck: build-ts
    pnpm -r exec tsc --noEmit
    uv run mypy packages/py

# ── Testing ──
# Core kernel + plugin-testkit
test-core:
    cargo nextest run -p kelir-core -p kelir-plugin-testkit

# Every optional domain plugin independently. Failure in one does not mask others.
test-plugins:
    cargo nextest run -p plugin-chaos -p plugin-container -p plugin-db-proxy -p plugin-ledger -p plugin-mcp -p plugin-mesh -p plugin-profile -p plugin-replay -p plugin-supply-chain -p plugin-tool-guard -p plugin-transaction -p plugin-vault -p plugin-wire-ingress

# Single plugin (debugging)
test-plugin plugin:
    cargo nextest run -p plugin-{{plugin}}

# No shim tests at base; when added, anti-TOCTOU, cgroup.freeze, and zombie
# reaper tests run in isolated container/namespaces
test-shim:
    cargo nextest run -p kelir-shim

# Driver primitives (Linux); excludes test_hw_* paths that need real hardware
# Windows openvmm and macOS hvf run in their platform jobs, not here.
test-drivers:
    cargo nextest run -p driver-rustvmm -p driver-bwrap -E 'not test(test_hw_)'

# Windows driver suite placeholder (no driver-openvmm tests at base); run on Windows only (driver-openvmm is WHP).
test-drivers-windows:
    cargo nextest run -p driver-openvmm -E 'not test(test_hw_)'

# macOS driver suite placeholder (no driver-hvf tests at base); run on Apple Silicon only (driver-hvf is HVF).
test-drivers-macos:
    cargo nextest run -p driver-hvf -E 'not test(test_hw_)'

# No SDK test suites at base (no pnpm test script, no pytest files): ForeignPluginProxy, memory pinning, unhandled freeze recovery
test-sdks: build-ts
    pnpm -r test
    uv run pytest packages/py

# Cross-plugin integration; requires real hypervisor (KVM/HVF)
test-integration:
    cargo nextest run -p kelir-integration

# Everything except integration (which requires real VM)
test-all: test-core test-plugins test-shim test-drivers test-sdks

# Pure-memory unit tests: the test-all set, zero hypervisor calls.
# The test_hw_* exclusion is the spec-named forward guard
# (15-operations.md): no test fn bears that name at base, so the
# filter matches nothing today and needs no widening.
test-unit: test-all

# ── Parity ──
# Single implementation: kelir-plugin-cli runs vectors (Rust fixtures →
# TS + Python suites) and the surface manifest diff. CI calls the same
# subcommand.
parity: build-ts
    cargo run -p kelir-plugin-cli -- parity run

# ── Pre-flight (single gate before push, <60s) ──
# Fast static gates plus pure-memory unit tests only: zero
# hypervisor calls. check-canon stays its own recipe (it runs
# test-all and lives in CI); hardware lives in test-integration.
pre-flight: check-manifest check-tests check-spec test-unit

check-canon:
    ./scripts/check-canon.sh
    just test-all

# ── Test plan gates (work-order.md Phase 6: Scorecard & Linter Harness) ──
# Generate cans/artifacts/scorecard.md from manifest and live telemetry
scorecard:
    cargo run -p kelir-testkit --bin scorecard -- generate

# CI Gate: Verify scorecard is byte-for-byte identical to physical evidence.
# At base this verifies against manifest truth only; platform-subset
# filtering rides this same recipe if added (no case-to-test map yet).
check-scorecard:
    cargo run -p kelir-testkit --bin scorecard -- check

# Validate parity between manifest.yaml and cases/**/*.yaml files
check-manifest:
    cargo run -p kelir-testkit --bin check-manifest

# Run static AST linter enforcing Anti-Slop Laws AS-01 to AS-08
check-tests:
    cargo run -p kelir-testkit --bin check-tests