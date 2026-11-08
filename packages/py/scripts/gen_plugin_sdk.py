#!/usr/bin/env python3
# see: cans/02-plugin-model.md#Auto-generated-manifest-contract
from __future__ import annotations

import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
PLUGINS_SRC = REPO_ROOT / "crates" / "kelir-plugins"
INTERFACE_SRC = REPO_ROOT / "crates" / "kelir-interface" / "src"
TS_OUT = REPO_ROOT / "packages" / "ts" / "core" / "src" / "plugins.ts"
PY_OUT_DIR = REPO_ROOT / "packages" / "py" / "kelir" / "plugins"

TS_HEADER = "// see: " + "cans/03-sdk-architecture.md#TypeScript-SDK"

# see: cans/03-sdk-architecture.md#Service-lookup-across-languages
SERVICE_LOOKUP_SEE = "// see: " + "cans/03-sdk-architecture.md#Service-lookup-across-languages"
CONTEXT_HANDLES = {
    "chaos": ("chaos", "ChaosHandle"),
    "ledger": ("ledger", "LedgerHandle"),
    "mesh": ("mesh", "MeshHandle"),
}
PY_BARREL_HEADER = "# see: " + "cans/03-sdk-architecture.md#Python-SDK"

LEGACY_ALIASES: dict[str, dict[str, str]] = {
    "chaos": {"NetworkFaultRule": "NetworkChaos", "StorageFaultRule": "FsChaos"},
}
LEGACY_FIELDS: dict[str, dict[str, list[tuple[str, str, str, str]]]] = {
    "db_proxy": {
        "EphemeralCredentials": [
            (
                "resolve",
                "Callable[[], str] | None",
                "None",
                "TODO (L3): resolve closure injection at the host connection boundary per cans/plugins/db-proxy.md.",
            ),
        ],
    },
}

INT_RE = re.compile(r"^(?:0[xX][0-9a-fA-F_]+|-?\d[\d_]*)$")
NUM = r"(?:0[xX][0-9a-fA-F_]+|\d[\d_]*)"
INT_EXPR_RE = re.compile(rf"^\(?-?{NUM}(?:\s*[-+*]\s*\(?-?{NUM}\)?)*\)?$")
FLOAT_RE = re.compile(r"^-?\d+\.\d+$")
STR_LIT_RE = re.compile(r'^"((?:[^"\\]|\\.)*)"(?:\.(?:into|to_string|to_owned|to_lowercase)\(\))?$')
CONST_RE = re.compile(r"^[A-Z][A-Z0-9_]*$")
PATH_LIT_RE = re.compile(r"^(\w+)::(\w+)$")
STRUCT_LIT_RE = re.compile(r"^(\w+)\s*\{(.*)\}$", re.S)
PUB_FIELD_RE = re.compile(r"^pub\s+(?:r#)?([a-z_][a-z0-9_]*)\s*:\s*(.+)$", re.S)
PLAIN_FIELD_RE = re.compile(r"^(?:pub(?:\([^\)]*\))?\s+)?(?:r#)?([a-z_][a-z0-9_]*)\s*:\s*(.+)$", re.S)
IMPL_FIELD_RE = re.compile(r"^([a-z_][a-z0-9_]*)\s*:\s*(.+)$", re.S)
STRUCT_DEF_RE = re.compile(r"pub struct (\w+)")
ENUM_DEF_RE = re.compile(r"pub enum (\w+)")
IMPL_DEFAULT_RE = re.compile(r"impl Default for (\w+)")
IMPL_PLUGIN_RE = re.compile(r"impl KelirPlugin for (\w+)")
PLUGIN_NAME_RE = re.compile(
    r"fn\s+name\s*\(\s*&\s*self\s*\)\s*->\s*&\s*'static\s+str\s*\{\s*\"([^\"]*)\"", re.S
)
CONST_DEF_RE = re.compile(r"(?:pub(?:\([^\)]*\))?\s+)?const\s+([A-Z]\w*)\s*:[^={;]+=\s*([^;]+);")
SEE_RE = re.compile(r"see: (cans/\S+)")
PASCAL_SPLIT_RE = re.compile(r"[A-Z][a-z]*|[A-Z]+|\d+|[a-z]+")


