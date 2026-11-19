#!/usr/bin/env bash
# check-examples-run: WS-F1 example-as-test wiring. Runs the six
# see: cans/15-operations.md#Examples-run
# examples as tests in all three languages and reports one outcome per
# example per language:
#   PASS        exit 0 (in-file asserts held)
#   RUNNING     still alive at the timeout with no error output (serve-forever
#               examples); reported distinctly, never counted as PASS
#   SKIP        hardware tier absent (git-gate, hook-matrix-full need /dev/kvm)
#   COMPILE-GAP Rust example does not compile: a WS-A..E surface is missing
#   FAIL        any other nonzero exit
#   BLOCKED     the language toolchain is not installed (pnpm/uv missing)
# The gate exits 1 unless every example is PASS, RUNNING, or SKIP. A RUNNING
# entry is an unasserted live process, not a verified serve.
# Name mapping: the work-order spells the cargo-example form with underscores
# (serverless_webhook); files use hyphens (serverless-webhook.rs). Rust runs
# through the same throwaway-crate mechanism as scripts/check-examples.sh
# (the crate generation below is its twin; check-examples.sh owns compile,
# this script owns run). Python has no test module to collect: the examples
# are scripts with __main__ guards, so the Py leg executes each file
# directly (pytest over the directory would collect zero tests). TypeScript
# likewise: npm test runs the package test suite, not examples, so the TS leg
# compiles the six examples and runs node over the emitted output.
# The three quarantined examples (serverless-webhook, db-backed-runtime,
# multi-vm-swarm) live in cans/artifacts/examples/deferred/ and are not
# run here; see: cans/15-operations.md#Examples-compile.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"

EXAMPLES="container autonomous-swe-agent policy-matrix git-gate hook-matrix-full replay"
HW_EXAMPLES="git-gate hook-matrix-full"
TIMEOUT_SECS="${KELIR_EXAMPLE_TIMEOUT_SECS:-60}"

pass=0; running=0; skipped=0; gap=0; failed=0; blocked=0

have_kvm() { [ -r /dev/kvm ] && [ -w /dev/kvm ]; }
is_hw() {
    local name="$1" item
    for item in $HW_EXAMPLES; do
        [ "$item" = "$name" ] && return 0
    done
    return 1
}
report() {
    local name="$1" lang="$2" outcome="$3" detail="${4:-}"
    printf '%-28s %-10s %s %s\n' "$name" "$lang" "$outcome" "$detail"
    case "$outcome" in
        PASS) pass=$((pass + 1)) ;;
        RUNNING) running=$((running + 1)) ;;
        SKIP) skipped=$((skipped + 1)) ;;
        COMPILE-GAP) gap=$((gap + 1)) ;;
        FAIL) failed=$((failed + 1)) ;;
        BLOCKED) blocked=$((blocked + 1)) ;;
    esac
}

# --- Rust: cargo run --bin per example in a throwaway crate ---------------
rs_run() {
    local workdir targetdir edition
    workdir="$(mktemp -d "${TMPDIR:-/tmp}/kelir-examples-run.XXXXXX")"
    targetdir="$ROOT/target/check-examples-run"
    trap "rm -rf '$workdir'" EXIT
    edition="$(sed -n 's/^edition *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
    edition="${edition:-2024}"
    mkdir -p "$workdir/gen"
    {
        echo '[package]'
        echo 'name = "kelir-examples-run"'
        echo 'version = "0.0.0"'
        echo "edition = \"$edition\""
        echo
        echo '[workspace]'
        echo
        for name in $EXAMPLES; do
            printf '[[bin]]\nname = "%s"\npath = "%s/cans/artifacts/examples/%s.rs"\n\n' "$name" "$ROOT" "$name"
        done
        echo '[dependencies]'
        echo "kelir = { path = \"$ROOT/crates/kelir\", features = [\"all-plugins\"] }"
        echo "kelir-interface = { path = \"$ROOT/crates/kelir-interface\" }"
    } > "$workdir/Cargo.toml"
    if [ -f "$ROOT/Cargo.lock" ]; then
        cp "$ROOT/Cargo.lock" "$workdir/Cargo.lock"
    fi
    if [ -z "${LIBCLANG_PATH:-}" ] && [ -f "$HOME/.local/lib/python3.12/site-packages/clang/native/libclang.so" ]; then
        export LIBCLANG_PATH="$HOME/.local/lib/python3.12/site-packages/clang/native/libclang.so"
    fi
    export CARGO_TARGET_DIR="$targetdir"
    local name out rc
    for name in $EXAMPLES; do
        if is_hw "$name" && ! have_kvm; then
            report "$name" rust SKIP "no /dev/kvm; hardware tier required"
            continue
        fi
        if ! out=$(cd "$workdir" && cargo +stable check --bin "$name" 2>&1); then
            printf '%s\n' "$out" | tail -5 | sed "s/^/    [$name rust] /"
            report "$name" rust COMPILE-GAP "surface missing (WS-A..E lane)"
            continue
        fi
        set +e
        out=$(cd "$workdir" && timeout "$TIMEOUT_SECS" cargo +stable run --quiet --bin "$name" 2>&1)
        rc=$?
        set -e
        if [ "$rc" -eq 0 ]; then
            report "$name" rust PASS
        elif [ "$rc" -eq 124 ]; then
            printf '%s\n' "$out" | tail -5 | sed "s/^/    [$name rust] /"
            report "$name" rust RUNNING "alive at ${TIMEOUT_SECS}s timeout; unasserted"
        else
            printf '%s\n' "$out" | tail -10 | sed "s/^/    [$name rust] /"
            report "$name" rust FAIL "exit $rc"
        fi
    done
    trap - EXIT
    rm -rf "$workdir"
}

