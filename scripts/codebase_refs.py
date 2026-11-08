#!/usr/bin/env python3
# see: cans/_rules.yaml
import fnmatch
import os
import re
from dataclasses import dataclass, field
from pathlib import Path, PurePath

_DASHES = frozenset(
    [
        "-",
        "_",
        "\u2010",
        "\u2011",
        "\u2012",
        "\u2013",
        "\u2014",
        "\u2015",
        "\u2043",
        "\u2212",
        "\ufe58",
        "\ufe63",
        "\uff0d",
    ]
)
_REF_RE = re.compile(r"see:\s+(cans/\S+)")
_MD_BULLET_RE = re.compile(r"^\s*-\s+(.*?)\s*$")
_MD_HEADING_RE = re.compile(r"^#{1,6}\s+(.*?)\s*#*\s*$")
_YAML_KEY_RE = re.compile(r"^(\s*)(?:-\s+)?([A-Za-z0-9_][A-Za-z0-9_.\-]*):(?:\s+.*)?$")
_YAML_LIST_RE = re.compile(r"^(\s*)-\s+([A-Za-z0-9_][A-Za-z0-9_.\-]*):(?:\s+.*)?$")
_TXT_BRACKET_RE = re.compile(r"\[([^\]]+)\]")
_TXT_TOKEN_STRIP = " \t\u2502\u251c\u2514\u250c\u2510\u2518\u2524\u252c\u2534\u253c\u2500\u2501\u25b6\u25bc\u25c4\u25b2()[]{},;:#\"'=<>|/`"
_CODE_WORD_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
_PRUNE_DIRS = {".git", ".venv", "node_modules", "target", "__pycache__"}


def slugify(text: str) -> str:
    out = []
    for ch in text:
        if ch.isspace() or ch in _DASHES:
            if out and out[-1] != "-":
                out.append("-")
        else:
            out.append(ch)
    while out and out[-1] == "-":
        out.pop()
    return "".join(out)


def _norm(text: str) -> str:
    return slugify(text).lower()


@dataclass
class Ref:
    raw: str
    target: str
    slug: str | None = None
    line_no: int = 0
    file: str = ""


@dataclass
class Exception:
    file: str
    line: int | None = None
    reason: str = ""


@dataclass
class Config:
    enabled: bool = True
    roots: list[str] = field(
        default_factory=lambda: ["crates", "packages/ts", "packages/py", "AGENTS.md"]
    )
    include: list[str] = field(
        default_factory=lambda: ["**/*.rs", "**/*.ts", "**/*.py", "**/*.md"]
    )
    exclude: list[str] = field(
        default_factory=lambda: ["**/.venv/**", "**/node_modules/**", "**/target/**"]
    )
    severity: str = "error"
    exceptions: list[Exception] = field(default_factory=list)


@dataclass
class Issue:
    type: str = "CodebaseBrokenReference"
    file: str = ""
    line: int = 0
    raw: str = ""
    targetFile: str = ""
    targetSlug: str | None = None
    reason: str = ""


def extract_refs(line: str) -> list[Ref]:
    refs = []
    for match in _REF_RE.finditer(line):
        raw = match.group(1).rstrip("`")
        if "#" in raw:
            target, slug = raw.split("#", 1)
            slug = slug or None
        else:
            target, slug = raw, None
        refs.append(Ref(raw=raw, target=target, slug=slug))
    return refs


def _strip_comment(value: str) -> str:
    value = value.strip()
    if value.startswith('"') and value.endswith('"') and len(value) >= 2:
        return value[1:-1]
    if value.startswith("'") and value.endswith("'") and len(value) >= 2:
        return value[1:-1]
    if " #" in value:
        value = value.split(" #", 1)[0].rstrip()
    return value


