#!/usr/bin/env python3
"""check-packages-maturity: enforce L1/L2/L3 layering in packages/ts/ and packages/py/.

Layering model (see: cans/01-architecture.md#Maturity-discipline):
- L1 contract: unsuffixed files holding only types, interfaces, error
  shapes, and pure functions. CONTRACT below names them; a contract file
  must not gain a stateful class or any live IO.
- L2 wiring: every other unsuffixed file (builders, registries, dispatch,
  session lifecycle plumbing). No live IO.
- L3 meat: .run.ts / .run.py files hold only signatures plus a TODO (L3)
  marker, returning a typed error or a zero value. No live IO except
  inside a function guarded by a TODO (L3) marker on the same function.

Live IO is fs/net/child_process/subprocess/socket reads and writes, the
native addon boot, and sandbox execution. Identifier false positives are
excluded by shape, not by name: .spawn/.exec delegations to a native
handle, on_socket_connect / on_fs_mkdir ring names, and relaySocketPath
are not IO; readFile/writeFile declarations in *.d.ts files are not IO.

Pinned exceptions, named here rather than hidden:
- check-builder-parity.sh reads builder.lifecycle.ts, hook.registry.ts,
  kelir_builder.py, and kelir_builder_scope.py by content, so those four
  stay mixed contract-plus-wiring and are exempt from the contract rule
  only (never from the live-IO ban).
- generated/ is codegen output, owned by the generator, and is skipped.
- *.d.ts files declare node signatures; they execute nothing.

Usage: python3 scripts/check-packages-maturity.py
Exit 0 when no finding, else exit 1 with one line per finding.
"""
import ast
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TS_ROOTS = [ROOT / "packages/ts/core/src", ROOT / "packages/ts/e2b-compat/src"]
PY_ROOTS = [ROOT / "packages/py/kelir"]
SKIP_DIRS = {"node_modules", "dist", ".venv", "tests", "generated"}

CONTRACT_TS = {
    "packages/ts/core/src/builder.config.ts",
    "packages/ts/core/src/builder.types.ts",
    "packages/ts/core/src/context.lifecycle.types.ts",
    "packages/ts/core/src/errors.sdk.ts",
    "packages/ts/core/src/fusion.plugin.ts",
    "packages/ts/core/src/profile.schema.ts",
    "packages/ts/core/src/schema.config.ts",
    "packages/ts/core/src/session.lifecycle.types.ts",
    "packages/ts/core/src/step-eval.protocol.ts",
    "packages/ts/core/src/types.core.ts",
    "packages/ts/core/src/verdict.protocol.ts",
}
CONTRACT_PY = {
    "packages/py/kelir/builder_scopes.py",
    "packages/py/kelir/profile_config.py",
    "packages/py/kelir/sdk_errors.py",
    "packages/py/kelir/storage_planes.py",
}
PINNED_MIXED = {
    "packages/ts/core/src/builder.lifecycle.ts",
    "packages/ts/core/src/hook.registry.ts",
    "packages/py/kelir/kelir_builder.py",
    "packages/py/kelir/kelir_builder_scope.py",
}

