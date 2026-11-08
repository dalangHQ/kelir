#!/usr/bin/env python3
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import codebase_refs as cr

REPO = Path(__file__).resolve().parent.parent
SCOPE_SUFFIXES = {".rs", ".ts", ".py"}
PRUNE = {".venv", "node_modules", "target", ".git", "__pycache__"}


def ensure_cans() -> bool:
    if shutil.which("cans"):
        return True
    print("cans binary missing, installing cans-spec@latest via npm ...")
    proc = subprocess.run(
        ["npm", "install", "-g", "cans-spec@latest"],
        capture_output=True,
        text=True,
        timeout=600,
    )
    if proc.returncode != 0:
        print("npm install -g cans-spec@latest failed:")
        print(proc.stdout[-2000:])
        print(proc.stderr[-2000:])
        return False
    return shutil.which("cans") is not None


def scope_files():
    files = []
    for root in (REPO / "crates", REPO / "packages" / "ts", REPO / "packages" / "py"):
        for path in sorted(root.rglob("*")):
            if path.is_dir() or path.suffix not in SCOPE_SUFFIXES:
                continue
            if PRUNE.intersection(path.parts):
                continue
            files.append(path)
    files.append(REPO / "AGENTS.md")
    return files


def cans_slug(text: str) -> str | None:
    if text.startswith("-"):
        return None
    try:
        proc = subprocess.run(
            ["cans", "slug", text],
            capture_output=True,
            text=True,
            timeout=60,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if proc.returncode != 0:
        return None
    return proc.stdout.strip("\n")


def main() -> int:
    if not ensure_cans():
        print("ERROR: cans binary unavailable")
        return 2
    anchors = []
    bare = 0
    for path in scope_files():
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for line_no, line in enumerate(text.splitlines(), 1):
            for ref in cr.extract_refs(line):
                ref.file = path.relative_to(REPO).as_posix()
                ref.line_no = line_no
                if ref.slug is None:
                    bare += 1
                else:
                    anchors.append(ref)
    print(f"anchored refs: {len(anchors)}, bare refs: {bare}")

    need_slug: dict[str, list[cr.Ref]] = {}
    failures: list[str] = []
    fallback_matched: list[str] = []
    matched = 0
    for ref in anchors:
        target = REPO / ref.target.lstrip("/")
        suffix = target.suffix.lower()
        texts = cr._leaf_texts(target)
        if suffix in (".yaml", ".yml"):
            if ref.slug in texts:
                matched += 1
            else:
                failures.append(
                    f"MISSING {ref.file}:{ref.line_no} {ref.raw} "
                    f"(no dotted path {ref.slug} in {ref.target})"
                )
            continue
        found = [t for t in texts if cr._norm(cr.slugify(t)) == cr._norm(ref.slug)]
        if not found:
            if suffix in (".rs", ".ts", ".py"):
                try:
                    words = cr._CODE_WORD_RE.findall(
                        target.read_text(encoding="utf-8", errors="replace")
                    )
                except OSError:
                    words = []
                if any(cr._norm(w) == cr._norm(ref.slug) for w in words):
                    fallback_matched.append(f"{ref.file}:{ref.line_no} {ref.raw}")
                    matched += 1
                    continue
            failures.append(
                f"MISSING {ref.file}:{ref.line_no} {ref.raw} "
                f"(no leaf matching {ref.slug} in {ref.target})"
            )
            continue
        for raw in dict.fromkeys(found):
            need_slug.setdefault(raw, []).append(ref)

    raws = sorted(need_slug)
    print(f"unique leaf texts to slug-check: {len(raws)}")
    with ThreadPoolExecutor(max_workers=16) as pool:
        canonical = dict(zip(raws, pool.map(cans_slug, raws)))
    for raw, refs in sorted(need_slug.items()):
        mine = cr.slugify(raw)
        theirs = canonical.get(raw)
        if theirs is None:
            failures.append(f"CANS-FAIL (no cans slug output for leaf text {raw!r})")
            continue
        if mine != theirs:
            failures.append(
                f"SLUG-DISAGREE leaf text {raw!r}: codebase_refs={mine!r} cans={theirs!r}"
            )
            continue
        for ref in refs:
            if cr._norm(ref.slug) == cr._norm(theirs):
                matched += 1
            else:
                failures.append(
                    f"ANCHOR-SPELLING {ref.file}:{ref.line_no} {ref.raw} "
                    f"(canonical slug {theirs!r})"
                )
    print(f"anchors checked: {len(anchors)}, matched: {matched}, mismatched: {len(failures)}")
    print(f"matched via code name-presence fallback (no declared leaf): {len(fallback_matched)}")
    for entry in fallback_matched:
        print(f"  FALLBACK {entry}")
    for failure in failures:
        print(failure)
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