class GenError(Exception):
    pass


def mask_source(text: str) -> str:
    out = list(text)
    i = 0
    n = len(text)
    state = "code"
    while i < n:
        c = text[i]
        nxt = text[i + 1] if i + 1 < n else ""
        if state == "code":
            if c == "/" and nxt == "/":
                out[i] = " "
                if i + 1 < n:
                    out[i + 1] = " "
                state = "line"
                i += 2
                continue
            if c == "/" and nxt == "*":
                out[i] = " "
                if i + 1 < n:
                    out[i + 1] = " "
                state = "block"
                i += 2
                continue
            if c == '"':
                out[i] = " "
                state = "str"
                i += 1
                continue
            i += 1
        elif state == "line":
            if c == "\n":
                state = "code"
            else:
                out[i] = " "
            i += 1
        elif state == "block":
            if c == "*" and nxt == "/":
                out[i] = " "
                if i + 1 < n:
                    out[i + 1] = " "
                state = "code"
                i += 2
                continue
            if c != "\n":
                out[i] = " "
            i += 1
        else:
            if c == "\\":
                out[i] = " "
                if i + 1 < n:
                    out[i + 1] = " "
                i += 2
                continue
            if c == '"':
                out[i] = " "
                state = "code"
            else:
                out[i] = " "
            i += 1
    return "".join(out)


def match_brace(masked: str, open_idx: int) -> int:
    depth = 0
    for i in range(open_idx, len(masked)):
        if masked[i] == "{":
            depth += 1
        elif masked[i] == "}":
            depth -= 1
            if depth == 0:
                return i
    raise GenError(f"unbalanced braces at offset {open_idx}")


def split_top_level(body: str) -> list[str]:
    parts: list[str] = []
    cur: list[str] = []
    depth = 0
    for ch in body:
        if ch in "({<[":
            depth += 1
        elif ch in ")}>]":
            if depth > 0:
                depth -= 1
        if ch == "," and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
    parts.append("".join(cur))
    return [p.strip() for p in parts if p.strip()]


def strip_attr_lines(text: str) -> str:
    return "\n".join(
        line for line in text.splitlines() if not line.strip().startswith("#[") and line.strip()
    )


@dataclass
class RustField:
    name: str
    type_text: str


@dataclass
class RustStruct:
    name: str
    fields: list[RustField]
    order: int


@dataclass
class RustEnumVariant:
    name: str
    fields: list[RustField]
    is_default: bool


@dataclass
class RustEnum:
    name: str
    variants: list[RustEnumVariant]
    order: int


@dataclass
class RustFile:
    structs: dict[str, RustStruct]
    enums: dict[str, RustEnum]
    consts: dict[str, str]
    defaults: dict[str, dict[str, str]]
    plugin_impls: dict[str, str]
    see_refs: list[str]


def find_item_brace(masked: str, keyword_end: int) -> int:
    i = keyword_end
    while i < len(masked):
        c = masked[i]
        if c in " \t\r\n":
            i += 1
            continue
        if c == "{":
            return i
        return -1
    return -1


def parse_fields(body: str, require_pub: bool) -> list[RustField]:
    out: list[RustField] = []
    regex = PUB_FIELD_RE if require_pub else PLAIN_FIELD_RE
    for part in split_top_level(body):
        clean = strip_attr_lines(part).strip()
        m = regex.match(clean)
        if m is None:
            continue
        out.append(RustField(name=m.group(1), type_text=m.group(2).strip().rstrip(",")))
    return out


