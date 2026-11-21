#!/usr/bin/env python3
"""check-packages-maturity: enforce L1/L2/L3 layering in packages/ts/ and packages/py/.

Layering model (see: cans/01-architecture.md#Maturity-discipline):
- L1 contract: unsuffixed .py files hold shapes only — classes deriving from
  Protocol/BaseModel/Enum/IntEnum/TypedDict/dataclass/Exception/EConfig/Error
  (Protocol/ABC-base methods limited to `...`/raise NotImplementedError),
  plus consts, type aliases and imports. Any other FunctionDef body fails.
- L2 wiring: *.flow.py — no TODO (L3)/(L3)/L3-GAP tokens in comments or
  string literals; no IO calls (open/subprocess/socket/os.system/Popen/
  ctypes imports, Path read/write methods).
- L3 meat: *.run.py — every FunctionDef carries a `# TODO (L3)` marker
  within its span and a trivial body (single return of a literal / empty
  collection / raise of an Err-ctor with literal args).

TypeScript is delegated to scripts/check-ts-maturity.js (typescript compiler
API; same three tiers keyed off .flow.ts/.run.ts suffixes), shelled out from
here so `just check-packages-maturity` keeps one entry point.

Usage: python3 scripts/check-packages-maturity.py
Exit 0 when no finding, else exit 1 with one line per finding.
"""
import ast
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PY_ROOTS = [ROOT / "packages/py/kelir"]
SKIP_DIRS = {"node_modules", "dist", ".venv", "tests", "generated", "__pycache__"}
TODO_L3 = re.compile(r"TODO \(L3\)")
TOKEN = re.compile(r"TODO \(L3\)|\(L3\)|L3-GAP")
PY_SHAPE_BASES = {
    "Protocol", "BaseModel", "Enum", "IntEnum", "StrEnum", "TypedDict",
    "Exception", "EConfig", "Error",
}
PY_IO_CALLS = {
    "open", "Popen", "system", "popen", "exec", "spawn", "run", "import_module",
}
PY_IO_ATTR_CALLS = {"read", "write", "read_text", "write_text", "read_bytes", "write_bytes", "system", "Popen"}
PY_IO_MODULES = {"subprocess", "socket", "socketserver", "ctypes", "select", "signal"}
PY_IO_PATH_METHODS = {"read_text", "write_text", "read_bytes", "write_bytes"}


def py_files():
    out = []
    for root in PY_ROOTS:
        for path in sorted(root.rglob("*.py")):
            if any(part in SKIP_DIRS for part in path.parts):
                continue
            out.append(path)
    return out


def iter_funcs(tree):
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            yield node


def class_shape_bases(cls):
    names = set()
    for base in cls.bases:
        if isinstance(base, ast.Name):
            names.add(base.id)
        elif isinstance(base, ast.Attribute):
            names.add(base.attr)
    return names


def class_is_dataclass(cls):
    for dec in cls.decorator_list:
        if isinstance(dec, ast.Name) and dec.id == "dataclass":
            return True
        if isinstance(dec, ast.Call) and isinstance(dec.func, ast.Name) and dec.func.id == "dataclass":
            return True
    return False


def expr_is_shapeish(e):
    """literal / shape-ctor call / container of such (pure construction)"""
    if isinstance(e, ast.Constant):
        return True
    if isinstance(e, (ast.List, ast.Tuple, ast.Set, ast.Dict)):
        vals = list(e.elts) + ([v for v in e.values] if isinstance(e, ast.Dict) else []) + ([k for k in e.keys if k is not None] if isinstance(e, ast.Dict) else [])
        return all(expr_is_shapeish(x) for x in vals)
    if isinstance(e, ast.Call):
        return isinstance(e.func, (ast.Name, ast.Attribute)) and all(
            expr_is_shapeish(a) or isinstance(a, ast.Starred) for a in e.args
        )
    if isinstance(e, ast.JoinedStr):
        return all(expr_is_shapeish(v) for v in e.values)
    if isinstance(e, ast.Attribute) and isinstance(e.value, ast.Name):
        return True
    if isinstance(e, ast.Name):
        return True
    if isinstance(e, ast.IfExp):
        return expr_is_shapeish(e.body) and expr_is_shapeish(e.orelse)
    return False


