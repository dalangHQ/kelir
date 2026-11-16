#!/usr/bin/env python3
"""check-spec-drift: WS-F3 doc<->code parity gate (L1/L2 skeleton).

Every depth<=1 normative leaf in the patch-series spec files must map to
a code symbol or test: a `see: <file>.md#<anchor>` reference from crates/,
packages/, cans/artifacts/examples/, scripts/, the justfile, or CI, or a
`law_reference:` in a test-plan case. Leaves with no reference are drift.

Drift recorded in scripts/spec-drift-gaps.yaml (one `file#slug<TAB>owner`
per line) passes; unrecorded drift fails. A recorded entry that is now
referenced, or whose leaf no longer exists, is stale and fails too, so the
inventory cannot rot. Depth<=1 is the section-leaf level: deeper bullets
are supporting text covered by their leaf.

The WS-F4 hygiene grep rides the same script: zero L3-GAP / TODO (L3) /
aspirational / TBD (no .* field) in cans/*.md, cans/plugins/*.md, and
cans/artifacts/examples/**. Out of scope by design (they are the L3-blocker
record itself, not places that hide a primitive): the test-plan registries
(cases, case-to-test-map, slop register), cans/artifacts/code|diagrams|
config, and cans/_collab history.
"""
import glob
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# see: cans/15-operations.md#Spec-drift
PATCHED = [
    "cans/01-architecture.md",
    "cans/02-plugin-model.md",
    "cans/03-sdk-architecture.md",
    "cans/05-storage-plane.md",
    "cans/06-storage-catalog.md",
    "cans/07-wire-plane.md",
    "cans/08-lifecycle.md",
    "cans/09-snapshots.md",
    "cans/10-attestation.md",
    "cans/11-protocol.md",
    "cans/12-adapters.md",
    "cans/14-observability.md",
    "cans/15-operations.md",
    "cans/plugins/container.md",
]

GAPS_FILE = ROOT / "scripts" / "spec-drift-gaps.yaml"

SEPS = set(["-", "_", "\u2010", "\u2011", "\u2012", "\u2013", "\u2014",
            "\u2015", "\u2043", "\u2212", "\uFE58", "\uFE63", "\uFF0D"])


def slugify(text):
    out = []
    for ch in text:
        if ch.isspace() or ch in SEPS:
            if out and out[-1] != "-":
                out.append("-")
        else:
            out.append(ch)
    return "".join(out).rstrip("-")


def norm(text):
    return slugify(text).lower()


def strip_pointer_suffix(title):
    pos = title.find("see:")
    if pos < 0:
        return title
    after = title[pos + 4:]
    lead = len(after) - len(after.lstrip())
    if lead == 0:
        return title
    words = after.split()
    token = words[0] if words else ""
    if "#" in token or ".md" in token or token.startswith("[") \
            or token.startswith("artifacts/"):
        return title[:pos].rstrip(" :(").strip()
    return title


def spec_leaves():
    leaves = {}
    for rel in PATCHED:
        path = ROOT / rel
        fenced = False
        for line in path.read_text(errors="ignore").splitlines():
            stripped = line.lstrip()
            if stripped.startswith("```") or stripped.startswith("~~~"):
                fenced = not fenced
                continue
            if fenced or not stripped.startswith("- "):
                continue
            depth = (len(line) - len(line.lstrip())) // 2
            if depth != 1:
                continue
            title = strip_pointer_suffix(stripped[2:].strip())
            if not title or re.match(r"^\[\d+\]:", title):
                continue
            leaves[f"{rel}#{norm(title)}"] = title
    return leaves


def collect_refs():
    refs = set()

    def add_ref(filename, anchor):
        if "/" not in filename:
            refs.add(("cans/" + filename, norm(anchor)))
            refs.add(("cans/plugins/" + filename, norm(anchor)))
        else:
            name = filename if filename.startswith("cans/") else "cans/" + filename
            refs.add((name, norm(anchor)))

    patterns = [
        "crates/**/*.rs", "crates/**/*.toml",
        "packages/**/*.ts", "packages/**/*.py",
        "cans/artifacts/examples/*.*",
        "scripts/*.sh", "scripts/*.py",
        "justfile", ".github/workflows/*.yml",
    ]
    see_re = re.compile(r"see:\s*([A-Za-z0-9_./-]+\.md)#([A-Za-z0-9_./-]+)")
    for pattern in patterns:
        for path in glob.glob(str(ROOT / pattern), recursive=True):
            text = Path(path).read_text(errors="ignore")
            for match in see_re.finditer(text):
                add_ref(match.group(1), match.group(2))
    law_re = re.compile(
        r"law_reference:\s*[\"']?([A-Za-z0-9_./-]+\.md)#([A-Za-z0-9_./-]+)")
    for path in glob.glob(
            str(ROOT / "cans/artifacts/test-plan/cases/**/*.yaml"),
            recursive=True):
        text = Path(path).read_text(errors="ignore")
        for match in law_re.finditer(text):
            add_ref(match.group(1), match.group(2))
    return refs


def load_gaps():
    gaps = {}
    if not GAPS_FILE.exists():
        return gaps
    for line in GAPS_FILE.read_text().splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        key, _, owner = line.partition("\t")
        gaps[key.strip()] = owner.strip()
    return gaps


HYGIENE_RE = re.compile(r"L3-GAP|TODO \(L3\)|aspirational|TBD \(no .* field")


def hygiene_hits():
    hits = []
    roots = [ROOT / "cans"]
    for base in roots:
        for path in sorted(base.rglob("*")):
            if not path.is_file():
                continue
            rel = str(path.relative_to(ROOT))
            if rel.startswith("cans/_collab/"):
                continue
            if rel.startswith("cans/artifacts/") and not rel.startswith(
                    "cans/artifacts/examples/"):
                continue
            if path.suffix not in (".md", ".rs", ".py", ".ts", ".toml", ".yaml"):
                continue
            for lineno, line in enumerate(
                    path.read_text(errors="ignore").splitlines(), 1):
                if HYGIENE_RE.search(line):
                    hits.append(f"{rel}:{lineno}: {line.strip()[:120]}")
    return hits


def main():
    leaves = spec_leaves()
    refs = collect_refs()
    drift = sorted(
        key for key in leaves
        if (key.split("#", 1)[0], key.split("#", 1)[1]) not in refs)
    gaps = load_gaps()
    unrecorded = [key for key in drift if key not in gaps]
    stale = sorted(
        key for key in gaps if key not in leaves or key not in drift)
    hits = hygiene_hits()
    fail = False
    print(f"spec-drift: {len(leaves)} leaves in {len(PATCHED)} patched files, "
          f"{len(drift)} unreferenced, {len(gaps)} recorded gaps")
    for key in unrecorded:
        print(f"DRIFT (unrecorded): {key} :: {leaves[key][:80]}")
    if unrecorded:
        fail = True
    for key in stale:
        print(f"DRIFT (stale inventory entry, remove it): {key} "
              f"(owner {gaps[key]})")
    if stale:
        fail = True
    for hit in hits:
        print(f"HYGIENE: {hit}")
    if hits:
        fail = True
    if fail:
        print("FAIL: spec-drift has unrecorded drift, stale inventory, "
              "or hygiene hits; see above")
        return 1
    print(f"OK: spec-drift clean ({len(drift)} recorded gaps, 0 hygiene hits)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