def parse_rust_file(path: Path, order_start: int) -> tuple[RustFile, int]:
    text = path.read_text(encoding="utf-8")
    masked = mask_source(text)
    result = RustFile(structs={}, enums={}, consts={}, defaults={}, plugin_impls={}, see_refs=[])
    order = order_start
    for m in STRUCT_DEF_RE.finditer(masked):
        brace = find_item_brace(masked, m.end())
        if brace == -1:
            continue
        close = match_brace(masked, brace)
        result.structs[m.group(1)] = RustStruct(
            name=m.group(1),
            fields=parse_fields(text[brace + 1 : close], True),
            order=order,
        )
        order += 1
    for m in ENUM_DEF_RE.finditer(masked):
        brace = find_item_brace(masked, m.end())
        if brace == -1:
            continue
        close = match_brace(masked, brace)
        body = text[brace + 1 : close]
        variants: list[RustEnumVariant] = []
        for part in split_top_level(body):
            clean = strip_attr_lines(part).strip()
            is_default = any(
                ln.strip().startswith("#[") and "default" in ln for ln in part.splitlines()
            )
            vm = re.match(
                r"^(?:pub(?:\([^\)]*\))?\s+)?([A-Z]\w*)\s*(?:\{(.*)\})?\s*,?$", clean, re.S
            )
            if vm is None:
                continue
            variants.append(
                RustEnumVariant(
                    name=vm.group(1),
                    fields=parse_fields(vm.group(2) or "", False),
                    is_default=is_default,
                )
            )
        result.enums[m.group(1)] = RustEnum(name=m.group(1), variants=variants, order=order)
        order += 1
    for m in CONST_DEF_RE.finditer(masked):
        result.consts[m.group(1)] = text[m.start(2) : m.end(2)].strip()
    for m in IMPL_DEFAULT_RE.finditer(masked):
        brace = find_item_brace(masked, m.end())
        if brace == -1:
            continue
        close = match_brace(masked, brace)
        impl_body = masked[brace + 1 : close]
        self_open = -1
        for sm in re.finditer(r"Self\s*\{", impl_body):
            before = impl_body[: sm.start()].rstrip()
            if before.endswith("->"):
                continue
            self_open = brace + 1 + sm.end() - 1
            break
        if self_open == -1:
            continue
        self_close = match_brace(masked, self_open)
        fields: dict[str, str] = {}
        for part in split_top_level(text[self_open + 1 : self_close]):
            fm = IMPL_FIELD_RE.match(part)
            if fm is not None:
                fields[fm.group(1)] = fm.group(2).strip().rstrip(",")
        result.defaults[m.group(1)] = fields
    for m in IMPL_PLUGIN_RE.finditer(masked):
        brace = find_item_brace(masked, m.end())
        if brace == -1:
            continue
        close = match_brace(masked, brace)
        nm = PLUGIN_NAME_RE.search(text[brace + 1 : close])
        if nm is not None:
            result.plugin_impls[m.group(1)] = nm.group(1)
    for m in SEE_RE.finditer(text):
        result.see_refs.append(m.group(1).rstrip("`."))
    return result, order


@dataclass
class Crate:
    dir_name: str
    plugin_name: str
    module_name: str
    structs: dict[str, RustStruct]
    enums: dict[str, RustEnum]
    consts: dict[str, str]
    defaults: dict[str, dict[str, str]]
    plugin_class: str
    plugin_class_name: str
    see_ref: str


def parse_crate(crate_dir: Path, interface_consts: dict[str, str]) -> Crate:
    files = sorted(crate_dir.glob("src/**/*.rs"))
    if not files:
        raise GenError(f"no sources under {crate_dir}")
    plugin_name = crate_dir.name.removeprefix("plugin-")
    structs: dict[str, RustStruct] = {}
    enums: dict[str, RustEnum] = {}
    consts: dict[str, str] = dict(interface_consts)
    defaults: dict[str, dict[str, str]] = {}
    plugin_impls: dict[str, str] = {}
    see_refs: list[str] = []
    order = 0
    for f in files:
        parsed, order = parse_rust_file(f, order)
        structs.update(parsed.structs)
        enums.update(parsed.enums)
        consts.update(parsed.consts)
        defaults.update(parsed.defaults)
        plugin_impls.update(parsed.plugin_impls)
        see_refs.extend(parsed.see_refs)
    if len(plugin_impls) != 1:
        raise GenError(
            f"{crate_dir.name}: expected exactly one KelirPlugin impl, got {sorted(plugin_impls)}"
        )
    plugin_class, plugin_class_name = next(iter(plugin_impls.items()))
    preferred = f"cans/plugins/{plugin_name}.md#Plugin-{plugin_name}"
    if preferred in see_refs:
        see_ref = preferred
    else:
        prefix = f"cans/plugins/{plugin_name}.md#"
        candidates = [r for r in see_refs if r.startswith(prefix)]
        see_ref = candidates[0] if candidates else preferred
    return Crate(
        dir_name=crate_dir.name,
        plugin_name=plugin_name,
        module_name=plugin_name.replace("-", "_"),
        structs=structs,
        enums=enums,
        consts=consts,
        defaults=defaults,
        plugin_class=plugin_class,
        plugin_class_name=plugin_class_name,
        see_ref=see_ref,
    )