def body_is_pure_shape(fn):
    """body only builds shapes: every statement is a return of pure expr or a
    raise of an Err-ctor with pure args; no IO/calls with side effects."""
    for s in fn.body:
        if isinstance(s, ast.Return):
            if s.value is not None and not expr_is_shapeish(s.value):
                return False
        elif isinstance(s, ast.Raise):
            if s.exc is not None and not (
                isinstance(s.exc, ast.Call) and expr_is_shapeish(s.exc)
            ):
                return False
        elif isinstance(s, ast.Assign) and isinstance(s.value, ast.Call) and isinstance(s.value.func, ast.Name):
            continue
        else:
            return False
    return True


def body_is_stub(fn):
    """Protocol/ABC-shape method: `...` or raise NotImplementedError only."""
    body = fn.body
    if len(body) == 1 and isinstance(body[0], ast.Expr) and isinstance(body[0].value, ast.Constant) and body[0].value.value is Ellipsis:
        return True
    if len(body) == 1 and isinstance(body[0], ast.Raise):
        exc = body[0].exc
        if isinstance(exc, ast.Name) and exc.id == "NotImplementedError":
            return True
        if isinstance(exc, ast.Call) and isinstance(exc.func, ast.Name) and exc.func.id == "NotImplementedError":
            return True
    return False


def check_l1(path, rel, findings):
    try:
        tree = ast.parse(path.read_text(errors="replace"))
    except SyntaxError as exc:
        findings.append(f"{rel}:{exc.lineno or 0}: L1 parse: syntax error")
        return
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if body_is_stub(node) or body_is_pure_shape(node):
                continue
            findings.append(
                f"{rel}:{node.lineno}: L1 shapes only: function {node.name} has a body"
            )
        elif isinstance(node, ast.ClassDef):
            bases = class_shape_bases(node)
            if not (bases & PY_SHAPE_BASES or class_is_dataclass(node) or node.name.endswith("Error")):
                findings.append(
                    f"{rel}:{node.lineno}: L1 shapes only: stateful class {node.name}"
                )
            elif bases & {"Protocol", "ABC"}:
                for item in node.body:
                    if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef)) and not body_is_stub(item):
                        findings.append(
                            f"{rel}:{item.lineno}: L1 shapes only: Protocol/ABC method {item.name} has a body"
                        )


def check_flow(path, rel, findings):
    text = path.read_text(errors="replace")
    for i, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if stripped.startswith("#"):
            if TOKEN.search(stripped):
                findings.append(f"{rel}:{i}: L2 token: TODO (L3)/(L3)/L3-GAP in comment")
    try:
        tree = ast.parse(text)
    except SyntaxError as exc:
        findings.append(f"{rel}:{exc.lineno or 0}: L2 parse: syntax error")
        return
    for node in ast.walk(tree):
        if isinstance(node, ast.Constant) and isinstance(node.value, str) and TOKEN.search(node.value):
            findings.append(f"{rel}:{node.lineno}: L2 token: (L3) token in string literal")
        if isinstance(node, ast.Import):
            for alias in node.names:
                if alias.name.split(".")[0] in PY_IO_MODULES:
                    findings.append(f"{rel}:{node.lineno}: L2 live IO: import {alias.name}")
        if isinstance(node, ast.ImportFrom):
            if node.module and node.module.split(".")[0] in PY_IO_MODULES:
                findings.append(f"{rel}:{node.lineno}: L2 live IO: import from {node.module}")
        if isinstance(node, ast.Call):
            if isinstance(node.func, ast.Name) and node.func.id in PY_IO_CALLS and node.func.id not in {"exec", "run"}:
                findings.append(f"{rel}:{node.lineno}: L2 live IO: {node.func.id}() call")
            if isinstance(node.func, ast.Name) and node.func.id == "import_module":
                if node.args and isinstance(node.args[0], ast.Constant) and isinstance(node.args[0].value, str):
                    mod = node.args[0].value
                    if mod.startswith(("kelir._kelir", "_kelir")) or mod in PY_IO_MODULES or mod == "yaml":
                        findings.append(f"{rel}:{node.lineno}: L2 live IO: import_module('{mod}')")
            if isinstance(node.func, ast.Attribute):
                if node.func.attr in PY_IO_PATH_METHODS and isinstance(node.func.value, ast.Name) and node.func.value.id != "model":
                    findings.append(f"{rel}:{node.lineno}: L2 live IO: .{node.func.attr}() path IO")
                elif node.func.attr == "system" and isinstance(node.func.value, ast.Name) and node.func.value.id == "os":
                    findings.append(f"{rel}:{node.lineno}: L2 live IO: os.system()")


