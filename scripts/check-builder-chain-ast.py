#!/usr/bin/env python3
"""check-builder-chain-ast: unbroken-builder-chain scanner (L1/L2 skeleton).

Parses cans/artifacts/examples/ non-recursively, so deferred/ is excluded
by design, and fails on any break of the single-expression typestate chain
(see: fluent-chain.work-order.md Part 1 section 4, Part 3 section 3):

Python (stdlib ast): a KelirBuilder binding reassigned or continued after
construction, a verb call on a bound builder as its own statement, a
deleted flat verb outside a scope lambda, a BasePlugin subclass or import,
a kelir.plugins.plugin_ alias import.

Rust/TypeScript (regex over comment- and string-stripped text): a mutable
builder binding, a lifecycle_mut call, a ? on a chain verb other than
seal, a deleted flat verb outside a scope closure, a @ts-expect-error
directive in a .ts example.

Scope rule: a ring verb is legal only inside its scope closure. The
scanner tracks scope regions (Rust .fs(|f| ...) ... .deny(|d| ...),
TypeScript .fs((f) => ...) ... .deny((d) => ...)) and only flags a
deleted verb outside every region. group() opens no region: a verb
directly inside group() with no inner scope is still a flat call.

Deliberate non-findings, per the work order: let mut session =
...seal()?.boot() binds the post-boot session, not a builder; .await?
is post-boot async suspension (Chain-decisions (h) puts post-boot
operations out of scope); KelirBuilder::from_config(...)? is the
documented profile entry form. The deleted-verb sets and the
scope-entry lists are derived from scripts/builder-parity-verbs.yaml
at runtime (scoped placement); there is no second list to drift.
The yaml is the only verb list: if it cannot be read, or parses to
no verbs, the scanner fails closed instead of scanning vacuous.

Usage: python3 scripts/check-builder-chain-ast.py [examples-dir]
Exit 0 when no finding, else exit 1 with one line per finding.
"""
import ast
import bisect
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# see: scripts/builder-parity-verbs.yaml
VERBS_YAML = ROOT / "scripts" / "builder-parity-verbs.yaml"


def snake_to_camel(name):
    parts = name.split("_")
    return parts[0] + "".join(p[:1].upper() + p[1:] for p in parts[1:])


def ts_name(name):
    if name == "require":
        return "requires"
    return snake_to_camel(name)


def load_verbs():
    """(deleted_rs, deleted_ts, deleted_py, scope_rs, scope_ts) from the yaml."""
    if not VERBS_YAML.is_file():
        raise ValueError(f"missing input file: {VERBS_YAML}")
    verbs, cur, section = {}, None, None
    for line in VERBS_YAML.read_text().splitlines():
        if line == "verbs:":
            section = "verbs"
            continue
        if line == "behavior:":
            section = "behavior"
            continue
        if re.match(r"^[a-z_]+:", line):
            section = None
        if section != "verbs":
            continue
        m = re.match(r"^  ([A-Za-z_][A-Za-z0-9_]*):\s*$", line)
        if m:
            cur = m.group(1)
            verbs[cur] = {}
            continue
        m = re.match(r"^    (placement|scope):\s*(.*)$", line)
        if m and cur:
            verbs[cur][m.group(1)] = m.group(2).strip()
    if not verbs:
        raise ValueError(f"no verbs parsed from {VERBS_YAML}")
    scoped = [n for n, f in verbs.items() if f.get("placement") == "scoped"]
    # params is a scope entry, not a deleted verb: the .params check below
    # tells the closure form from the deleted direct-model form, and this
    # set would flag every legitimate .params(|p| ...) entry. fork_with
    # keeps a legitimate flat plugin-only form on the builder in all
    # three SDKs, so a flat call is not a deleted verb either.
    # register_veto is also a LifecycleCtx method (policy-matrix calls
    # ctx.register_veto in its on_vm_init handler), and this regex
    # scanner cannot tell receivers apart; the parity gate still checks
    # its LifecycleScope presence, which is the real gate for it.
    excluded = {"params", "fork_with", "register_veto"}
    deleted = [n for n in scoped if n not in excluded]
    if not deleted:
        raise ValueError(f"no scoped verbs parsed from {VERBS_YAML}")
    flat_scopes = [(n, f["scope"]) for n, f in verbs.items() if f.get("placement") == "flat" and f.get("scope")]
    scope_rs = sorted({"trap_scope" if n == "trap" else s for n, s in flat_scopes} | {"params"})
    scope_ts = sorted({s for _, s in flat_scopes} | {"params"})
    return (deleted, [ts_name(n) for n in deleted], deleted, scope_rs, scope_ts)


