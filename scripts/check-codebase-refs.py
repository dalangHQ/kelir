#!/usr/bin/env python3
# see: cans/_rules.yaml
import argparse
import fnmatch
import json
import re
import sys
from pathlib import Path, PurePath

sys.path.insert(0, str(Path(__file__).resolve().parent))

from codebase_refs import Exception, extract_refs, load_config, scan_repo, slugify

_PRUNE_DIRS = {".git", ".venv", "node_modules", "target", "__pycache__"}
_SIDECAR_REL = Path("scripts") / "codebase-refs-exceptions.yaml"
_FILE_RE = re.compile(r"^\s*-\s+file:\s+(.+?)\s*$")
_FIELD_RE = re.compile(r"^\s+(file|line|reason):\s+(.+?)\s*$")
_USAGE = "usage: check-codebase-refs.py {check [--root DIR] [--json] | refs <node> [--root DIR] [--json] | --help}"


class ConfigError(ValueError):
    pass


def default_root() -> Path:
    script_root = Path(__file__).resolve().parent.parent
    if script_root.is_dir():
        return script_root
    return Path.cwd()


def _unquote(value: str) -> str:
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in ("\"", "'"):
        return value[1:-1]
    return value


def load_sidecar_exceptions(root: Path) -> list[Exception]:
    path = root / _SIDECAR_REL
    if not path.is_file():
        return []
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        raise ConfigError(f"cannot read {path}: {exc}") from exc
    entries: list[dict] = []
    current: dict | None = None
    for line in text.splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        match = _FILE_RE.match(line)
        if match:
            current = {"file": _unquote(match.group(1))}
            entries.append(current)
            continue
        match = _FIELD_RE.match(line)
        if match and current is not None:
            key, value = match.group(1), _unquote(match.group(2))
            if key == "line":
                try:
                    current["line"] = int(value)
                except ValueError:
                    current["line"] = None
            else:
                current[key] = value
    exceptions = []
    for entry in entries:
        if not entry.get("file"):
            continue
        exceptions.append(
            Exception(
                file=str(entry["file"]),
                line=entry.get("line") if isinstance(entry.get("line"), int) else None,
                reason=str(entry.get("reason", "")),
            )
        )
    return exceptions


def load_exceptions(root: Path, config) -> dict[tuple[str, int | None], str]:
    merged: dict[tuple[str, int | None], str] = {}
    for exc in list(config.exceptions) + load_sidecar_exceptions(root):
        if not exc.file:
            continue
        merged.setdefault((exc.file, exc.line), exc.reason)
    return merged


def _glob_match(rel_posix: str, pattern: str) -> bool:
    path = PurePath(rel_posix)
    if path.match(pattern):
        return True
    if pattern.startswith("**/") and path.match(pattern[3:]):
        return True
    return fnmatch.fnmatchcase(rel_posix, pattern)


def _in_scope(rel_posix: str, config) -> bool:
    if not any(_glob_match(rel_posix, pat) for pat in config.include):
        return False
    return not any(_glob_match(rel_posix, pat) for pat in config.exclude)


def iter_scanned_files(root: Path, config):
    for root_entry in config.roots:
        entry = root / root_entry
        files: list[Path] = []
        if entry.is_file():
            files.append(entry)
        elif entry.is_dir():
            import os

            for dirpath, dirnames, filenames in os.walk(entry):
                dirnames[:] = sorted(d for d in dirnames if d not in _PRUNE_DIRS)
                for name in sorted(filenames):
                    files.append(Path(dirpath) / name)
        for path in files:
            try:
                rel = path.resolve().relative_to(root.resolve()).as_posix()
            except ValueError:
                continue
            if _in_scope(rel, config):
                yield rel, path


def count_refs(root: Path, config) -> int:
    total = 0
    for _rel, path in iter_scanned_files(root, config):
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for line in text.splitlines():
            total += len(extract_refs(line))
    return total


def find_occurrences(root: Path, config, target: str, slug: str | None):
    want_slug = slugify(slug).lower() if slug else None
    found = []
    for rel, path in iter_scanned_files(root, config):
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for line_no, line in enumerate(text.splitlines(), 1):
            for ref in extract_refs(line):
                if ref.target != target:
                    continue
                if want_slug is not None:
                    if ref.slug is None or slugify(ref.slug).lower() != want_slug:
                        continue
                found.append({"file": rel, "line": line_no, "raw": ref.raw})
    found.sort(key=lambda item: (item["file"], item["line"], item["raw"]))
    return found


