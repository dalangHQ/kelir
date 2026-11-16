#!/usr/bin/env bash
# check-spec-drift: WS-F3 doc<->code parity gate. Runs the Python drift
# check over the patch-series spec files (see scripts/check-spec-drift.py
# for the scope, the inventory file, and the hygiene-grep carve-outs).
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/check-spec-drift.py
