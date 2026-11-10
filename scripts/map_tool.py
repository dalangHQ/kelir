#!/usr/bin/env python3
"""Surgical editor for cans/artifacts/test-plan/case-to-test-map.yaml.

Rewrites only the target case block (byte-preserving for everything else)
and recomputes the header status counts. Shared tool so parallel agents
produce identical YAML formatting.

Usage:
  map_tool.py set <case_id> --status red --test-file F --test-fn N --reason "..."
  map_tool.py set <case_id> --status gap --reason "..."
  map_tool.py header
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
MAP = REPO / "cans/artifacts/test-plan/case-to-test-map.yaml"
CASES_DIR = REPO / "cans/artifacts/test-plan"


def yaml_scalar(value: str) -> str:
    """Quote only when needed for a single-line plain scalar."""
    if value == "":
        return "''"
    if re.match(r"^[\w./<>=: +-][\w./<>: +=@#()|,;'!\?\-\[\]]*$", value) and not (
        value.startswith(("-", "?", ":")) or ": " in value or " #" in value
    ):
        return value
    return "'" + value.replace("'", "''") + "'"


def find_block(lines: list[str], case_id: str) -> tuple[int, int]:
    """Return (start, end) line span of the `- case_id: <id>` entry."""
    start = None
    for i, line in enumerate(lines):
        m = re.match(r"^- case_id: (\S+)$", line)
        if m:
            if m.group(1) == case_id:
                start = i
            elif start is not None:
                return start, i
        if start is not None and line.startswith("harness_tests:"):
            return start, i
    if start is not None:
        return start, len(lines)
    raise SystemExit(f"case_id not found: {case_id}")


def parse_block(block: list[str]) -> dict[str, str | None]:
    entry: dict[str, str | None] = {}
    key = None
    for line in block:
        m = re.match(r"^- (\w+): (.*)$", line) or re.match(r"^  (\w+): ?(.*)$", line)
        if m:
            key = m.group(1)
            val = m.group(2).strip()
            entry[key] = None if val in ("null", "~", "") else val.strip("'\"")
        elif key and line.startswith("    ") and entry.get(key) is not None:
            entry[key] += " " + line.strip()
    return entry


def render_block(e: dict) -> list[str]:
    out = [f"- case_id: {e['case_id']}"]
    for k in ("case_file", "target_crate", "target_tier", "status"):
        out.append(f"  {k}: {e[k]}")
    out.append(f"  test_file: {e.get('test_file') or 'null'}")
    out.append(f"  test_fn: {e.get('test_fn') or 'null'}")
    if e.get("reason"):
        out.append("  reason: " + "'" + e["reason"].replace("'", "''") + "'")
    return out


def cmd_set(args: argparse.Namespace) -> None:
    lines = MAP.read_text().splitlines()
    start, end = find_block(lines, args.case_id)
    entry = parse_block(lines[start:end])
    if args.status:
        entry["status"] = args.status
    if args.test_file is not None:
        entry["test_file"] = args.test_file
    if args.test_fn is not None:
        entry["test_fn"] = args.test_fn
    if args.clear_test:
        entry["test_file"] = None
        entry["test_fn"] = None
    if args.reason is not None:
        entry["reason"] = " ".join(" ".join(args.reason).split())
    if args.status == "red":
        if not entry.get("test_file") or not entry.get("test_fn"):
            raise SystemExit("red status requires --test-file and --test-fn")
        if not entry.get("reason"):
            raise SystemExit("red status requires --reason (failure signature)")
    if args.status == "implemented":
        if not entry.get("test_file") or not entry.get("test_fn"):
            raise SystemExit("implemented status requires --test-file and --test-fn")
    new_block = render_block(entry)
    lines[start:end] = new_block
    MAP.write_text("\n".join(lines) + "\n")
    print(f"set {args.case_id}: status={entry['status']}")


def cmd_header(_: argparse.Namespace) -> None:
    lines = MAP.read_text().splitlines()
    counts = {"implemented": 0, "red": 0, "gap": 0}
    in_cases = False
    for line in lines:
        if line == "cases:":
            in_cases = True
            continue
        if in_cases:
            m = re.match(r"^  status: (\w+)$", line)
            if m and m.group(1) in counts:
                counts[m.group(1)] += 1
            if line.startswith("harness_tests:"):
                break
    total = sum(counts.values())
    for i, line in enumerate(lines):
        if re.match(r"^total_cases: \d+$", line):
            lines[i] = f"total_cases: {total}"
        elif re.match(r"^implemented: \d+$", line):
            lines[i] = f"implemented: {counts['implemented']}"
            if not any(re.match(r"^red: \d+$", x) for x in lines):
                lines.insert(i + 1, f"red: {counts['red']}")
                lines.insert(i + 2, f"gap: {counts['gap']}")
                if re.match(r"^gap: \d+$", lines[i + 3]):
                    del lines[i + 3]
            else:
                lines[i + 1] = f"red: {counts['red']}"
                lines[i + 2] = f"gap: {counts['gap']}"
            break
    MAP.write_text("\n".join(lines) + "\n")
    print(
        f"header: total={total} implemented={counts['implemented']} "
        f"red={counts['red']} gap={counts['gap']}"
    )


def main() -> None:
    p = argparse.ArgumentParser()
    sub = p.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("set")
    s.add_argument("case_id")
    s.add_argument("--status")
    s.add_argument("--test-file")
    s.add_argument("--test-fn")
    s.add_argument("--reason", nargs="+")
    s.add_argument("--clear-test", action="store_true")
    s.set_defaults(func=cmd_set)
    h = sub.add_parser("header")
    h.set_defaults(func=cmd_header)
    args = p.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