def line_of(starts, offset):
    return bisect.bisect_right(starts, offset)


def strip_rs_ts(text):
    """Blank comments and string literals, keeping offsets and newlines."""
    out = list(text)
    i = 0
    n = len(text)
    while i < n:
        c = text[i]
        two = text[i:i + 2]
        if two == "//":
            j = text.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
        elif two == "/*":
            j = text.find("*/", i + 2)
            j = n if j < 0 else j + 2
            for k in range(i, j):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        elif c == '"' or c == "`":
            quote = c
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == quote:
                    j += 1
                    break
                if quote != "`" and text[j] == "\n":
                    break
                j += 1
            for k in range(i, min(j, n)):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        elif c == "'":
            m = re.match(r"'(\\.|[^\\'])'", text[i:i + 4])
            if m:
                for k in range(i, i + len(m.group(0))):
                    out[k] = " "
                i += len(m.group(0))
            else:
                i += 1
        else:
            i += 1
    return "".join(out)


def matching_paren(text, open_idx):
    depth = 0
    for i in range(open_idx, len(text)):
        if text[i] == "(":
            depth += 1
        elif text[i] == ")":
            depth -= 1
            if depth == 0:
                return i
    return None


def scope_regions(cleaned, entries, closer):
    """(open, close) offset spans of scope closures in cleaned text."""
    regions = []
    pat = re.compile(r"\.(" + "|".join(entries) + r")\s*\(")
    for m in pat.finditer(cleaned):
        open_idx = m.end() - 1
        probe = cleaned[open_idx + 1:open_idx + 1 + closer[1]]
        if closer[0] == "pipe":
            if not re.match(r"\s*\|", probe):
                continue
        else:
            if "=>" not in probe:
                continue
        close_idx = matching_paren(cleaned, open_idx)
        if close_idx is not None:
            regions.append((open_idx, close_idx))
    return regions


def in_region(regions, offset):
    return any(start < offset < end for start, end in regions)


def check_rs_ts(path, text, findings, deleted_rs, deleted_ts, scope_rs, scope_ts):
    starts = [i for i, c in enumerate(text) if c == "\n"]
    starts = [0] + [s + 1 for s in starts]
    cleaned = strip_rs_ts(text)
    is_rs = path.suffix == ".rs"
    entries = scope_rs if is_rs else scope_ts
    closer = ("pipe", 80) if is_rs else ("arrow", 120)
    regions = scope_regions(cleaned, entries, closer)

    def add(offset, msg):
        findings.append((str(path), line_of(starts, offset), msg))

    if is_rs:
        for m in re.finditer(r"let\s+mut\s+([A-Za-z_][A-Za-z0-9_]*)", cleaned):
            if "builder" in m.group(1).lower():
                add(m.start(), "mutable builder binding: let mut %s" % m.group(1))
        for m in re.finditer(r"(?m)^\s*builder\s*\+?=(?!=)", cleaned):
            add(m.start(), "builder rebinding post-construction: builder =")
        for m in re.finditer(r"lifecycle_mut\s*\(", cleaned):
            add(m.start(), "deleted lifecycle_mut() call")
        for m in re.finditer(r"\.(\w+)\?", cleaned):
            if m.group(1) not in ("seal", "await"):
                add(m.start(), "fallible chain verb: .%s?" % m.group(1))
        deleted = deleted_rs
        for m in re.finditer(r"\.(" + "|".join(deleted) + r")\s*\(", cleaned):
            if not in_region(regions, m.start()):
                add(m.start(), "deleted flat verb outside a scope: .%s()" % m.group(1))
        for m in re.finditer(r"\.params\s*\(", cleaned):
            after = cleaned[m.end():m.end() + 8]
            if not re.match(r"\s*\|", after) and not in_region(regions, m.start()):
                add(m.start(), "deleted direct-model params() outside a scope")
    else:
        for m in re.finditer(r"(?m)^\s*let\s+(\w*[Bb]uilder\w*)\s*=", cleaned):
            add(m.start(), "mutable builder binding: let %s" % m.group(1))
        for m in re.finditer(r"(?m)^\s*builder\s*\+?=(?!=)", cleaned):
            add(m.start(), "builder rebinding post-construction: builder =")
        for m in re.finditer(r"@ts-expect-error", text):
            add(m.start(), "@ts-expect-error in a .ts example")
        for m in re.finditer(r"\bon\s*\(", cleaned):
            add(m.start(), "deleted flat .on() verb")
        deleted = deleted_ts
        for m in re.finditer(r"\.(" + "|".join(deleted) + r")\s*\(", cleaned):
            if not in_region(regions, m.start()):
                add(m.start(), "deleted flat verb outside a scope: .%s()" % m.group(1))
        for m in re.finditer(r"\.params\s*\(", cleaned):
            close_idx = matching_paren(cleaned, m.end() - 1)
            body = "" if close_idx is None else cleaned[m.end():close_idx]
            if "=>" not in body:
                add(m.start(), "deleted direct-model params() outside a scope")


