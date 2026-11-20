# kelir development orchestrator
set shell := ["bash", "-uc"]

default:
    @just --list

# ── Toolchain & setup ──
bootstrap:
    cargo install cargo-nextest cargo-sweep sccache --locked
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

# Builds the PyO3 extension module into the Python package so KelirBuilder.boot() can load it.
build-py:
    cargo build -p kelir-pyo3
    cp target/debug/libkelir_pyo3.so packages/py/kelir/_kelir.abi3.so

# Disk-conscious default: one crate under the dev-min profile (incremental
# off, debug symbols off). Never --workspace here; that is the 30G path.
check-one crate:
    cargo check --profile dev-min -p {{crate}}

# Prune build artifacts older than 7 days. Full `cargo clean` is the last
# resort, never routine: it wipes the whole cache for a full rebuild.
clean-old:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v cargo-sweep >/dev/null 2>&1; then
        cargo sweep --time 7
    else
        echo "cargo-sweep not installed; run: cargo install cargo-sweep --locked" >&2
        exit 1
    fi

# ── Audit & validation ──
# check-spec is the spec gate only (refs + structure + redundancy).
# Coverage gates live elsewhere; see 15-operations.md#Coverage-gates.
# Flag string lives here: CI calls just check-spec, never raw cans flags.
# see: cans/15-operations.md#Spec-gate
check-spec:
    @cans check --refs-only --json | jq -e '.ok and .exitCode == 0'
    @cans check --json          | jq -e '.ok and .exitCode == 0'

# Pull-request tier: cans 0.21.0 has no --touched-files flag, so the PR tier
# runs the same refs-only check as check-spec over the whole corpus.
check-spec-touched:
    @cans check --refs-only --json | jq -e '.ok and .exitCode == 0'

# --plugin-coverage: every community plugin names a demonstrating example.
check-plugin-coverage:
    ./scripts/check-plugin-coverage.sh

# --action-coverage: every Action variant appears in at least one vector.
check-action-coverage:
    cargo run -p kelir-plugin-cli -- parity run vectors.json --action-coverage

# --l7-stream-coverage: at least one l7-stream-* vector exercises in-ring
# L7 HTTP/TLS parsing. Live execution behind the vector is L3; the flag
# and the assertion call site are the L1/L2 wiring. Evidence owner: WS-B1.
check-l7-stream-coverage:
    cargo run -p kelir-plugin-cli -- parity run vectors.json --l7-stream-coverage

# --outbox-coverage: at least one outbox-* vector exercises the egress
# outbox (release on commit, purge on rollback). Evidence owner: WS-B6/WS-C3.
check-outbox-coverage:
    cargo run -p kelir-plugin-cli -- parity run vectors.json --outbox-coverage

# --step-dag-coverage: at least one step-dag-* vector exercises the
# branching step DAG (attribution + SoD). Evidence owner: WS-B7.
check-step-dag-coverage:
    cargo run -p kelir-plugin-cli -- parity run vectors.json --step-dag-coverage

# --swarm-mesh-coverage: at least one swarm-mesh-* vector exercises a
# broadcast/await winner cycle. Evidence owner: WS-B9/WS-C4.
check-swarm-mesh-coverage:
    cargo run -p kelir-plugin-cli -- parity run vectors.json --swarm-mesh-coverage

# All four WS-F2 coverage gates in one recipe (each fails closed on missing
# evidence; see cans/15-operations.md#Coverage-gates).
check-coverage-gates: check-l7-stream-coverage check-outbox-coverage check-step-dag-coverage check-swarm-mesh-coverage

# --spec-drift: every depth<=1 normative leaf in the patch-series spec files
# maps to a code symbol or test; drift recorded in scripts/spec-drift-gaps.yaml
# passes, unrecorded drift fails. See scripts/check-spec-drift.sh.
# see: cans/15-operations.md#Spec-drift
check-spec-drift:
    ./scripts/check-spec-drift.sh

# --examples-run: run the six examples as tests (Rust via the
# throwaway crate, Python directly, TypeScript via node on compiled output).
# Hardware tier required for git-gate and hook-matrix-full; emulated Class 2
# allowed for the rest. See scripts/check-examples-run.sh.
# see: cans/15-operations.md#Examples-run
check-examples-run:
    ./scripts/check-examples-run.sh

# --examples-canonical: cans/artifacts/examples/ is the single canonical home.
check-examples-canonical:
    ./scripts/check-examples-canonical.sh

# --examples: the canonical examples compile/typecheck in all three languages.
# see: cans/15-operations.md#Examples-compile
check-examples:
    ./scripts/check-examples.sh