# see: cans/02-plugin-model.md#Unified-builder (from_config hydrates declarative profiles directly)
# see: cans/plugins/profile.md (export_spec emits active plugin topologies to TOML or YAML)
# see: cans/01-architecture.md (named exemption: host-side profile file IO)
# The named exemption covers the shared loader and the writers that implement it:
# _parse_profile is the read behind KelirBuilder::from_config; export_spec is the write.
PY_IO_EXEMPT = {
    ("packages/py/kelir/lifecycle_sandbox.py", "_parse_profile"),
    ("packages/py/kelir/lifecycle_sandbox.py", "export_spec"),
    ("packages/py/kelir/lifecycle_session.py", "export_spec"),
}
TS_IO = [
    (r"""\b(?:readFile|writeFile|appendFile|createReadStream|createWriteStream|readdir|mkdir|rmdir|unlink|stat|open)\s*\(""", "fs call"),
    (r"""\b(?:spawn|execFile|execSync|spawnSync|fork)\s*\(""", "process spawn/exec call"),
    (r"""\bchild_process\b""", "child_process use"),
    (r"""\b(?:net\.createConnection|net\.createServer|createServer|createSocket|new\s+Socket)\b""", "net call"),
    (r"""(?<![\w$.])import\s*\(""", "dynamic import"),
]
TS_IO_RAW = [
    (r"""from\s+['"](?:node:)?(?:fs|child_process|net)(?:/[^'"]*)?['"]""", "node fs/net/child_process import"),
    (r"""require\s*\(\s*['"](?:node:)?(?:fs|child_process|net)(?:/[^'"]*)?['"]""", "node fs/net/child_process require"),
]
PY_IO = [
    (r"""\bimport\s+(?:subprocess|socket|ctypes)\b""", "subprocess/socket/ctypes import"),
    (r"""\bfrom\s+(?:subprocess|socket)\s+import\b""", "subprocess/socket import"),
    (r"""\b(?:subprocess|socket|socketserver)\.""", "subprocess/socket use"),
    (r"""\bPopen\s*\(""", "subprocess Popen"),
    (r"""\bos\.(?:system|exec\w*|spawn\w*|popen)\s*\(""", "os process call"),
    (r"""\bopen\s*\(""", "open call"),
    (r"""\b(?:read_text|write_text|read_bytes|write_bytes)\b""", "path read/write"),
    (r"""\.(?:read|write)\s*\(""", "file read/write"),
    (r"""import_module\s*\(\s*['"](?:kelir\._kelir|_kelir)['"]""", "native bridge import"),
]
TS_IO_RES = [(re.compile(p), label) for p, label in TS_IO]
TS_IO_RAW_RES = [(re.compile(p), label) for p, label in TS_IO_RAW]
PY_IO_RES = [(re.compile(p), label) for p, label in PY_IO]
TODO_L3 = re.compile(r"TODO \(L3\)")
TS_CLASS = re.compile(r"^\s*(?:export\s+)?(?:abstract\s+)?class\s+(\w+)(?:\s+extends\s+([\w.]+))?")
TS_DEF_LINE = re.compile(r"^\s*(?:(?:public|private|protected|static|async)\s+)*(?:spawn|exec|fork)\s*\(")
TS_DELEGATE = re.compile(r"\.(?:spawn|exec|fork)\s*\(")
PY_CLASS = re.compile(r"^class\s+(\w+)\s*(?:\(([^)]*)\))?\s*:")
PY_SHAPE_BASES = {"Protocol", "BaseModel", "Enum", "TypedDict", "Exception", "EConfig", "Error"}
PY_DEF_LINE = re.compile(r"^\s*(?:async\s+)?def\s+(?:spawn|exec|open)\s*\(")
PY_DELEGATE = re.compile(r"\.(?:spawn|exec)\s*\(")


def strip_ts(text):
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    lines = []
    for line in text.splitlines():
        line = re.sub(r"//.*", "", line)
        line = re.sub(r"""'(?:\\.|[^'\\])*'""", "''", line)
        line = re.sub(r'''"(?:\\.|[^"\\])*"''', '""', line)
        line = re.sub(r"`(?:\\.|[^`\\])*`", "``", line)
        lines.append(line)
    return lines


def strip_py(text):
    lines = []
    for line in text.splitlines():
        line = re.sub(r"#.*", "", line)
        line = re.sub(r"""'(?:\\.|[^'\\])*'""", "''", line)
        line = re.sub(r'''"(?:\\.|[^"\\])*"''', '""', line)
        lines.append(line)
    return lines


def ts_functions(lines):
    funcs = []
    depth = 0
    stack = []
    start_re = re.compile(
        r"(?:(?:export|declare)\s+)?(?:async\s+)?function\s+\w+"
        r"|^\s*(?:(?:public|private|protected|static|async|readonly)\s+)*\w+\s*\([^;{}]*\)\s*(?::\s*[^{]+)?\{"
        r"|=>\s*\{"
    )
    for i, line in enumerate(lines, start=1):
        if start_re.search(line) and "{" in line:
            stack.append([i, depth + line.count("{") - line.count("}")])
        depth += line.count("{") - line.count("}")
        while stack and depth < stack[-1][1]:
            start, _ = stack.pop()
            funcs.append((start, i))
    for start, _ in stack:
        funcs.append((start, len(lines)))
    return funcs