def collect_consts(root: Path) -> dict[str, str]:
    consts: dict[str, str] = {}
    for f in sorted(root.glob("**/*.rs")):
        parsed, _ = parse_rust_file(f, 0)
        consts.update(parsed.consts)
    return consts


@dataclass
class TypeRef:
    kind: str
    name: str = ""
    items: list[TypeRef] = field(default_factory=list)
    array_len: int = 0


INT_TYPES = {"u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize"}


def parse_type(text: str) -> TypeRef:
    t = " ".join(text.split()).rstrip(",").rstrip(";")
    if t in ("String", "&str", "str"):
        return TypeRef(kind="string")
    if t in ("PathBuf", "Path", "&Path"):
        return TypeRef(kind="path")
    if t == "bool":
        return TypeRef(kind="bool")
    if t in INT_TYPES:
        return TypeRef(kind="int")
    if t in ("f32", "f64"):
        return TypeRef(kind="float")
    if t.startswith("Option<") and t.endswith(">"):
        return TypeRef(kind="option", items=[parse_type(t[len("Option<") : -1])])
    if t.startswith("Vec<") and t.endswith(">"):
        return TypeRef(kind="vec", items=[parse_type(t[len("Vec<") : -1])])
    if t.startswith("(") and t.endswith(")"):
        inner = t[1:-1].strip()
        if not inner:
            raise GenError(f"empty tuple type: {text}")
        return TypeRef(kind="tuple", items=[parse_type(p) for p in split_top_level(inner)])
    m = re.match(r"^\[u8;\s*(\d+)\]$", t)
    if m is not None:
        return TypeRef(kind="bytes", array_len=int(m.group(1)))
    if re.match(r"^[A-Za-z_]\w*$", t) is not None:
        return TypeRef(kind="named", name=t)
    raise GenError(f"unsupported rust type: {text}")


def named_refs(tr: TypeRef) -> list[str]:
    out: list[str] = []
    if tr.kind == "named":
        out.append(tr.name)
    for item in tr.items:
        out.extend(named_refs(item))
    return out


@dataclass
class ProjStruct:
    rust_name: str
    fields: list[RustField]
    order: int


@dataclass
class Projection:
    crate: Crate
    root: ProjStruct
    structs: list[ProjStruct]
    enum_literals: dict[str, tuple[list[str], str]]


def kebab(name: str) -> str:
    return "-".join(tok.lower() for tok in PASCAL_SPLIT_RE.findall(name))


def camel(name: str) -> str:
    parts = re.split(r"[-_]", name)
    return parts[0] + "".join(p[:1].upper() + p[1:] for p in parts[1:])


def pascal(name: str) -> str:
    return "".join(p[:1].upper() + p[1:] for p in name.split("-"))


def build_projection(crate: Crate) -> Projection:
    root_name = pascal(crate.plugin_name) + "Config"
    structs: dict[str, ProjStruct] = {}
    enum_literals: dict[str, tuple[list[str], str]] = {}

    def add_named(name: str) -> None:
        if name in structs or name in enum_literals:
            return
        if name in crate.structs:
            rs = crate.structs[name]
            structs[name] = ProjStruct(rust_name=name, fields=rs.fields, order=rs.order)
            for f in rs.fields:
                collect_type(f.type_text)
            return
        if name in crate.enums:
            rust_enum = crate.enums[name]
            data_variants = [v for v in rust_enum.variants if v.fields]
            if data_variants:
                if len(rust_enum.variants) > 1:
                    raise GenError(f"{crate.dir_name}: enum {name} mixes data and unit variants")
                v = data_variants[0]
                structs[name] = ProjStruct(rust_name=name, fields=v.fields, order=rust_enum.order)
                for f in v.fields:
                    collect_type(f.type_text)
                return
            literals = [kebab(v.name) for v in rust_enum.variants]
            if not literals:
                raise GenError(f"{crate.dir_name}: enum {name} has no variants")
            default_v = next((v for v in rust_enum.variants if v.is_default), rust_enum.variants[0])
            enum_literals[name] = (literals, kebab(default_v.name))
            return
        raise GenError(f"{crate.dir_name}: type {name} is not defined in the plugin crate")

    def collect_type(type_text: str) -> None:
        for name in named_refs(parse_type(type_text)):
            add_named(name)

    if root_name in crate.structs:
        add_named(root_name)
    else:
        structs[root_name] = ProjStruct(rust_name=root_name, fields=[], order=-1)
    root = structs[root_name]
    ordered = topo_sort(list(structs.values()))
    return Projection(crate=crate, root=root, structs=ordered, enum_literals=enum_literals)