# --examples-thin: the canonical examples import only the public SDK surface plus the standard library.
check-examples-thin:
    ./scripts/check-examples-thin.sh

# --builder-parity: the unified KelirBuilder chain is identical in Rust/TS/Python.
check-builder-parity:
    ./scripts/check-builder-parity.sh

# --builder-verbs-sync: the spec-side verb mirror agrees with the machine SSOT.
# see: cans/artifacts/config/builder-verbs.yaml
check-builder-verbs-sync:
    python3 scripts/check-builder-verbs-sync.py

# --builder-chain: the unbroken single-expression chain holds in every
# example under cans/artifacts/examples/: no builder rebinding, no
# statement-level verb calls, no flat verb outside a scope, no BasePlugin,
# no kelir.plugins.plugin_ alias. see: cans/15-operations.md#Builder-chain
check-builder-chain:
    python3 scripts/check-builder-chain-ast.py cans/artifacts/examples/

# --negative-suites: compile-fail invariants hold in all three SDKs
# (scope bleeding, post-seal mutation, host verbs on plugin builders).
# The Python suite lands with the py track; until then this recipe fails
# on its missing file. see: cans/15-operations.md#Negative-suites
check-negative-suites:
    cargo test -p kelir --test compile_fail
    pnpm --filter @kelir/core run test:compile-fail
    pytest packages/py/tests/test_compile_fail.py

# Compile-only gate for drivers with no CI test runner (Android AVF,
# Termux proot); test execution needs hardware CI does not have.
check-drivers:
    cargo check -p driver-avf -p driver-proot

check-layers:
    cargo run -p kelir-plugin-cli -- check --layers

# ── Law 8: core blindness gate ──
# G1 facade manifest: zero plugin-* deps. G2 facade src: zero plugin tokens
# (plugin_model is the accepted seam: the generic model lives in
# kelir-interface per cans/02-plugin-model.md#Facade-blindness).
# G3 facade dep graph: zero plugin crates at any depth. G4 structural lint.
check-core-blindness:
    #!/usr/bin/env bash
    set -euo pipefail
    ! grep -qE '^[[:space:]]*plugin-' crates/kelir/Cargo.toml
    [ "$(git grep -iE 'plugin_' -- crates/kelir/src | grep -viE 'plugin_model|plugin_instance|\[plugin_engine\]' | wc -l)" -eq 0 ]
    [ "$(cargo tree -p kelir -e normal 2>/dev/null | grep -cE 'plugin-')" -eq 0 ]
    cargo run -p kelir-plugin-cli -- check --layers

check-budgets:
    cargo run -p kelir-plugin-cli -- check --budgets perf/budgets.toml

# --maturity: filename level ∈ crate allowed_levels, forbid/structural
# rules S1-S6 gated by [policy] forbid_mode (error), placement always hard.
# see: cans/15-operations.md#Maturity-check
check-maturity:
    cargo run -p kelir-maturity -- check --strict

# Same, plus the per-PR L2 delta budget (net-new .flow.rs files) against a
# base ref. CI PRs pass the PR base; local default is origin/main.
check-maturity-base base="origin/main":
    cargo run -p kelir-maturity -- check --strict --base {{base}}

# --packages-maturity: contract/wiring/meat split in packages/ts + packages/py.
check-packages-maturity:
    python3 scripts/check-packages-maturity.py

panic-hunt:
    cargo nextest run -p kelir-engine --test panic_hunt

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
    cargo deny check licenses
    echo "OK: All files <= 1000 LOC; zero inline tests in src/; licenses ok."

# see: cans/15-operations.md#Tiered-compilation
check-tiered:
    cargo check --profile dev-min --workspace --all-targets

check-graph:
    cargo run -p kelir-plugin-cli -- check --graph

# Both kelir-plugin-cli structural gates in one recipe: layering + plugin DAG.
check-cli:
    cargo run -p kelir-plugin-cli -- check --layers && cargo run -p kelir-plugin-cli -- check --graph

check-plugin plugin:
    cargo run -p kelir-plugin-cli -- check -p plugin-{{plugin}}

check-plugin-all:
    #!/usr/bin/env bash
    set -euo pipefail
    for p in anomaly chaos container db-proxy egress-proxy l7-http ledger mcp mesh profile redaction replay step-eval supply-chain taint terminal-guard token-shield tool-guard transaction vault wire-ingress; do
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
    uv run --with ruff --no-project ruff check --select F packages/py

typecheck: build-ts
    pnpm -r exec tsc --noEmit
    pnpm --filter @kelir/core run typecheck
    uv run --no-project mypy packages/py