class PyChecker(ast.NodeVisitor):
    def __init__(self, path, tree, deleted):
        self.path = str(path)
        self.findings = []
        self.deleted = set(deleted)
        self.parents = {}
        for node in ast.walk(tree):
            for child in ast.iter_child_nodes(node):
                self.parents[child] = node
        self.builders = set()
        self.tree = tree

    def add(self, node, msg):
        self.findings.append((self.path, getattr(node, "lineno", 1), msg))

    def add_attr(self, call, msg):
        # Point at the attribute name itself: a chained call's lineno is
        # the chain start, while func.end_* is just past the attribute.
        func = call.func
        lineno = getattr(func, "end_lineno", None) or getattr(call, "lineno", 1)
        self.findings.append((self.path, lineno, msg))

    def in_lambda(self, node):
        cur = node
        while cur in self.parents:
            cur = self.parents[cur]
            if isinstance(cur, ast.Lambda):
                return True
        return False

    def chain_root(self, node):
        while isinstance(node, ast.Call):
            func = node.func
            if isinstance(func, ast.Attribute):
                node = func.value
            else:
                return func
        return node

    def is_builder_expr(self, node):
        if isinstance(node, ast.Await):
            node = node.value
        root = self.chain_root(node)
        if isinstance(root, ast.Name):
            if root.id == "KelirBuilder":
                return True
            return root.id in self.builders
        if isinstance(root, ast.Call):
            func = root.func
            if isinstance(func, ast.Name) and func.id == "KelirBuilder":
                return True
            if isinstance(func, ast.Attribute):
                if func.attr in ("plugin", "from_config") and isinstance(func.value, ast.Name) and func.value.id == "KelirBuilder":
                    return True
                if func.attr == "builder" and isinstance(func.value, ast.Name) and func.value.id == "kelir":
                    return True
        return False

    def outer_verb(self, node):
        if isinstance(node, ast.Await):
            node = node.value
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute):
            return node.func.attr
        return None

    def check_assignments(self):
        assigns = []
        for node in ast.walk(self.tree):
            if isinstance(node, (ast.Assign, ast.AnnAssign)):
                assigns.append(node)
        assigns.sort(key=lambda n: (n.lineno, n.col_offset))
        for node in assigns:
            value = node.value
            if value is None:
                continue
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            names = [t.id for t in targets if isinstance(t, ast.Name)]
            if not names:
                continue
            if not self.is_builder_expr(value):
                continue
            root = self.chain_root(value)
            continued = isinstance(root, ast.Name) and root.id in self.builders
            for name in names:
                if name in self.builders:
                    self.add(node, "builder rebinding post-construction: %s =" % name)
                elif continued:
                    self.add(node, "chain continuation rebinding: %s = <bound builder>..." % name)
                if self.outer_verb(value) != "boot":
                    self.builders.add(name)

    def check_statements(self):
        for node in ast.walk(self.tree):
            if not isinstance(node, ast.Expr):
                continue
            value = node.value
            if isinstance(value, ast.Await):
                value = value.value
            if not isinstance(value, ast.Call):
                continue
            func = value.func
            if isinstance(func, ast.Attribute) and isinstance(func.value, ast.Name):
                if func.value.id in self.builders:
                    self.add_attr(value, "statement-level verb call: %s.%s()" % (func.value.id, func.attr))

    def check_deleted_verbs(self):
        deleted = self.deleted
        for node in ast.walk(self.tree):
            if not isinstance(node, ast.Call):
                continue
            func = node.func
            if isinstance(func, ast.Attribute) and func.attr in deleted:
                if not self.in_lambda(node):
                    self.add_attr(node, "deleted flat verb outside a scope: .%s()" % func.attr)

    def check_base_plugin(self):
        seen = set()
        for node in ast.walk(self.tree):
            if isinstance(node, ast.ClassDef):
                for base in node.bases:
                    if isinstance(base, ast.Name) and base.id == "BasePlugin":
                        self.add(node, "BasePlugin subclass: class %s" % node.name)
                        seen.add(node.lineno)
            elif isinstance(node, ast.ImportFrom):
                for alias in node.names:
                    if alias.name == "BasePlugin":
                        self.add(node, "BasePlugin import")
                        seen.add(node.lineno)
            elif isinstance(node, ast.Import):
                for alias in node.names:
                    if alias.name == "BasePlugin" or alias.name.endswith(".BasePlugin"):
                        self.add(node, "BasePlugin import")
                        seen.add(node.lineno)
        for node in ast.walk(self.tree):
            if isinstance(node, ast.Name) and node.id == "BasePlugin" and node.lineno not in seen:
                if isinstance(self.parents.get(node), (ast.ClassDef, ast.ImportFrom, ast.Import)):
                    continue
                if isinstance(node.ctx, ast.Load):
                    self.add(node, "BasePlugin use")
                    seen.add(node.lineno)

    def check_plugin_alias_imports(self):
        for node in ast.walk(self.tree):
            if isinstance(node, ast.ImportFrom):
                mod = node.module or ""
                if mod.startswith("kelir.plugins.plugin_"):
                    self.add(node, "kelir.plugins.plugin_ alias import: %s" % mod)
                elif mod == "kelir.plugins":
                    for alias in node.names:
                        if alias.name.startswith("plugin_"):
                            self.add(node, "kelir.plugins.plugin_ alias import: %s" % alias.name)
            elif isinstance(node, ast.Import):
                for alias in node.names:
                    if alias.name.startswith("kelir.plugins.plugin_"):
                        self.add(node, "kelir.plugins.plugin_ alias import: %s" % alias.name)

    def run(self):
        self.check_assignments()
        self.check_statements()
        self.check_deleted_verbs()
        self.check_base_plugin()
        self.check_plugin_alias_imports()
        return self.findings