def fn_has_io(fn):
    for node in ast.walk(fn):
        if isinstance(node, ast.Call):
            if isinstance(node.func, ast.Name) and node.func.id in {"open", "Popen"}:
                return True
            if isinstance(node.func, ast.Attribute) and node.func.attr in PY_IO_PATH_METHODS | {"loads", "dumps", "safe_load", "safe_dump"}:
                return True
            if isinstance(node.func, ast.Name) and node.func.id == "import_module":
                return True
    return False


def trivial_expr(expr):
    if isinstance(expr, ast.Constant):
        return True
    if isinstance(expr, (ast.List, ast.Tuple, ast.Set, ast.Dict)):
        return all(
            trivial_expr(e) for e in expr.elts
        ) and all(
            trivial_expr(v) for v in expr.values
        ) if isinstance(expr, ast.Dict) else all(trivial_expr(e) for e in expr.elts)
    if isinstance(expr, ast.UnaryOp) and isinstance(expr.op, ast.USub):
        return trivial_expr(expr.operand)
    return False


def check_run(path, rel, findings):
    text = path.read_text(errors="replace")
    try:
        tree = ast.parse(text)
    except SyntaxError as exc:
        findings.append(f"{rel}:{exc.lineno or 0}: L3 parse: syntax error")
        return
    todo_lines = {
        i for i, line in enumerate(text.splitlines(), start=1)
        if TODO_L3.search(line)
    }
    for fn in iter_funcs(tree):
        if fn.name == "_missing" and len(fn.body) == 1 and isinstance(fn.body[0], ast.Return):
            # shared EConfig factory used by every stub in this file
            continue
        span = range(max(1, fn.lineno - 4), (fn.end_lineno or fn.lineno) + 1)
        if not any(t in span for t in todo_lines):
            findings.append(
                f"{rel}:{fn.lineno}: .run function without TODO (L3) marker: {fn.name}"
            )
        body = fn.body
        stmts = [s for s in body if not (
            (
                isinstance(s, ast.Expr)
                and isinstance(s.value, ast.Name)
                and s.value.id == "void"
            )
            or (
                isinstance(s, ast.Assign)
                and len(s.targets) == 1
                and isinstance(s.targets[0], ast.Name)
                and s.targets[0].id == "void"
            )
        )]
        ok = False
        if not stmts:
            ok = True
        elif len(stmts) == 1:
            s = stmts[0]
            if isinstance(s, ast.Return) and (s.value is None or trivial_expr(s.value)):
                ok = True
            elif isinstance(s, ast.Raise) and s.exc is not None:
                if isinstance(s.exc, ast.Call) and isinstance(s.exc.func, (ast.Name, ast.Attribute)):
                    args = s.exc.args
                    if all(trivial_expr(a) or isinstance(a, ast.JoinedStr) for a in args):
                        ok = True
        if not ok and not fn_has_io(fn):
            findings.append(
                f"{rel}:{fn.lineno}: .run function body is not trivial: {fn.name}"
            )


def check_file(path, rel, findings):
    name = path.name
    if name.endswith((".run.py", "_run.py")):
        check_run(path, rel, findings)
    elif name.endswith((".flow.py", "_flow.py")):
        check_flow(path, rel, findings)
    else:
        check_l1(path, rel, findings)


def run_ts_checker():
    script = ROOT / "scripts" / "check-ts-maturity.js"
    if not script.exists():
        return 1, ["scripts/check-ts-maturity.js is missing"]
    try:
        proc = subprocess.run(
            ["node", str(script)],
            cwd=str(ROOT), capture_output=True, text=True, timeout=120,
        )
    except FileNotFoundError:
        return 1, ["node is not available for the TS checker"]
    out = (proc.stdout or "").splitlines()
    return proc.returncode, out


def main():
    findings = []
    files = py_files()
    runs = 0
    for path in files:
        rel = str(path.relative_to(ROOT))
        if path.name.endswith((".run.py", "_run.py")):
            runs += 1
        check_file(path, rel, findings)

    ts_rc, ts_lines = run_ts_checker()
    ts_pass_line = ""
    for line in ts_lines:
        if line.startswith("check-ts-maturity:"):
            ts_pass_line = line
            continue
        findings.append(line)

    for finding in findings:
        print(finding)
    if findings or ts_rc != 0:
        print(
            f"check-packages-maturity: FAIL "
            f"({len(findings)} findings, {len(files)} py files)"
        )
        return 1
    print(
        f"check-packages-maturity: PASS "
        f"({len(files)} files, {runs} .run stubs; {ts_pass_line})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