def _split_inline_list(text: str) -> list[str]:
    text = text.strip()
    if not (text.startswith("[") and text.endswith("]")):
        return [_strip_comment(text)] if text else []
    inner = text[1:-1].strip()
    if not inner:
        return []
    items, cur, quote = [], "", ""
    for ch in inner:
        if quote:
            cur += ch
            if ch == quote:
                quote = ""
        elif ch in "\"'":
            quote, cur = ch, cur + ch
        elif ch == ",":
            items.append(_strip_comment(cur))
            cur = ""
        else:
            cur += ch
    if cur.strip():
        items.append(_strip_comment(cur))
    return items


def _scalar(value: str):
    value = _strip_comment(value)
    low = value.lower()
    if low in ("true", "yes", "on"):
        return True
    if low in ("false", "no", "off"):
        return False
    if low in ("null", "~", ""):
        return None
    if value.startswith("["):
        return _split_inline_list(value)
    try:
        return int(value)
    except ValueError:
        return value


def _parse_block(lines: list[str], start: int, indent: int):
    node: dict = {}
    items: list = []
    i = start
    while i < len(lines):
        line = lines[i]
        if not line.strip() or line.lstrip().startswith("#"):
            i += 1
            continue
        cur_indent = len(line) - len(line.lstrip(" "))
        if cur_indent < indent:
            break
        stripped = line.strip()
        if stripped.startswith("- ") or stripped == "-":
            item_text = stripped[2:].strip() if stripped.startswith("- ") else ""
            if item_text and ":" in item_text and not item_text.startswith(("[", "{", '"', "'")):
                key, _, val = item_text.partition(":")
                key = key.strip().strip("\"'")
                sub, i = _parse_block(lines, i + 1, cur_indent + 2)
                entry: dict = {}
                if val.strip():
                    entry[key] = _scalar(val)
                if isinstance(sub, dict):
                    entry.update(sub)
                elif val.strip() == "":
                    entry[key] = sub
                items.append(entry)
            else:
                items.append(_scalar(item_text))
                i += 1
            continue
        if ":" not in stripped:
            i += 1
            continue
        key, _, val = stripped.partition(":")
        key = key.strip().strip("\"'")
        if val.strip():
            node[key] = _scalar(val)
            i += 1
        else:
            sub, i = _parse_block(lines, i + 1, cur_indent + 1)
            node[key] = sub
    if items:
        return items, i
    return node, i


def _codebase_block(text: str) -> dict:
    lines = text.splitlines()
    start = None
    for i, line in enumerate(lines):
        if re.match(r"^codebase:\s*(#.*)?$", line):
            start = i + 1
            break
    if start is None:
        return {}
    block = []
    for line in lines[start:]:
        if not line.strip() or line.lstrip().startswith("#"):
            block.append(line)
            continue
        if not line.startswith((" ", "\t")):
            break
        block.append(line)
    parsed, _ = _parse_block(block, 0, 0)
    return parsed if isinstance(parsed, dict) else {}


def _as_str_list(value, default: list[str]) -> list[str]:
    if isinstance(value, list):
        return [str(v) for v in value]
    if isinstance(value, str) and value:
        return [value]
    return default


def load_config(repo_root: str | Path) -> Config:
    defaults = Config()
    rules_path = Path(repo_root) / "cans" / "_rules.yaml"
    try:
        text = rules_path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return defaults
    try:
        block = _codebase_block(text)
    except (ValueError, IndexError):
        return defaults
    if not block:
        return defaults
    config = Config()
    if isinstance(block.get("enabled"), bool):
        config.enabled = block["enabled"]
    config.roots = _as_str_list(block.get("roots"), defaults.roots)
    config.include = _as_str_list(block.get("include"), defaults.include)
    config.exclude = _as_str_list(block.get("exclude"), defaults.exclude)
    if isinstance(block.get("severity"), str) and block["severity"]:
        config.severity = block["severity"]
    exceptions = block.get("exceptions")
    if isinstance(exceptions, list):
        for entry in exceptions:
            if not isinstance(entry, dict):
                continue
            line = entry.get("line")
            config.exceptions.append(
                Exception(
                    file=str(entry.get("file", "")),
                    line=line if isinstance(line, int) else None,
                    reason=str(entry.get("reason", "")),
                )
            )
    return config


