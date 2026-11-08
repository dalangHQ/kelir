#!/usr/bin/env bash
# check-codebase-refs-selftest: proves the codebase-refs checker (RFC #66)
# against synthetic trees in scripts/fixtures/codebase-refs/. Each case is
# copied to a temp dir and checked with --root pointed at it; the real tree
# is never used for a negative case. Exit 1 on any wrong behaviour, OK on pass.
# Override the CLI under test with CODEBASE_REFS_CLI (default: the part2 CLI).
set -euo pipefail
cd "$(dirname "$0")/.."

CLI="${CODEBASE_REFS_CLI:-scripts/check-codebase-refs.py}"
PY="${PYTHON:-python3}"
FIXTURES="scripts/fixtures/codebase-refs"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
fail=0
passed=0

if [ ! -f "$CLI" ]; then
    echo "FAIL: checker CLI missing: $CLI (part2 has not landed it yet)"
    exit 1
fi

# expect_check <case> <want_exit> <jq_filter>: run `check --root <tmp/case>
# --json`, assert the exit code and the jq filter over the JSON output.
# Leaves the last output in LAST_OUT for extra assertions.
expect_check() {
    local case="$1" want="$2" filter="$3"
    cp -r "$FIXTURES/$case" "$TMP/$case"
    local code=0
    LAST_OUT=$("$PY" "$CLI" check --root "$TMP/$case" --json 2>&1) || code=$?
    if [ "$code" -ne "$want" ] || ! jq -e "$filter" <<<"$LAST_OUT" >/dev/null 2>&1; then
        echo "FAIL: $case: want exit $want + $filter, got exit $code:"
        echo "$LAST_OUT"
        fail=1
    else
        passed=$((passed + 1))
    fi
}

expect_check a-bad-file 1 '.ok == false and .exitCode == 1 and (.issues | length) == 1 and .issues[0].type == "CodebaseBrokenReference" and .issues[0].reason == "FileNotFound"'
expect_check b-bad-anchor 1 '.ok == false and .exitCode == 1 and (.issues | length) == 1 and .issues[0].type == "CodebaseBrokenReference" and .issues[0].reason == "AnchorNotFound"'
expect_check c-missing-anchor 1 '.ok == false and .exitCode == 1 and (.issues | length) == 1 and .issues[0].type == "CodebaseBrokenReference" and .issues[0].reason == "AnchorMissing"'
expect_check d-good-md 0 '.ok == true and .exitCode == 0 and (.issues | length) == 0'
expect_check e-good-yaml 0 '.ok == true and .exitCode == 0 and (.issues | length) == 0'
expect_check f-good-txt 0 '.ok == true and .exitCode == 0 and (.issues | length) == 0'
expect_check g-good-code 0 '.ok == true and .exitCode == 0 and (.issues | length) == 0'
expect_check h-trailing-prose 0 '.ok == true and .exitCode == 0 and (.issues | length) == 0'

# i-exceptions: the two anchor-less pointers are excepted by the fixture
# sidecar, so the run passes — but the exceptions must be REPORTED, not
# silent: the JSON names them (an excepted/exceptions marker + the file).
expect_check i-exceptions 0 '.ok == true and .exitCode == 0 and (.issues | length) == 0'
if ! grep -qi 'except' <<<"$LAST_OUT" || ! grep -q 'crates/x.rs' <<<"$LAST_OUT"; then
    echo "FAIL: i-exceptions: excepted pointers not reported in JSON output:"
    echo "$LAST_OUT"
    fail=1
    passed=$((passed - 1))
fi

# j-refs-query: `refs <node>` lists both occurrences as file:line + raw.
cp -r "$FIXTURES/j-refs-query" "$TMP/j-refs-query"
code=0
REFS_OUT=$("$PY" "$CLI" refs 'cans/06-storage-catalog.md#Entity-model' --root "$TMP/j-refs-query" 2>&1) || code=$?
REFS_JSON=""
code_json=0
REFS_JSON=$("$PY" "$CLI" refs 'cans/06-storage-catalog.md#Entity-model' --root "$TMP/j-refs-query" --json 2>&1) || code_json=$?
if [ "$code" -ne 0 ] || [ "$code_json" -ne 0 ] \
    || ! grep -q 'crates/a.rs:1' <<<"$REFS_OUT" \
    || ! grep -q 'crates/b.rs:3' <<<"$REFS_OUT" \
    || ! jq -e . <<<"$REFS_JSON" >/dev/null 2>&1 \
    || ! grep -q 'crates/a.rs' <<<"$REFS_JSON" \
    || ! grep -q 'crates/b.rs' <<<"$REFS_JSON"; then
    echo "FAIL: j-refs-query: want both occurrences (crates/a.rs:1, crates/b.rs:3):"
    echo "--- human (exit $code):"; echo "$REFS_OUT"
    echo "--- json (exit $code_json):"; echo "$REFS_JSON"
    fail=1
else
    passed=$((passed + 1))
fi

if [ "$fail" -eq 0 ]; then
    echo "OK: codebase-refs self-test passed ($passed/10 cases)"
    exit 0
fi
echo "FAIL: codebase-refs self-test failed ($passed/10 cases passed)"
exit 1
