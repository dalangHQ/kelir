#!/usr/bin/env bash
# check-examples-canonical: asserts cans/artifacts/examples/ is the single
# canonical home for demonstrating examples. Exit 1 if any examples/ dir
# exists under crates/ or packages/ (excluding target/), or if any example
# file named in the cans/02-plugin-model.md coverage sections is missing
# from cans/artifacts/examples/.
set -euo pipefail
cd "$(dirname "$0")/.."

SPEC=cans/02-plugin-model.md
EXDIR=cans/artifacts/examples
fail=0

# No scattered examples/ dirs under crates/ or packages/.
while IFS= read -r -d '' dir; do
    echo "FAIL: scattered examples dir exists (canonical home is $EXDIR): $dir"
    fail=1
done < <(find crates packages -type d -name examples -not -path "*/target/*" -print0)

# Every example file named in the coverage sections must exist in EXDIR.
# Sections mirror scripts/check-plugin-coverage.sh.
matrix=$(awk '/Coverage matrix \(community plugins/,/Userland SDK policy demonstrations/' "$SPEC")
demos=$(awk '/Userland SDK policy demonstrations/,/Non-plugin surface examples/' "$SPEC")
coverage="$matrix
$demos"

expand_braces() {
    local tok="$1"
    case "$tok" in
        *.\{*\})
            local base="${tok%%.\{*}"
            local inner="${tok##*.\{}"
            inner="${inner%\}}"
            local IFS=,
            for ext in $inner; do printf '%s.%s\n' "$base" "$ext"; done
            ;;
        *) printf '%s\n' "$tok" ;;
    esac
}

while read -r tok; do
    [ -n "$tok" ] || continue
    while read -r f; do
        if [ ! -e "$EXDIR/$f" ]; then
            echo "FAIL: example file missing: $EXDIR/$f"
            fail=1
        fi
    done < <(expand_braces "$tok")
done < <(grep -oE ': [A-Za-z0-9_.-]+(\{[A-Za-z,]+\})?' <<<"$coverage" | sed 's/^: //')

if [ "$fail" -eq 0 ]; then
    echo "OK: examples are canonical in $EXDIR"
fi
exit "$fail"