_MD_SEE_RE = re.compile(r"see:\s+(\S+)")


def _strip_pointer_suffix(text: str) -> str:
    m = _MD_SEE_RE.search(text)
    if m:
        token = m.group(1)
        if token.startswith(("#", "[")) or ".md" in token or "#" in token or token.startswith("artifacts/"):
            return text[: m.start()].rstrip(" :(").strip()
    return text


def _md_texts(text: str) -> list[str]:
    texts, fenced = [], False
    for line in text.splitlines():
        if line.strip().startswith("```") or line.strip().startswith("~~~"):
            fenced = not fenced
            continue
        if fenced:
            continue
        m = _MD_BULLET_RE.match(line)
        if m and m.group(1):
            title = _strip_pointer_suffix(m.group(1))
            if title:
                texts.append(title)
            continue
        m = _MD_HEADING_RE.match(line)
        if m and m.group(1):
            title = _strip_pointer_suffix(m.group(1))
            if title:
                texts.append(title)
    return texts


def _yaml_texts(text: str) -> list[str]:
    paths: list[str] = []
    stack: list[tuple[int, str]] = []
    for line in text.splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        m = _YAML_LIST_RE.match(line)
        indent = None
        if m:
            indent = len(m.group(1)) + 2
            key = m.group(2)
        else:
            m = _YAML_KEY_RE.match(line)
            if not m:
                continue
            indent = len(m.group(1))
            key = m.group(2)
        while stack and stack[-1][0] >= indent:
            stack.pop()
        stack.append((indent, key))
        paths.append(".".join(k for _, k in stack))
    return paths


def _txt_texts(text: str) -> list[str]:
    texts: list[str] = []
    for line in text.splitlines():
        cleaned = "".join(
            ch for ch in line if ch not in "│├└┌┐┘┤┬┴┼─━►▼◄▲"
        )
        cleaned = " ".join(cleaned.split())
        if len(cleaned) >= 2:
            texts.append(cleaned)
        for bracket in _TXT_BRACKET_RE.findall(line):
            inner = " ".join(bracket.split())
            if len(inner) >= 2:
                texts.append(inner)
        for token in line.split():
            token = token.strip(_TXT_TOKEN_STRIP)
            if len(token) >= 2 and any(ch.isalnum() for ch in token):
                texts.append(token)
    return texts


_RS_DECL_RE = re.compile(
    r"\b(?:struct|enum|trait|union|fn|const|static|type|mod)\s+([A-Za-z_][A-Za-z0-9_]*)"
)
_RS_MACRO_RE = re.compile(r"\bmacro_rules!\s+([A-Za-z_][A-Za-z0-9_]*)")
_RS_VARIANT_RE = re.compile(r"^\s*([A-Z][A-Za-z0-9_]*)\s*(?:\([^)]*\)\s*)?(?:,|=)")
_RS_USE_RE = re.compile(r"^\s*use\s+([^;]+);", re.MULTILINE)
_RS_IMPL_RE = re.compile(
    r"\bimpl\s+(?:<[^>]*>\s+)?([A-Za-z_][A-Za-z0-9_]*)\s+for\s+([A-Za-z_][A-Za-z0-9_]*)"
)
_RS_IMPL_SELF_RE = re.compile(
    r"\bimpl\s+(?:<[^>]*>\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>]*>)?\s*\{"
)
_TS_DECL_RE = re.compile(
    r"\b(?:interface|type|class|enum|function|const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)"
)
_TS_IMPORT_RE = re.compile(r"import\s*\{([^}]*)\}")
_TS_DEFAULT_IMPORT_RE = re.compile(r"^\s*import\s+([A-Za-z_$][A-Za-z0-9_$]*)\s+from\b", re.MULTILINE)
_PY_DEF_RE = re.compile(r"^\s*(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)")
_PY_CLASS_RE = re.compile(r"^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)")
_PY_ASSIGN_RE = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)\s*=")
_PY_FROM_IMPORT_RE = re.compile(r"^\s*from\s+\S+\s+import\s+(.+)$", re.MULTILINE)
_PY_IMPORT_RE = re.compile(r"^\s*import\s+(.+)$", re.MULTILINE)