def check_py(path, text, findings, deleted):
    try:
        tree = ast.parse(text, filename=str(path))
    except SyntaxError as exc:
        findings.append((str(path), exc.lineno or 1, "unparsable: %s" % exc.msg))
        return
    findings.extend(PyChecker(path, tree, deleted).run())


def main(argv):
    exdir = Path(argv[1]) if len(argv) > 1 else ROOT / "cans" / "artifacts" / "examples"
    if not exdir.is_dir():
        print("FAIL: examples dir missing: %s" % exdir)
        return 1
    try:
        deleted_rs, deleted_ts, deleted_py, scope_rs, scope_ts = load_verbs()
    except ValueError as exc:
        print(f"FAIL: {exc}")
        return 1
    findings = []
    files = sorted(p for p in exdir.iterdir() if p.is_file() and p.suffix in (".rs", ".ts", ".py"))
    for path in files:
        text = path.read_text(encoding="utf-8")
        if path.suffix == ".py":
            check_py(path, text, findings, deleted_py)
        else:
            check_rs_ts(path, text, findings, deleted_rs, deleted_ts, scope_rs, scope_ts)
    findings.sort(key=lambda f: (f[0], f[1], f[2]))
    for path, lineno, msg in findings:
        print("%s:%s: %s" % (path, lineno, msg))
    if findings:
        hit = {f[0] for f in findings}
        print("FAIL: check-builder-chain-ast: %d finding(s) in %d file(s)" % (len(findings), len(hit)))
        return 1
    print("OK: check-builder-chain-ast: 0 findings in %d file(s)" % len(files))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