# ── Testing ──
# Core kernel + plugin-testkit
test-core:
    cargo nextest run -p kelir-engine -p kelir-plugin-testkit

# Every optional domain plugin independently. Failure in one does not mask others.
test-plugins:
    cargo nextest run -p plugin-anomaly -p plugin-chaos -p plugin-container -p plugin-db-proxy -p plugin-egress-proxy -p plugin-l7-http -p plugin-ledger -p plugin-mcp -p plugin-mesh -p plugin-profile -p plugin-redaction -p plugin-replay -p plugin-step-eval -p plugin-supply-chain -p plugin-taint -p plugin-terminal-guard -p plugin-token-shield -p plugin-tool-guard -p plugin-transaction -p plugin-vault -p plugin-wire-ingress

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
    OUT=$(cargo nextest run --workspace --run-ignored ignored-only --no-fail-fast 2>&1)
    set -e
    printf '%s\n' "$OUT" | tail -15
    RUST_FAILS=$(printf '%s\n' "$OUT" | grep -oE '[0-9]+ failed' | grep -oE '[0-9]+' | tail -1 || true)
    RUST_PASSES=$(printf '%s\n' "$OUT" | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' | tail -1 || true)
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
        TS_FAILS=$(printf '%s\n' "$TS_OUT" | grep -oE '^# fail [0-9]+' | grep -oE '[0-9]+' | tail -1 || true)
        TS_PASSES=$(printf '%s\n' "$TS_OUT" | grep -oE '^# pass [0-9]+' | grep -oE '[0-9]+' | tail -1 || true)
        TS_FAILS=${TS_FAILS:-0}
        TS_PASSES=${TS_PASSES:-0}
    fi
    PY_FAILS=0
    PY_PASSES=0
    if ls packages/py/tests/red/test_*.py >/dev/null 2>&1; then
        set +e
        PY_OUT=$(PYTHONPATH=packages/py uv run --no-project pytest packages/py/tests/red 2>&1)
        set -e
        printf '%s\n' "$PY_OUT" | tail -15
        PY_FAILS=$(printf '%s\n' "$PY_OUT" | grep -oE '[0-9]+ failed' | grep -oE '[0-9]+' | tail -1 || true)
        PY_PASSES=$(printf '%s\n' "$PY_OUT" | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' | tail -1 || true)
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
    PYTHONPATH=packages/py uv run --no-project pytest packages/py

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
pre-flight: check-manifest check-tests check-spec check-fast-boot check-dax test-unit

# --fast-boot-coverage: cache hit, miss, drift, backend-change paths.
# --no-tests pass: the filter selects the 4 hd_fast_boot stubs in
# kelir-integration/tests/full_pipeline_test.rs (#[ignore], run via test-red);
# it must not fail the run when the stubs are absent. Scoped to
# kelir-integration; -p kelir-engine --lib is wrong here — the kelir-engine
# integration suite does not compile at base (E0063 in receipt_attest_test.rs,
# pre-existing).
check-fast-boot:
    cargo nextest run -p kelir-integration -E 'test(hd_fast_boot)' --no-tests pass

# --dax-coverage: DAX available, unavailable, teardown paths.
check-dax:
    cargo nextest run -p kelir-integration -E 'test(sp_dax)' --no-tests pass

check-canon:
    ./scripts/check-canon.sh
    just test-all

# Validate parity between manifest.yaml and cases/**/*.yaml files
check-manifest:
    cargo run -p kelir-testkit --bin check-manifest

# Run static AST linter enforcing Anti-Slop Laws AS-01 to AS-08
check-tests:
    cargo run -p kelir-testkit --bin check-tests

    cargo run -p kelir-plugin-cli -- parity run
    cargo run -p kelir-plugin-cli -- fixtures --verify

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
    sh crates/kelir-engine/codegen/gen-fuse-uapi.sh
    sh crates/kelir-shim/codegen/gen-elf-uapi.sh

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
check-gates: check-manifest check-tests

# Full local gate: every check above, in dependency order. Excludes test-integration (hypervisor), test-drivers-windows/macos (platform jobs), build-shim cross-compile, check-spec-touched (PR-only).
check-all: check-coherence check-spec check-plugin-coverage check-examples-canonical check-examples check-examples-thin check-builder-parity check-builder-verbs-sync check-hygiene check-maturity check-packages-maturity lint typecheck check-tiered check-cli check-budgets check-plugin-all check-drivers check-gates check-fast-boot check-dax test-red parity panic-hunt check-canon