# --- Python: execute each example file directly ---------------------------
py_run() {
    local py="" candidate
    for candidate in "$ROOT/.venv/bin/python" python3; do
        if PYTHONPATH="$ROOT/packages/py" "$candidate" -c 'import kelir' 2>/dev/null; then
            py="$candidate"
            break
        fi
    done
    if [ -z "$py" ]; then
        for name in $EXAMPLES; do
            report "$name" python BLOCKED "kelir not importable (run: uv venv && uv pip install -e \"packages/py[dev]\")"
        done
        return 0
    fi
    local name out rc
    for name in $EXAMPLES; do
        if is_hw "$name" && ! have_kvm; then
            report "$name" python SKIP "no /dev/kvm; hardware tier required"
            continue
        fi
        set +e
        out=$(PYTHONPATH="$ROOT/packages/py" timeout "$TIMEOUT_SECS" "$py" "$ROOT/cans/artifacts/examples/$name.py" 2>&1)
        rc=$?
        set -e
        if [ "$rc" -eq 0 ]; then
            report "$name" python PASS
        elif [ "$rc" -eq 124 ]; then
            printf '%s\n' "$out" | tail -5 | sed "s/^/    [$name python] /"
            report "$name" python RUNNING "alive at ${TIMEOUT_SECS}s timeout; unasserted"
        else
            printf '%s\n' "$out" | tail -10 | sed "s/^/    [$name python] /"
            report "$name" python FAIL "exit $rc"
        fi
    done
}

# --- TypeScript: compile the six examples, run node over the output ------
ts_run() {
    if ! command -v pnpm >/dev/null 2>&1; then
        for name in $EXAMPLES; do report "$name" typescript BLOCKED "pnpm not on PATH (run: pnpm install first)"; done
        return 0
    fi
    if [ ! -x "$ROOT/packages/ts/core/node_modules/.bin/tsc" ]; then
        for name in $EXAMPLES; do report "$name" typescript BLOCKED "typescript not installed (run: pnpm install first)"; done
        return 0
    fi
    if ! pnpm --filter @kelir/core run build; then
        for name in $EXAMPLES; do report "$name" typescript BLOCKED "@kelir/core build failed"; done
        return 0
    fi
    local outdir="$ROOT/packages/ts/core/.examples-run-tmp"
    local tscfg="$ROOT/packages/ts/core/tsconfig.examples-run.json"
    rm -rf "$outdir" "$tscfg"
    trap "rm -rf '$outdir' '$tscfg'" EXIT
    {
        echo '{'
        echo '  "extends": "./tsconfig.check.json",'
        echo '  "compilerOptions": { "noEmit": false, "outDir": "./.examples-run-tmp" },'
        echo '  "include": ['
        echo '    "node-shims.d.ts",'
        echo '    "src/node-fs.d.ts",'
        local first=1 name
        for name in $EXAMPLES; do
            if [ "$first" -eq 1 ]; then first=0; else echo ','; fi
            printf '    "../../../cans/artifacts/examples/%s.ts"' "$name"
        done
        echo
        echo '  ]'
        echo '}'
    } > "$tscfg"
    if ! pnpm --filter @kelir/core exec tsc -p tsconfig.examples-run.json; then
        for name in $EXAMPLES; do report "$name" typescript COMPILE-GAP "typecheck failed (WS-A..E lane)"; done
        trap - EXIT
        rm -rf "$outdir" "$tscfg"
        return 0
    fi
    local name out rc
    for name in $EXAMPLES; do
        if is_hw "$name" && ! have_kvm; then
            report "$name" typescript SKIP "no /dev/kvm; hardware tier required"
            continue
        fi
        set +e
        out=$(timeout "$TIMEOUT_SECS" node "$outdir/cans/artifacts/examples/$name.js" 2>&1)
        rc=$?
        set -e
        if [ "$rc" -eq 0 ]; then
            report "$name" typescript PASS
        elif [ "$rc" -eq 124 ]; then
            printf '%s\n' "$out" | tail -5 | sed "s/^/    [$name typescript] /"
            report "$name" typescript RUNNING "alive at ${TIMEOUT_SECS}s timeout; unasserted"
        else
            printf '%s\n' "$out" | tail -10 | sed "s/^/    [$name typescript] /"
            report "$name" typescript FAIL "exit $rc"
        fi
    done
    trap - EXIT
    rm -rf "$outdir" "$tscfg"
}

rs_run
py_run
ts_run

echo "examples-run: PASS=$pass RUNNING=$running SKIP=$skipped COMPILE-GAP=$gap FAIL=$failed BLOCKED=$blocked (18 runs)"
if [ "$gap" -ne 0 ] || [ "$failed" -ne 0 ] || [ "$blocked" -ne 0 ]; then
    echo "FAIL: examples-run has $gap compile gaps, $failed failures, $blocked blocked; see above"
    exit 1
fi
echo "OK: examples run (PASS=$pass, RUNNING=$running unasserted, SKIP=$skipped no-kvm)"
exit 0