def _use_names(path_text: str) -> list[str]:
    names = []
    for m in _RS_USE_RE.finditer(path_text):
        body = m.group(1)
        if "{" in body:
            depth, buf = 0, ""
            for ch in body:
                if ch == "{":
                    depth += 1
                elif ch == "}":
                    depth -= 1
                elif depth > 0:
                    buf += ch
            for word in _CODE_WORD_RE.findall(buf):
                if word not in ("self", "super", "crate"):
                    names.append(word)
        else:
            part = body.strip()
            if " as " in part:
                part = part.split(" as ", 1)[1].strip()
            else:
                part = part.split("::")[-1].strip()
            if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", part):
                names.append(part)
    return names


def _import_names(text: str) -> list[str]:
    names = []
    for m in _TS_IMPORT_RE.finditer(text):
        for part in m.group(1).split(","):
            part = part.strip()
            if not part:
                continue
            if " as " in part:
                part = part.split(" as ", 1)[1].strip()
            if re.fullmatch(r"[A-Za-z_$][A-Za-z0-9_$]*", part):
                names.append(part)
    for m in _TS_DEFAULT_IMPORT_RE.finditer(text):
        names.append(m.group(1))
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        m = _PY_FROM_IMPORT_RE.match(lines[i])
        if m:
            inner = m.group(1).strip()
            if inner.startswith("(") and ")" not in inner:
                parts = [inner.lstrip("(")]
                i += 1
                while i < len(lines) and ")" not in lines[i]:
                    parts.append(lines[i].strip())
                    i += 1
                if i < len(lines):
                    parts.append(lines[i].split(")", 1)[0].strip())
                inner = " ".join(parts)
            else:
                inner = inner.strip("()")
            for part in inner.split(","):
                part = part.strip()
                if not part:
                    continue
                if " as " in part:
                    part = part.split(" as ", 1)[1].strip()
                if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", part):
                    names.append(part)
        i += 1
    for m in _PY_IMPORT_RE.finditer(text):
        for part in m.group(1).split(","):
            part = part.strip()
            if " as " in part:
                part = part.split(" as ", 1)[1].strip()
            else:
                part = part.split(".")[0].strip()
            if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", part):
                names.append(part)
    return names


def _code_texts(text: str, suffix: str) -> list[str]:
    names: list[str] = []
    if suffix == ".rs":
        for m in _RS_DECL_RE.finditer(text):
            names.append(m.group(1))
        for m in _RS_MACRO_RE.finditer(text):
            names.append(m.group(1))
        for line in text.splitlines():
            m = _RS_VARIANT_RE.match(line)
            if m:
                names.append(m.group(1))
        for m in _RS_IMPL_RE.finditer(text):
            names.extend([m.group(1), m.group(2)])
        for m in _RS_IMPL_SELF_RE.finditer(text):
            names.append(m.group(1))
        names.extend(_use_names(text))
    elif suffix == ".ts":
        for m in _TS_DECL_RE.finditer(text):
            names.append(m.group(1))
    elif suffix == ".py":
        for line in text.splitlines():
            for rx in (_PY_DEF_RE, _PY_CLASS_RE, _PY_ASSIGN_RE):
                m = rx.match(line)
                if m:
                    names.append(m.group(1))
                    break
    names.extend(_import_names(text))
    return names


def _leaf_texts(path: Path) -> list[str]:
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return []
    suffix = path.suffix.lower()
    if suffix == ".md":
        return _md_texts(text)
    if suffix in (".yaml", ".yml"):
        return _yaml_texts(text)
    if suffix == ".txt":
        return _txt_texts(text)
    if suffix in (".rs", ".ts", ".py"):
        return _code_texts(text, suffix)
    return []


