#!/usr/bin/env bash
# check-examples-thin: thin-consumer gate. Canonical examples in
# cans/artifacts/examples/ import ONLY the public SDK surface plus the
# standard library; language-idiomatic constructs come through the SDK,
# not through external crates. Rust: kelir, kelir-interface, std.
# TypeScript: @kelir/core, plus relative paths (none exist today).
# Python: kelir plus the standard library (sys.stdlib_module_names).
# A violation prints the offending file:line and fails the gate.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
fail=0

# --- Rust: grep ^use lines in cans/artifacts/examples/*.rs ------------------
# Indented use lines inside functions are included: a fn-body import of
# an external crate would break the rule just as a top-level one would.
rs_check() {
    local rs_fail=0 f line line_no crate
    for f in "$ROOT"/cans/artifacts/examples/*.rs; do
        while IFS= read -r line; do
            line_no="${line%%:*}"
            crate="$(printf '%s' "${line#*:}" | sed -E 's/^[[:space:]]*use[[:space:]]+([A-Za-z_][A-Za-z0-9_]*).*/\1/')"
            case "$crate" in
                kelir|kelir_interface|std) ;;
                *) echo "FAIL: rust example imports non-SDK crate: ${f#"$ROOT"/}:$line_no: $crate"; rs_fail=1 ;;
            esac
        done < <(grep -nE '^[[:space:]]*use[[:space:]]+[A-Za-z_]' "$f" || true)
    done
    # Fully-qualified paths (#[tokio::main], libc::getuid) escape the use
    # grep; a crate path not prefixed by kelir:: is a violation. The
    # [^:] guard lets kelir::serde_json and kelir::serde through.
    for f in "$ROOT"/cans/artifacts/examples/*.rs; do
        while IFS= read -r line; do
            echo "FAIL: rust example uses non-SDK crate path: ${f#"$ROOT"/}:${line%%:*}: ${line#*:}"
            rs_fail=1
        done < <(grep -nE '(^|[^A-Za-z0-9_:])(tokio|libc|serde|serde_json|ciborium)::' "$f" || true)
    done
    if [ "$rs_fail" -eq 0 ]; then
        echo "OK: rust examples import only kelir, kelir-interface, std"
    fi
    return "$rs_fail"
}

# --- TypeScript: grep from "..." lines in cans/artifacts/examples/*.ts ------
# Multi-line imports put from on the closing line, so the match is
# unanchored; bare side-effect imports (import "...") are caught too.
ts_check() {
    local ts_fail=0 f line line_no spec
    for f in "$ROOT"/cans/artifacts/examples/*.ts; do
        while IFS= read -r line; do
            line_no="${line%%:*}"
            spec="$(printf '%s' "${line#*:}" | sed -E "s/.*(from|import)[[:space:]]+['\"]([^'\"]+)['\"].*/\2/")"
            case "$spec" in
                @kelir/core|./*|../*) ;;
                *) echo "FAIL: typescript example imports non-SDK package: ${f#"$ROOT"/}:$line_no: $spec"; ts_fail=1 ;;
            esac
        done < <(grep -nE "(from|import)[[:space:]]+['\"]" "$f" || true)
    done
    if [ "$ts_fail" -eq 0 ]; then
        echo "OK: typescript examples import only @kelir/core and relative paths"
    fi
    return "$ts_fail"
}

# --- Python: grep top-level and fn-body imports in *.py ---------------------
# from X import ... contributes X; import a, b contributes each name;
# only the first dotted segment is checked (kelir.plugins.chaos is kelir).
py_check() {
    local py_fail=0 f line line_no raw mod first
    local stdlib
    if ! stdlib="$(python3 -c 'import sys; print("\n".join(sorted(sys.stdlib_module_names)))')"; then
        echo "FAIL: python3 not on PATH (needed for the stdlib module list)"
        return 1
    fi
    for f in "$ROOT"/cans/artifacts/examples/*.py; do
        while IFS= read -r line; do
            line_no="${line%%:*}"
            raw="$(printf '%s' "${line#*:}" | sed -E -e 's/^[[:space:]]*from[[:space:]]+([A-Za-z_][A-Za-z0-9_.]*)[[:space:]]+import.*/\1/' -e 's/^[[:space:]]*import[[:space:]]+//; s/[[:space:]]+as[[:space:]]+[A-Za-z_][A-Za-z0-9_]*//g; s/,/ /g')"
            for mod in $raw; do
                first="${mod%%.*}"
                case "$first" in
                    kelir) ;;
                    *)
                        if ! grep -qxF "$first" <<<"$stdlib"; then
                            echo "FAIL: python example imports non-SDK module: ${f#"$ROOT"/}:$line_no: $first"
                            py_fail=1
                        fi
                        ;;
                esac
            done
        done < <(grep -nE '^[[:space:]]*(import|from)[[:space:]]+[A-Za-z_]' "$f" || true)
    done
    if [ "$py_fail" -eq 0 ]; then
        echo "OK: python examples import only kelir and the standard library"
    fi
    return "$py_fail"
}

rs_check || fail=1
ts_check || fail=1
py_check || fail=1

if [ "$fail" -eq 0 ]; then
    echo "OK: examples are thin consumers in all three languages"
fi
exit "$fail"