def issue_dict(issue) -> dict:
    return {
        "type": issue.type,
        "file": issue.file,
        "line": issue.line,
        "raw": issue.raw,
        "targetFile": issue.targetFile,
        "targetSlug": issue.targetSlug,
        "reason": issue.reason,
    }


def target_display(issue) -> str:
    if issue.targetSlug:
        return f"{issue.targetFile}#{issue.targetSlug}"
    return issue.targetFile


def resolve_root(raw_root: str | None) -> Path:
    if raw_root:
        return Path(raw_root).expanduser()
    return default_root()


def fail(message: str, as_json: bool, extra: dict | None = None) -> int:
    if as_json:
        payload = {"ok": False, "exitCode": 2, "error": message}
        if extra:
            payload.update(extra)
        print(json.dumps(payload))
    else:
        print(f"check-codebase-refs: error: {message}", file=sys.stderr)
        print(_USAGE, file=sys.stderr)
    return 2


def cmd_check(args) -> int:
    root = resolve_root(args.root)
    if not root.is_dir():
        return fail(f"root is not a directory: {root}", args.as_json)
    try:
        config = load_config(root)
        exceptions = load_exceptions(root, config)
    except ConfigError as exc:
        return fail(str(exc), args.as_json)
    config.enabled = True
    issues = scan_repo(root, config)
    scanned = count_refs(root, config)
    broken, excepted = [], []
    for issue in issues:
        reason = exceptions.get((issue.file, issue.line))
        if reason is None:
            reason = exceptions.get((issue.file, None))
        if reason is None:
            broken.append(issue)
        else:
            entry = issue_dict(issue)
            entry["exceptionReason"] = reason
            excepted.append(entry)
    exit_code = 0 if not broken else 1
    if args.as_json:
        print(
            json.dumps(
                {
                    "ok": exit_code == 0,
                    "exitCode": exit_code,
                    "issues": [issue_dict(issue) for issue in broken],
                    "excepted": excepted,
                    "scanned": scanned,
                }
            )
        )
        return exit_code
    for issue in broken:
        print(
            f"{issue.file}:{issue.line}: CodebaseBrokenReference: "
            f"{issue.reason} {issue.raw} -> {target_display(issue)}"
        )
    for entry in excepted:
        print(f"{entry['file']}:{entry['line']}: excepted ({entry['exceptionReason']})")
    print(f"codebase-refs: {scanned} scanned, {len(broken)} broken, {len(excepted)} excepted")
    return exit_code


def normalize_node(node: str) -> tuple[str, str | None]:
    node = node.strip().strip("`\"'")
    if "#" in node:
        target, slug = node.split("#", 1)
        slug = slug or None
    else:
        target, slug = node, None
    target = target.strip().lstrip("/")
    if not target.startswith("cans/"):
        target = "cans/" + target
    return target, slug


def cmd_refs(args) -> int:
    root = resolve_root(args.root)
    if not root.is_dir():
        return fail(
            f"root is not a directory: {root}",
            args.as_json,
            {"node": args.node, "occurrences": []},
        )
    target, slug = normalize_node(args.node)
    if not target or target == "cans/":
        return fail(f"missing node target: {args.node!r}", args.as_json, {"node": args.node, "occurrences": []})
    if not (root / target).is_file():
        return fail(
            f"node target does not exist in the corpus: {target}",
            args.as_json,
            {"node": args.node, "occurrences": []},
        )
    config = load_config(root)
    config.enabled = True
    occurrences = find_occurrences(root, config, target, slug)
    if args.as_json:
        print(
            json.dumps(
                {
                    "ok": True,
                    "exitCode": 0,
                    "node": args.node,
                    "occurrences": occurrences,
                }
            )
        )
        return 0
    for item in occurrences:
        print(f"{item['file']}:{item['line']}: {item['raw']}")
    if not occurrences:
        print("0 occurrences")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="check-codebase-refs.py",
        description="Local checker for codebase see: pointers (RFC #66).",
    )
    sub = parser.add_subparsers(dest="command")
    check = sub.add_parser("check", help="check every codebase see: pointer resolves")
    check.add_argument("--root", default=None, help="repo root (default: derived from the script location)")
    check.add_argument("--json", dest="as_json", action="store_true", help="emit JSON")
    refs = sub.add_parser("refs", help="list codebase occurrences pointing at a node")
    refs.add_argument("node", help="target like cans/02-plugin-model.md[#Slug]")
    refs.add_argument("--root", default=None, help="repo root (default: derived from the script location)")
    refs.add_argument("--json", dest="as_json", action="store_true", help="emit JSON")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.command == "check":
        return cmd_check(args)
    if args.command == "refs":
        return cmd_refs(args)
    parser.print_usage(sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