_LEAVES_CACHE: dict[tuple[str, str], tuple[int, int, list[str]]] = {}


def list_leaves(repo_root: str | Path, target_file: str) -> list[str]:
    root = Path(repo_root)
    target = target_file.lstrip("/").replace("\\", "/")
    path = root / target
    key = (str(root), target)
    try:
        stat = path.stat()
        stamp = (stat.st_mtime_ns, stat.st_size)
    except OSError:
        return []
    cached = _LEAVES_CACHE.get(key)
    if cached is not None and cached[0] == stamp[0] and cached[1] == stamp[1]:
        return list(cached[2])
    texts = _leaf_texts(path)
    suffix = path.suffix.lower()
    if suffix in (".yaml", ".yml"):
        leaves = list(dict.fromkeys(texts))
    elif suffix in (".rs", ".ts", ".py"):
        leaves = list(dict.fromkeys(texts))
    else:
        leaves = list(dict.fromkeys(slugify(t) for t in texts if slugify(t)))
    _LEAVES_CACHE[key] = (stamp[0], stamp[1], list(leaves))
    return leaves


def _issue(ref: Ref, reason: str) -> Issue:
    return Issue(
        file=ref.file,
        line=ref.line_no,
        raw=ref.raw,
        targetFile=ref.target,
        targetSlug=ref.slug,
        reason=reason,
    )


def resolve_ref(repo_root: str | Path, ref: Ref) -> Issue | None:
    root = Path(repo_root)
    target = ref.target.lstrip("/").replace("\\", "/")
    path = root / target
    try:
        inside = path.resolve().is_relative_to(root.resolve())
    except (OSError, ValueError):
        inside = False
    if not inside or not path.is_file():
        return _issue(ref, "FileNotFound")
    if ref.slug is None:
        return _issue(ref, "AnchorMissing")
    want = _norm(ref.slug)
    leaves = list_leaves(root, target)
    if any(_norm(leaf) == want for leaf in leaves):
        return None
    if path.suffix.lower() in (".rs", ".ts", ".py"):
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            text = ""
        if any(_norm(word) == want for word in _CODE_WORD_RE.findall(text)):
            return None
    return _issue(ref, "AnchorNotFound")


def _glob_match(rel_posix: str, pattern: str) -> bool:
    path = PurePath(rel_posix)
    if path.match(pattern):
        return True
    if pattern.startswith("**/") and path.match(pattern[3:]):
        return True
    return fnmatch.fnmatchcase(rel_posix, pattern)


def _in_scope(rel_posix: str, config: Config) -> bool:
    if not any(_glob_match(rel_posix, pat) for pat in config.include):
        return False
    return not any(_glob_match(rel_posix, pat) for pat in config.exclude)


def scan_repo(repo_root: str | Path, config: Config) -> list[Issue]:
    root = Path(repo_root)
    if not config.enabled:
        return []
    issues: list[Issue] = []
    for root_entry in config.roots:
        entry = root / root_entry
        files: list[Path] = []
        if entry.is_file():
            files.append(entry)
        elif entry.is_dir():
            for dirpath, dirnames, filenames in os.walk(entry):
                dirnames[:] = sorted(d for d in dirnames if d not in _PRUNE_DIRS)
                for name in sorted(filenames):
                    files.append(Path(dirpath) / name)
        for path in files:
            try:
                rel = path.resolve().relative_to(root.resolve()).as_posix()
            except ValueError:
                continue
            if not _in_scope(rel, config):
                continue
            try:
                text = path.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            for line_no, line in enumerate(text.splitlines(), 1):
                for ref in extract_refs(line):
                    ref.file = rel
                    ref.line_no = line_no
                    issue = resolve_ref(root, ref)
                    if issue is not None:
                        issues.append(issue)
    issues.sort(key=lambda i: (i.file, i.line, i.raw))
    return issues