def py_named_functions(path):
    try:
        tree = ast.parse(path.read_text(errors="replace"))
    except SyntaxError:
        return []
    funcs = []
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            funcs.append((node.name, node.lineno, node.end_lineno or node.lineno))
    return funcs


def py_functions(path, lines):
    try:
        tree = ast.parse(path.read_text(errors="replace"))
    except SyntaxError:
        return []
    funcs = []
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            funcs.append((node.lineno, node.end_lineno or node.lineno))
    return funcs


def todo_lines(lines):
    return {i for i, line in enumerate(lines, start=1) if TODO_L3.search(line)}


def guarded(funcs, todos, lineno):
    for start, end in funcs:
        if start <= lineno <= end and any(start - 1 <= t <= end for t in todos):
            return True
    return False


def check_file(path, rel, findings):
    text = path.read_text(errors="replace")
    is_ts = path.suffix == ".ts"
    is_run = ".run." in path.name
    lines = strip_ts(text) if is_ts else strip_py(text)
    raw_lines = text.splitlines()
    funcs = ts_functions(lines) if is_ts else py_functions(path, lines)
    named_funcs = [] if is_ts else py_named_functions(path)
    todos = {i for i, line in enumerate(raw_lines, start=1) if TODO_L3.search(line)}
    patterns = TS_IO_RES if is_ts else PY_IO_RES
    raw_patterns = TS_IO_RAW_RES if is_ts else []
    for i, line in enumerate(lines, start=1):
        for regex, label in patterns:
            for match in regex.finditer(line):
                if is_ts and (TS_DEF_LINE.match(line) or TS_DELEGATE.search(line)):
                    continue
                if not is_ts and (PY_DEF_LINE.match(line) or PY_DELEGATE.search(line)):
                    continue
                if not is_ts:
                    fname = next(
                        (n for n, s, e in named_funcs if s <= i <= e), None
                    )
                    if fname and (rel, fname) in PY_IO_EXEMPT:
                        continue
                if not is_run or not guarded(funcs, todos, i):
                    findings.append(f"{rel}:{i}: live IO ({label}): {raw_lines[i - 1].strip()[:100]}")
        for regex, label in raw_patterns:
            if regex.search(raw_lines[i - 1]):
                if not is_run or not guarded(funcs, todos, i):
                    findings.append(f"{rel}:{i}: live IO ({label}): {raw_lines[i - 1].strip()[:100]}")
    if is_run:
        for start, end in funcs:
            if not any(start - 1 <= t <= end for t in todos):
                findings.append(f"{rel}:{start}: .run function without TODO (L3) marker")
    if rel in CONTRACT_TS or rel in CONTRACT_PY:
        for i, line in enumerate(raw_lines, start=1):
            if is_ts:
                m = TS_CLASS.match(line)
                if m and not (m.group(1).endswith("Error") or (m.group(2) or "").split(".")[-1] in ("EConfig", "Error")):
                    findings.append(f"{rel}:{i}: contract file defines stateful class {m.group(1)}")
            else:
                m = PY_CLASS.match(line)
                if m:
                    bases = {b.strip().split(".")[-1] for b in (m.group(2) or "").split(",") if b.strip()}
                    decorated = i > 1 and "dataclass" in raw_lines[i - 2]
                    if not (bases & PY_SHAPE_BASES or decorated or m.group(1).endswith("Error")):
                        findings.append(f"{rel}:{i}: contract file defines stateful class {m.group(1)}")


def collect():
    files = []
    for root in TS_ROOTS + PY_ROOTS:
        for path in sorted(root.rglob("*")):
            if not path.is_file() or path.suffix not in (".ts", ".py"):
                continue
            if path.suffix == ".ts" and path.name.endswith(".d.ts"):
                continue
            if any(part in SKIP_DIRS for part in path.parts):
                continue
            files.append(path)
    return files


def main():
    findings = []
    files = collect()
    runs = 0
    for path in files:
        rel = str(path.relative_to(ROOT))
        if ".run." in path.name:
            runs += 1
        check_file(path, rel, findings)
    for finding in findings:
        print(finding)
    if findings:
        print(f"check-packages-maturity: FAIL ({len(findings)} findings, {len(files)} files)")
        return 1
    contracts = len(CONTRACT_TS) + len(CONTRACT_PY)
    print(f"check-packages-maturity: PASS ({len(files)} files, {contracts} contract, {runs} .run stubs)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