def topo_sort(items: list[ProjStruct]) -> list[ProjStruct]:
    remaining = sorted(items, key=lambda p: p.order)
    names = {p.rust_name for p in items}
    emitted: list[ProjStruct] = []
    done: set[str] = set()
    while remaining:
        progressed = False
        for ps in list(remaining):
            deps: set[str] = set()
            for f in ps.fields:
                deps.update(n for n in named_refs(parse_type(f.type_text)) if n in names)
            if deps <= done:
                emitted.append(ps)
                done.add(ps.rust_name)
                remaining.remove(ps)
                progressed = True
        if not progressed:
            raise GenError(f"cyclic struct dependency among {[p.rust_name for p in remaining]}")
    return emitted


@dataclass
class Default:
    ctor: str
    immutable: bool


def py_quote(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def struct_by_name(proj: Projection, name: str) -> ProjStruct:
    for ps in proj.structs:
        if ps.rust_name == name:
            return ps
    raise GenError(f"unknown projected struct {name}")


def resolve_struct_defaults(proj: Projection, ps: ProjStruct) -> dict[str, Default]:
    manual = proj.crate.defaults.get(ps.rust_name, {})
    out: dict[str, Default] = {}
    for f in ps.fields:
        if f.name in manual:
            out[f.name] = eval_default_expr(proj, manual[f.name], parse_type(f.type_text), f.name, 0)
        else:
            out[f.name] = zero_default(proj, parse_type(f.type_text))
    return out


def struct_ctor(proj: Projection, name: str, overrides: dict[str, str] | None) -> str:
    ps = struct_by_name(proj, name)
    own = resolve_struct_defaults(proj, ps)
    if overrides is None:
        return f"{name}()"
    override_ctor = {
        f.name: eval_default_expr(proj, overrides[f.name], parse_type(f.type_text), f.name, 0)
        for f in ps.fields
        if f.name in overrides
    }
    if all(override_ctor.get(f.name, own[f.name]).ctor == own[f.name].ctor for f in ps.fields):
        return f"{name}()"
    args = ", ".join(f"{f.name}={override_ctor.get(f.name, own[f.name]).ctor}" for f in ps.fields)
    return f"{name}({args})"


def zero_default(proj: Projection, tr: TypeRef) -> Default:
    if tr.kind in ("string", "path"):
        return Default(ctor='""', immutable=True)
    if tr.kind == "bool":
        return Default(ctor="False", immutable=True)
    if tr.kind == "int":
        return Default(ctor="0", immutable=True)
    if tr.kind == "float":
        return Default(ctor="0.0", immutable=True)
    if tr.kind == "option":
        return Default(ctor="None", immutable=True)
    if tr.kind == "vec":
        return Default(ctor="[]", immutable=False)
    if tr.kind == "bytes":
        return Default(ctor=f"bytes({tr.array_len})", immutable=True)
    if tr.kind == "tuple":
        items = [zero_default(proj, i) for i in tr.items]
        inner = ", ".join(i.ctor for i in items)
        if len(items) == 1:
            inner += ","
        return Default(ctor=f"({inner})", immutable=all(i.immutable for i in items))
    if tr.kind == "named":
        if tr.name in proj.enum_literals:
            return Default(ctor=py_quote(proj.enum_literals[tr.name][1]), immutable=True)
        if any(p.rust_name == tr.name for p in proj.structs):
            return Default(ctor=struct_ctor(proj, tr.name, None), immutable=False)
    raise GenError(f"{proj.crate.dir_name}: no zero default for type {tr.name}")


def eval_default_expr(
    proj: Projection, expr: str, tr: TypeRef, where: str, depth: int
) -> Default:
    if depth > 8:
        raise GenError(f"{proj.crate.dir_name}: default expr nesting too deep at {where}")
    e = " ".join(expr.split())
    if e == "None":
        return Default(ctor="None", immutable=True)
    if e in ("true", "false"):
        return Default(ctor="True" if e == "true" else "False", immutable=True)
    if INT_RE.match(e) or INT_EXPR_RE.match(e):
        return Default(ctor=e, immutable=True)
    if FLOAT_RE.match(e):
        return Default(ctor=e, immutable=True)
    sm = STR_LIT_RE.match(e)
    if sm is not None:
        return Default(ctor=py_quote(sm.group(1)), immutable=True)
    if e == "String::new()":
        return Default(ctor='""', immutable=True)
    if e == "Vec::new()":
        return Default(ctor="[]", immutable=False)
    if e.startswith("vec![") and e.endswith("]"):
        inner = e[len("vec![") : -1]
        elems = [
            eval_default_expr(proj, el, TypeRef(kind="string"), where, depth + 1)
            for el in split_top_level(inner)
        ]
        return Default(ctor="[" + ", ".join(el.ctor for el in elems) + "]", immutable=False)
    if CONST_RE.match(e):
        if e in proj.crate.consts:
            return eval_default_expr(proj, proj.crate.consts[e], tr, where, depth + 1)
        raise GenError(f"{proj.crate.dir_name}: unresolved const {e} in default for {where}")
    pm = PATH_LIT_RE.match(e)
    if pm is not None and pm.group(1) in proj.crate.enums:
        rust_enum = proj.crate.enums[pm.group(1)]
        if not any(v.name == pm.group(2) for v in rust_enum.variants):
            raise GenError(f"{proj.crate.dir_name}: unknown variant {e}")
        return Default(ctor=py_quote(kebab(pm.group(2))), immutable=True)
    lm = STRUCT_LIT_RE.match(e)
    if lm is not None and any(p.rust_name == lm.group(1) for p in proj.structs):
        overrides: dict[str, str] = {}
        for part in split_top_level(lm.group(2)):
            fm = IMPL_FIELD_RE.match(part)
            if fm is not None:
                overrides[fm.group(1)] = fm.group(2).strip().rstrip(",")
        return Default(
            ctor=struct_ctor(proj, lm.group(1), overrides), immutable=False
        )
    raise GenError(f"{proj.crate.dir_name}: unsupported default expr for {where}: {expr}")


def py_type(proj: Projection, tr: TypeRef) -> str:
    if tr.kind in ("string", "path"):
        return "str"
    if tr.kind == "bool":
        return "bool"
    if tr.kind == "int":
        return "int"
    if tr.kind == "float":
        return "float"
    if tr.kind == "option":
        return f"{py_type(proj, tr.items[0])} | None"
    if tr.kind == "vec":
        return f"list[{py_type(proj, tr.items[0])}]"
    if tr.kind == "tuple":
        return "tuple[" + ", ".join(py_type(proj, i) for i in tr.items) + "]"
    if tr.kind == "bytes":
        return "bytes"
    if tr.kind == "named":
        if tr.name in proj.enum_literals:
            lits = ", ".join(py_quote(l) for l in proj.enum_literals[tr.name][0])
            return f"Literal[{lits}]"
        return tr.name
    raise GenError(f"no python type for kind {tr.kind}")


def ts_type(proj: Projection, tr: TypeRef) -> str:
    if tr.kind in ("string", "path"):
        return "string"
    if tr.kind == "bool":
        return "boolean"
    if tr.kind in ("int", "float"):
        return "number"
    if tr.kind == "option":
        return ts_type(proj, tr.items[0])
    if tr.kind == "vec":
        return wrap_ts_array(ts_type(proj, tr.items[0]))
    if tr.kind == "tuple":
        return "[" + ", ".join(ts_type(proj, i) for i in tr.items) + "]"
    if tr.kind == "bytes":
        return "Uint8Array"
    if tr.kind == "named":
        if tr.name in proj.enum_literals:
            return " | ".join(f'"{l}"' for l in proj.enum_literals[tr.name][0])
        return tr.name
    raise GenError(f"no typescript type for kind {tr.kind}")


def wrap_ts_array(inner: str) -> str:
    if re.match(r"^[A-Za-z0-9_$]+$", inner):
        return f"readonly {inner}[]"
    return f"readonly ({inner})[]"


def emit_py_module(proj: Projection) -> str:
    crate = proj.crate
    legacy = LEGACY_FIELDS.get(crate.module_name, {})
    needs_factory = False
    for ps in proj.structs:
        defaults = resolve_struct_defaults(proj, ps)
        if any(not defaults[f.name].immutable for f in ps.fields):
            needs_factory = True
    uses_literal = bool(proj.enum_literals)
    uses_callable = any(
        "Callable" in lf[1] for fields in legacy.values() for lf in fields
    )
    lines: list[str] = []
    lines.append(f"# see: {crate.see_ref}")
    lines.append("from __future__ import annotations")
    lines.append("")
    if uses_callable:
        lines.append("from collections.abc import Callable")
    dc_import = "from dataclasses import dataclass"
    if needs_factory:
        dc_import += ", field"
    lines.append(dc_import)
    if uses_literal:
        lines.append("from typing import Literal")
    lines.append("")
    lines.append("from kelir.plugin import BasePlugin")
    lines.append("")
    public = [p.rust_name for p in proj.structs]
    public.append(crate.plugin_class)
    public.extend(LEGACY_ALIASES.get(crate.module_name, {}).values())
    lines.append("__all__ = [")
    for name in sorted(public):
        lines.append(f'    "{name}",')
    lines.append("]")
    for ps in proj.structs:
        lines.append("")
        lines.append("")
        lines.append("@dataclass")
        lines.append(f"class {ps.rust_name}:")
        defaults = resolve_struct_defaults(proj, ps)
        fields_legacy: list[tuple[str, str, str, str]] = legacy.get(ps.rust_name, [])
        if not ps.fields and not fields_legacy:
            lines.append("    pass")
            continue
        for f in ps.fields:
            d = defaults[f.name]
            ftype = py_type(proj, parse_type(f.type_text))
            if d.immutable:
                lines.append(f"    {f.name}: {ftype} = {d.ctor}")
            elif d.ctor == "[]":
                lines.append(f"    {f.name}: {ftype} = field(default_factory=list)")
            else:
                lines.append(f"    {f.name}: {ftype} = field(default_factory=lambda: {d.ctor})")
        for lf in fields_legacy:
            lines.append(f"    # {lf[3]}")
            lines.append(f"    {lf[0]}: {lf[1]} = {lf[2]}")
    aliases = LEGACY_ALIASES.get(crate.module_name, {})
    for rust_name in sorted(aliases):
        lines.append("")
        lines.append("")
        lines.append(f"{aliases[rust_name]} = {rust_name}")
    lines.append("")
    lines.append("")
    lines.append(f"class {crate.plugin_class}(BasePlugin):")
    lines.append(f'    name = "{crate.plugin_class_name}"')
    lines.append("")
    lines.append(f"    def __init__(self, config: {proj.root.rust_name} | None = None) -> None:")
    lines.append(
        f"        self.config = config if config is not None else {proj.root.rust_name}()"
    )
    lines.append("")
    return "\n".join(lines)


def emit_ts_group(proj: Projection) -> list[str]:
    crate = proj.crate
    lines: list[str] = []
    for ps in proj.structs:
        if ps.rust_name == proj.root.rust_name:
            continue
        lines.append(f"export interface {ps.rust_name} {{")
        for f in ps.fields:
            lines.append(f"  readonly {camel(f.name)}?: {ts_type(proj, parse_type(f.type_text))};")
        lines.append("}")
        lines.append("")
        lines.append("")
    lines.append(f"export interface {proj.root.rust_name} extends PluginEnablement {{")
    for f in proj.root.fields:
        lines.append(f"  readonly {camel(f.name)}?: {ts_type(proj, parse_type(f.type_text))};")
    lines.append("}")
    lines.append("")
    lines.append("")
    factory = camel(crate.plugin_name)
    handle = CONTEXT_HANDLES.get(crate.plugin_name)
    if handle is None:
        lines.append(f"export function {factory}(config: {proj.root.rust_name}): KelirPlugin {{")
    else:
        context_key, handle_type = handle
        lines.append(SERVICE_LOOKUP_SEE)
        lines.append(f"export function {factory}(")
        lines.append(f"  config: {proj.root.rust_name},")
        lines.append(f"): KelirPlugin & PluginContext<{{ {context_key}: {handle_type} }}> {{")
    lines.append(
        f'  return {{ name: "{crate.plugin_class_name}", enabled: config.enabled, config }};'
    )
    lines.append("}")
    return lines


def emit_ts(projections: list[Projection]) -> str:
    handles = [
        CONTEXT_HANDLES[proj.crate.plugin_name]
        for proj in projections
        if proj.crate.plugin_name in CONTEXT_HANDLES
    ]
    lines: list[str] = [TS_HEADER, ""]
    if handles:
        handle_types = ", ".join(handle_type for _, handle_type in handles)
        lines.append(f'import type {{ {handle_types} }} from "./context.js";')
        lines.append('import type { KelirPlugin, PluginContext } from "./plugin.js";')
    else:
        lines.append('import type { KelirPlugin } from "./plugin.js";')
    lines.append("")
    lines.append("export interface PluginEnablement {")
    lines.append("  readonly enabled?: boolean;")
    lines.append("}")
    for proj in projections:
        lines.append("")
        lines.append("")
        lines.extend(emit_ts_group(proj))
    lines.append("")
    return "\n".join(lines)


def emit_py_barrel(projections: list[Projection]) -> str:
    lines: list[str] = [PY_BARREL_HEADER]
    ordered = sorted(projections, key=lambda p: p.crate.module_name)
    for proj in ordered:
        config = proj.root.rust_name
        plugin = proj.crate.plugin_class
        lines.append(f"from kelir.plugins.{proj.crate.module_name} import {config} as {config}")
        lines.append(f"from kelir.plugins.{proj.crate.module_name} import {plugin} as {plugin}")
    lines.append("")
    names = sorted(
        [p.root.rust_name for p in projections] + [p.crate.plugin_class for p in projections]
    )
    lines.append("__all__ = [")
    for name in names:
        lines.append(f'    "{name}",')
    lines.append("]")
    lines.append("")
    return "\n".join(lines)


def write_if_changed(path: Path, content: str) -> bool:
    if path.exists() and path.read_text(encoding="utf-8") == content:
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8", newline="\n")
    return True


def main() -> int:
    interface_consts = collect_consts(INTERFACE_SRC)
    crate_dirs = sorted(
        d for d in PLUGINS_SRC.iterdir() if d.is_dir() and d.name.startswith("plugin-")
    )
    projections = [build_projection(parse_crate(d, interface_consts)) for d in crate_dirs]
    written: list[str] = []
    if write_if_changed(TS_OUT, emit_ts(projections)):
        written.append(str(TS_OUT.relative_to(REPO_ROOT)))
    expected_modules = {p.crate.module_name for p in projections}
    for path in sorted(PY_OUT_DIR.glob("*.py")):
        if path.name != "__init__.py" and path.stem not in expected_modules:
            raise GenError(f"unexpected python plugin module: {path.name}")
    for proj in projections:
        out = PY_OUT_DIR / f"{proj.crate.module_name}.py"
        if write_if_changed(out, emit_py_module(proj)):
            written.append(str(out.relative_to(REPO_ROOT)))
    barrel = PY_OUT_DIR / "__init__.py"
    if write_if_changed(barrel, emit_py_barrel(projections)):
        written.append(str(barrel.relative_to(REPO_ROOT)))
    for rel in written:
        print(f"wrote {rel}")
    print(f"plugin sdk projections: {len(projections)} plugins ok")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except GenError as exc:
        print(f"gen_plugin_sdk: {exc}", file=sys.stderr)
        sys.exit(1)
