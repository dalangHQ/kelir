#!/usr/bin/env bash
set -euo pipefail

BANNED_REGEX='(formerly|replaces the old|kept for compatibility|kept as-is|aa0e011|issue #[0-9]+|was [0-9]+|deleted on that basis|backward-compatible|compatibility views|retained as)'

MATCHES=$(grep -rnEI "$BANNED_REGEX" cans/ scripts/ --exclude='check-canon.sh' --exclude-dir='_collab' || true)

if [ -n "$MATCHES" ]; then
    echo "ERROR: Non-canonical historical debt detected:"
    echo "$MATCHES"
    exit 1
fi
echo "OK: Corpus is 100% Day-1 canonical."
