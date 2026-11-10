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

# --examples-canonical: cans/artifacts/examples/ is the single canonical home.
check-examples-canonical:
    ./scripts/check-examples-canonical.sh

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
    pnpm --filter @kelir/core run typecheck
    uv run --no-project mypy packages/py

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

# Expected-fail red suite (work-order Phase 0.2): every #[ignore]d red
# test must fail with the signature recorded in the case-to-test map.
# Green iff every language reports exactly its map red count and 0 passes:
# Rust via nextest ignored-only, TypeScript via node --test over the
# packages/ts/core/tests/red suite, Python via pytest over
# packages/py/tests/red (both excluded from the default SDK runs).
test-red:
    #!/usr/bin/env bash
    set -euo pipefail
    MAP=$(cargo run -q -p kelir-testkit --bin check-manifest)
    RED=$(printf '%s\n' "$MAP" | grep -oE '[0-9]+ red' | grep -oE '[0-9]+' | head -1)
    LANG_LINE=$(printf '%s\n' "$MAP" | grep -E '^Red by language: rust=[0-9]+ typescript=[0-9]+ python=[0-9]+$')
    RED_RUST=$(printf '%s\n' "$LANG_LINE" | sed -E 's/^Red by language: rust=([0-9]+).*/\1/')
    RED_TS=$(printf '%s\n' "$LANG_LINE" | sed -E 's/^.*typescript=([0-9]+).*/\1/')
    RED_PY=$(printf '%s\n' "$LANG_LINE" | sed -E 's/^.*python=([0-9]+)$/\1/')
    RED=${RED:-0}
    if [ "$RED" -ne $((RED_RUST + RED_TS + RED_PY)) ]; then
        echo "FAIL: map red count $RED disagrees with per-language rust=$RED_RUST typescript=$RED_TS python=$RED_PY." >&2
        exit 1
    fi
    set +e
    OUT=$(cargo nextest run --run-ignored ignored-only --no-fail-fast 2>&1)
    set -e
    printf '%s\n' "$OUT" | tail -15
    RUST_FAILS=$(printf '%s\n' "$OUT" | grep -oE '[0-9]+ failed' | grep -oE '[0-9]+' | tail -1)
    RUST_PASSES=$(printf '%s\n' "$OUT" | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' | tail -1)
    RUST_FAILS=${RUST_FAILS:-0}
    RUST_PASSES=${RUST_PASSES:-0}
    TS_FAILS=0
    TS_PASSES=0
    if ls packages/ts/core/tests/red/*.test.ts >/dev/null 2>&1; then
        pnpm -r build > /dev/null
        set +e
        TS_OUT=$(node --test --test-reporter=tap packages/ts/core/tests/red/*.test.ts 2>&1)
        set -e
        printf '%s\n' "$TS_OUT" | grep -E '^(not ok|# (tests|pass|fail))' | tail -15
        TS_FAILS=$(printf '%s\n' "$TS_OUT" | grep -oE '^# fail [0-9]+' | grep -oE '[0-9]+' | tail -1)
        TS_PASSES=$(printf '%s\n' "$TS_OUT" | grep -oE '^# pass [0-9]+' | grep -oE '[0-9]+' | tail -1)
        TS_FAILS=${TS_FAILS:-0}
        TS_PASSES=${TS_PASSES:-0}
    fi
    PY_FAILS=0
    PY_PASSES=0
    if ls packages/py/tests/red/test_*.py >/dev/null 2>&1; then
        set +e
        PY_OUT=$(uv run --no-project pytest packages/py/tests/red 2>&1)
        set -e
        printf '%s\n' "$PY_OUT" | tail -15
        PY_FAILS=$(printf '%s\n' "$PY_OUT" | grep -oE '[0-9]+ failed' | grep -oE '[0-9]+' | tail -1)
        PY_PASSES=$(printf '%s\n' "$PY_OUT" | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' | tail -1)
        PY_FAILS=${PY_FAILS:-0}
        PY_PASSES=${PY_PASSES:-0}
    fi
    if [ "$RUST_PASSES" -ne 0 ] || [ "$TS_PASSES" -ne 0 ] || [ "$PY_PASSES" -ne 0 ] \
        || [ "$RUST_FAILS" -ne "$RED_RUST" ] || [ "$TS_FAILS" -ne "$RED_TS" ] || [ "$PY_FAILS" -ne "$RED_PY" ]; then
        echo "FAIL: expected rust=$RED_RUST typescript=$RED_TS python=$RED_PY failures and 0 passes; observed rust=$RUST_FAILS/$RUST_PASSES typescript=$TS_FAILS/$TS_PASSES python=$PY_FAILS/$PY_PASSES (failures/passes)." >&2
        exit 1
    fi
    echo "OK: $RED red tests failing exactly as specified (rust=$RUST_FAILS typescript=$TS_FAILS python=$PY_FAILS, 0 passes)."

# SDK suites: TS node --test over compiled dist, Python pytest.
# ForeignPluginProxy, memory pinning, unhandled freeze recovery
test-sdks: build-ts
    pnpm -r test
    uv run --no-project pytest packages/py

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
# Regenerate cans/artifacts/scorecard.md in place from manifest truth +
# perf/budgets.toml; check-scorecard gates the committed artifact byte-for-byte.
scorecard:
    cargo run -p kelir-testkit --bin scorecard -- generate --in-place

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

# ── Codegen coherence pipeline (work-order-2: CI/CD Coherence Architecture) ──
# SSOT flow: cans/*.md + artifacts -> kelir-interface (macros/schemas)
#   -> code generation (TS/PY types, plugin projections, fusion tables,
#      wire consts, UAPI tables, anchors, scorecard) -> committed output
#   -> CI coherence gate: regenerate + `git diff --exit-code` (zero drift).
# Generated files are checked in (Hermetic Commit Invariant); every
# generator below is deterministic and byte-stable across runs.
codegen:
    #!/usr/bin/env bash
    set -euo pipefail
    # Item 1/2/3/12/15: one native generator emits TS/PY SDK types, plugin SDK
    # projections, fusion ring/hook tables, protocol consts, shim wire.h,
    # OTLP records, Merkle leaves
    cargo run --quiet -p kelir-interface --bin codegen
    # Item 13: spec anchor index (committed at crates/kelir-testkit/anchors.json)
    cargo run --quiet -p kelir-testkit --bin anchors
    # Item 5/11: UAPI tables (linux/fuse.h, linux/elf.h) via C oracle
    sh crates/kelir-core/codegen/gen-fuse-uapi.sh
    sh crates/kelir-shim/codegen/gen-elf-uapi.sh
    # Item 9: scorecard render from perf/budgets.toml. Stays a target/ preview
    # here, NOT --in-place: the platform-subset section is host-dependent, and
    # check-coherence must stay zero-drift on any host. Sync the committed
    # artifact explicitly via `just scorecard`; check-scorecard gates it.
    cargo run --quiet -p kelir-testkit --bin scorecard -- generate

# Zero-drift verification gate: regenerate everything, tree must stay clean.
# check-codegen is the work-order-2 section-2 spelling of the same gate.
check-coherence: codegen
    #!/usr/bin/env bash
    set -euo pipefail
    if ! git diff --exit-code; then
        echo "FAIL: Generated code out of sync. Run \`just codegen\`" >&2
        exit 1
    fi
    echo "OK: zero drift between SSOT schemas and committed artifacts."

check-codegen: check-coherence

# Compile-only gate across all three languages (work-order-2 section 3).
check-types:
    cargo check --workspace --all-targets
    pnpm -r exec tsc --noEmit
    uv run mypy packages/py

# Test-plan gates in one recipe (work-order-2 section 3).
check-gates: check-manifest check-tests check-scorecard