#!/usr/bin/env bash
# check-plugin-coverage: asserts the 02-plugin-model.md coverage invariant.
# Every community plugin (one spec file in cans/plugins/, except
# commercial.md) names a demonstrating example in the coverage matrix or
# the userland SDK policy demonstrations, or is explicitly marked in 02
# as having none yet; every named example file exists in
# cans/artifacts/examples/. Exit 1 on any orphan or gap.
set -euo pipefail
cd "$(dirname "$0")/.."

SPEC=cans/02-plugin-model.md
EXDIR=cans/artifacts/examples
fail=0

# Roster: one plugin spec file per community plugin.
mapfile -t plugins < <(ls cans/plugins/*.md | xargs -n1 basename | sed 's/\.md$//' | grep -v '^commercial$' | sort)

# Coverage text: the matrix section. The "Userland SDK policy demonstrations"
# section no longer exists as its own section; its content moved inline into
# the matrix (db-proxy, supply-chain carry a "(userland SDK policy)" note).
matrix=$(awk '/Coverage matrix \(plugin -> demonstrating example\)/,/Non-plugin surface examples/' "$SPEC")
coverage="$matrix"

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

for p in "${plugins[@]}"; do
    if ! grep -qw "$p" <<<"$coverage"; then
        if grep -q "$p.*none yet\|none yet.*$p" "$SPEC"; then
            echo "OK (marked none yet): $p"
            continue
        fi
        echo "FAIL: $p names no demonstrating example in 02 and is not marked none yet"
        fail=1
    fi
done

# Every example file named in the two sections must exist.
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
    echo "OK: plugin coverage holds for ${#plugins[@]} community plugins"
fi
exit "$fail"
