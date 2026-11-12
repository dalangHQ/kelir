#!/usr/bin/env bash
# check-examples: SSOT meta-gate. Compiles/typechecks the canonical examples
# in cans/artifacts/examples/ in all three languages. Rust examples are not
# workspace members, so they are checked via a throwaway crate in a temp dir
# (path deps on the workspace crates, Cargo.lock copied for version pins);
# each bin is checked separately because cargo stops scheduling new units
# after a failure, which would hide errors in later bins. Python is gated on
# py_compile (mypy strict over the examples reports only packaging noise:
# kelir ships no py.typed marker, so every import resolves to Any and no
# SSOT drift is visible; see the gate report). TypeScript runs through the
# fixed packages/ts/core/tsconfig.check.json, which includes the cans
# examples; the @kelir/core dist build runs first because the tests import
# ../dist/index.js.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
fail=0

# --- Rust: compile-check cans/artifacts/examples/*.rs -----------------------
rs_check() {
    local workdir targetdir edition
    workdir="$(mktemp -d "${TMPDIR:-/tmp}/kelir-examples-check.XXXXXX")"
    # Persistent worktree-local target dir: /tmp is a small tmpfs and a
    # fresh full dep build does not fit there; this also makes repeat
    # runs incremental. It is a cache only; never committed (target/ is
    # gitignored) and never cleaned by this gate.
    targetdir="$ROOT/target/check-examples"
    # Keep the temp crate dir only when the leg fails, for debugging.
    trap 'rm -rf "$workdir"' EXIT
    edition="$(sed -n 's/^edition *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
    edition="${edition:-2024}"
    mkdir -p "$workdir/gen"
    {
        echo '[package]'
        echo 'name = "kelir-examples-check"'
        echo 'version = "0.0.0"'
        echo "edition = \"$edition\""
        echo
        # Empty [workspace]: the temp crate may live under target/ (TMPDIR
        # points there because /tmp is a small tmpfs), i.e. inside the repo
        # workspace root; without this cargo adopts it into the workspace.
        echo '[workspace]'
        echo
        for f in "$ROOT"/cans/artifacts/examples/*.rs; do
            name="$(basename "$f" .rs)"
            if grep -Eq '^\s*(async\s+)?fn main\s*\(' "$f"; then
                printf '[[bin]]\nname = "%s"\npath = "%s"\n\n' "$name" "$f"
            else
                printf '#[path = "%s"]\nmod example;\nfn main() {}\n' "$f" > "$workdir/gen/$name-bin.rs"
                printf '[[bin]]\nname = "%s"\npath = "gen/%s-bin.rs"\n\n' "$name" "$name"
            fi
        done
        echo '[dependencies]'
        echo "kelir = { path = \"$ROOT/crates/kelir\", features = [\"all-plugins\"] }"
        echo "kelir-interface = { path = \"$ROOT/crates/kelir-interface\" }"
        echo 'tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync", "fs", "io-util", "net"] }'
        echo 'serde = { version = "1.0", features = ["derive"] }'
        echo 'serde_json = "1.0"'
        echo 'libc = "0.2"'
        echo 'async-trait = "0.1"'
        echo 'ciborium = "0.2"'
        echo '# NOTE: an example importing a new external crate needs it added here.'
    } > "$workdir/Cargo.toml"
    if [ -f "$ROOT/Cargo.lock" ]; then
        cp "$ROOT/Cargo.lock" "$workdir/Cargo.lock"
    fi
    if [ -z "${LIBCLANG_PATH:-}" ] && [ -f "$HOME/.local/lib/python3.12/site-packages/clang/native/libclang.so" ]; then
        export LIBCLANG_PATH="$HOME/.local/lib/python3.12/site-packages/clang/native/libclang.so"
    fi
    export CARGO_TARGET_DIR="$targetdir"
    local rs_fail=0 bin
    for f in "$ROOT"/cans/artifacts/examples/*.rs; do
        bin="$(basename "$f" .rs)"
        if ! (cd "$workdir" && cargo +stable check --bin "$bin" 2>&1); then
            echo "FAIL: rust example does not compile: cans/artifacts/examples/$bin.rs"
            rs_fail=1
        fi
    done
    if [ "$rs_fail" -eq 0 ]; then
        echo "OK: rust examples compile"
    fi
    trap - EXIT
    rm -rf "$workdir"
    return "$rs_fail"
}

# --- Python: syntax-check cans/artifacts/examples/*.py ----------------------
# Builtin compile() only parses: no bytecode is written, so no __pycache__
# lands next to the examples. (py_compile refuses cfile=/dev/null and
# writes __pycache__ by default.)
py_check() {
    if python3 -c 'import sys; [compile(open(f).read(), f, "exec") for f in sys.argv[1:]]' "$ROOT"/cans/artifacts/examples/*.py; then
        echo "OK: python examples compile"
        return 0
    fi
    echo "FAIL: python examples do not compile"
    return 1
}

# --- TypeScript: typecheck the cans examples via tsconfig.check.json -------
ts_check() {
    if ! command -v pnpm >/dev/null 2>&1; then
        echo "FAIL: pnpm not on PATH (run: pnpm install first)"
        return 1
    fi
    if [ ! -x "$ROOT/packages/ts/core/node_modules/.bin/tsc" ]; then
        echo "FAIL: typescript not installed (run: pnpm install first)"
        return 1
    fi
    if ! pnpm --filter @kelir/core run build; then
        echo "FAIL: @kelir/core build failed"
        return 1
    fi
    if pnpm --filter @kelir/core exec tsc -p tsconfig.check.json; then
        echo "OK: typescript examples typecheck"
        return 0
    fi
    echo "FAIL: typescript examples do not typecheck"
    return 1
}

rs_check || fail=1
py_check || fail=1
ts_check || fail=1

if [ "$fail" -eq 0 ]; then
    echo "OK: examples compile in all three languages"
fi
exit "$fail"
