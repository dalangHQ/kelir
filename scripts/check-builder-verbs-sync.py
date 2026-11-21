#!/usr/bin/env python3
"""check-builder-verbs-sync: the two builder-verb lists agree (L1/L2).

SSOT decision: scripts/builder-parity-verbs.yaml is the machine SSOT for
the builder verb set. It is what scripts/check-builder-parity.sh reads, it
carries the behavior: section the gate needs, and it matches the live SDKs.
cans/artifacts/config/builder-verbs.yaml is the spec-side mirror: the same
verb set plus per-verb args signature summaries and notes, which the
machine schema has no fields for. Generation from the machine file would
destroy the args/notes, so the mirror stays hand-edited and this script
keeps the two in agreement instead. The cans header claim that the parity
gate reads the cans file was false; the gate never read it.

Checks, all fail-closed (exit 1 with one FAIL line each):
  1. every scripts verb has a cans entry (params maps to the cans
     params + params_model overload split, both must exist);
  2. every cans verb has a scripts verb, except params_model (covered
     by scripts params), requires (known-open: scope requires is not
     yet gated, per-SDK spelling split plus undecided scope home),
     and the future-surface set below;
  3. future-surface cans verbs (spec-pinned, not live) must be absent
     from all three SDKs; the moment one is implemented this fails
     until scripts/builder-parity-verbs.yaml gains the verb;
  4. for shared verbs: the cans non-null SDK columns cover the scripts
     sdks set (extra columns only for the Rust scope entries listed
     below), the per-SDK spellings agree (same name mappings as
     scripts/check-builder-parity.sh), and placement/scope agree after
     vocabulary normalization (scripts scoped == cans scoped-entry
     for ring verbs; scripts flat + scope == cans scoped-entry for a
     pure scope entry or cans both for a dual-shape verb such as trap).

Known divergences, recorded not blessed:
  - params: flat typed-model form exists in TS/Py only; Rust has the
    scope form only. Neither schema has per-SDK placement, so the
    placement check is waived for params.
  - requires: scope requires exists in all three SDKs but the scripts
    file gates only flat require; gating the scope form needs a
    per-SDK spelling table the gate does not have yet.
  - Rust scope entries (actions, streams, vsock, deny) live in
    builder_scopes.flow.rs, so the parity gate does not flat-check
    them in Rust; the cans rust column is still correct.

Usage: python3 scripts/check-builder-verbs-sync.py
Exit 0 when the lists agree, else exit 1.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS_YAML = ROOT / "scripts" / "builder-parity-verbs.yaml"
CANS_YAML = ROOT / "cans" / "artifacts" / "config" / "builder-verbs.yaml"

# see: scripts/check-builder-parity.sh
RS_BUILDER_FILES = [
    ROOT / "crates" / "kelir" / "src" / "builder_sdk.flow.rs",
    ROOT / "crates" / "kelir" / "src" / "builder_plugin.flow.rs",
    ROOT / "crates" / "kelir" / "src" / "builder_scopes.flow.rs",
]
TS_BUILDER_FILES = [
    ROOT / "packages" / "ts" / "core" / "src" / "builder.lifecycle.flow.ts",
    ROOT / "packages" / "ts" / "core" / "src" / "hook.registry.flow.ts",
]
PY_BUILDER_FILES = [
    ROOT / "packages" / "py" / "kelir" / "kelir_builder.py",
    ROOT / "packages" / "py" / "kelir" / "kelir_builder_scope.py",
]

# see: cans/02-plugin-model.md#Nested-scopes
FORWARD_MAP = {"params": {"params", "params_model"}}
REVERSE_MAP = {"params_model": "params"}
KNOWN_OPEN = {
    "requires": "cans models require/requires as two entries while the "
    "scripts file gates them as one verb (require, both/all); the sync "
    "matches by name, so this entry stays excepted. Scope home is decided "
    "(universal, cans/02#Nested-scopes) and gated, not open.",
}
# see: cans/artifacts/config/builder-verbs.yaml
FUTURE_SURFACE = {
    "driver",
    "kernel",
    "memory_mb",
    "rootfs",
    "tsc_locked",
    "vcpus",
    "watchdog",
    "with_composite",
}
# see: crates/kelir/src/builder_scopes.flow.rs
EXTRA_SDK_OK = {
    ("actions", "rust"),
    ("streams", "rust"),
    ("vsock", "rust"),
    ("deny", "rust"),
}
# see: scripts/check-builder-parity.sh
SCOPE_METHOD = {
    "register_custom_action": "register",
    "register_stream_kind": "register",
    "vsock_service": "service",
    "params": "model",
}


def snake_to_camel(name):
    parts = name.split("_")
    return parts[0] + "".join(p[:1].upper() + p[1:] for p in parts[1:])


def ts_name(name):
    if name == "require":
        return "requires"
    return snake_to_camel(name)


def parse_scripts_yaml(text):
    verbs, cur, section = {}, None, None
    for line in text.splitlines():
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
        m = re.match(r"^    (placement|scope|return|sdks|status):\s*(.*)$", line)
        if m and cur:
            verbs[cur][m.group(1)] = m.group(2).strip()
    for name, fields in verbs.items():
        fields["sdks"] = set(fields.get("sdks", "").split(",")) - {""}
    return verbs


def parse_cans_yaml(text):
    verbs, cur, section = {}, None, None
    for line in text.splitlines():
        if line == "verbs:":
            section = "verbs"
            continue
        if line == "meta:":
            section = None
        if section != "verbs":
            continue
        m = re.match(r"^  - name: (\S+)\s*$", line)
        if m:
            cur = m.group(1)
            verbs[cur] = {}
            continue
        m = re.match(
            r"^    (rust|typescript|python|placement|scope|returns):\s*(.*)$", line
        )
        if m and cur:
            verbs[cur][m.group(1)] = m.group(2).strip()
    return verbs


def sdk_implemented(name):
    """True when a live SDK defines a method with this canonical name."""
    hits = []
    rs_pat = re.compile(rf"pub (async )?fn {re.escape(name)}[<(]")
    for path in RS_BUILDER_FILES:
        if rs_pat.search(path.read_text()):
            hits.append(f"rust:{path.name}")
    ts_pat = re.compile(rf"^  {re.escape(snake_to_camel(name))}[<(]", re.M)
    for path in TS_BUILDER_FILES:
        if ts_pat.search(path.read_text()):
            hits.append(f"ts:{path.name}")
    py_pat = re.compile(rf"^    (async )?def {re.escape(name)}\(", re.M)
    for path in PY_BUILDER_FILES:
        if py_pat.search(path.read_text()):
            hits.append(f"py:{path.name}")
    return hits


def expected_spellings(scripts_name):
    """(rust, typescript, python) spellings for a shared verb name."""
    if scripts_name in SCOPE_METHOD:
        method = SCOPE_METHOD[scripts_name]
        return (method, method, method)
    rust = "r#use" if scripts_name == "use" else scripts_name
    return (rust, ts_name(scripts_name), scripts_name)


def check_placement(scripts_name, s_fields, c_fields, failures, checks):
    if scripts_name == "params":
        return
    s_placement, s_scope = s_fields.get("placement"), s_fields.get("scope")
    c_placement, c_scope = c_fields.get("placement"), c_fields.get("scope")
    checks[0] += 1
    if s_placement == "flat" and not s_scope:
        ok = c_placement == "flat" and c_scope in (None, "null")
    elif s_placement == "flat" and s_scope:
        ok = (c_placement == "scoped-entry" and c_scope in (None, "null")) or (
            c_placement == "both" and c_scope == s_scope
        )
    elif s_placement == "scoped":
        ok = c_placement in ("scoped", "scoped-entry") and c_scope == s_scope
    elif s_placement == "both":
        ok = c_placement == "both" and c_scope == s_scope
    elif s_placement == "registry":
        ok = c_placement == "registry" and c_scope in (None, "null")
    else:
        ok = False
    if not ok:
        failures.append(
            f"placement: {scripts_name} scripts "
            f"{s_placement}/{s_scope or '-'} vs cans "
            f"{c_placement}/{c_scope or '-'}"
        )


def main():
    failures, checks = [], [0]
    if not SCRIPTS_YAML.is_file():
        print(f"FAIL: missing input file: {SCRIPTS_YAML}")
        return 1
    if not CANS_YAML.is_file():
        print(f"FAIL: missing input file: {CANS_YAML}")
        return 1
    scripts = parse_scripts_yaml(SCRIPTS_YAML.read_text())
    cans = parse_cans_yaml(CANS_YAML.read_text())
    if not scripts:
        print("FAIL: no verbs parsed from scripts/builder-parity-verbs.yaml")
        return 1
    if not cans:
        print("FAIL: no verbs parsed from cans/artifacts/config/builder-verbs.yaml")
        return 1

    for name in sorted(scripts):
        checks[0] += 1
        want = FORWARD_MAP.get(name, {name})
        missing = want - set(cans)
        if missing:
            failures.append(
                f"coverage: scripts verb {name} has no cans entry for {sorted(missing)}"
            )
    for name in sorted(cans):
        checks[0] += 1
        if name in scripts or name in REVERSE_MAP or name in KNOWN_OPEN:
            continue
        if name in FUTURE_SURFACE:
            hits = sdk_implemented(name)
            checks[0] += 1
            if hits:
                failures.append(
                    f"future-surface: cans-only verb {name} is now implemented "
                    f"in {', '.join(hits)}; add it to "
                    "scripts/builder-parity-verbs.yaml"
                )
            continue
        failures.append(f"coverage: cans verb {name} has no scripts verb")

    for name in sorted(set(REVERSE_MAP) | set(KNOWN_OPEN) | FUTURE_SURFACE):
        checks[0] += 1
        if name not in cans:
            failures.append(
                f"stale exception: {name} is listed as a known exception "
                "but has no cans entry; remove it from the exception list"
            )

    for name in sorted(set(scripts) & set(cans)):
        s_fields, c_fields = scripts[name], cans[name]
        cans_sdks = {
            sdk
            for sdk, col in (
                ("rust", "rust"),
                ("ts", "typescript"),
                ("py", "python"),
            )
            if c_fields.get(col) not in (None, "null")
        }
        checks[0] += 1
        if not cans_sdks >= s_fields["sdks"]:
            failures.append(
                f"sdks: {name} scripts requires {sorted(s_fields['sdks'])} "
                f"but cans lists only {sorted(cans_sdks)}"
            )
        checks[0] += 1
        extra = {(name, sdk) for sdk in cans_sdks - s_fields["sdks"]}
        if not extra <= EXTRA_SDK_OK:
            failures.append(
                f"sdks: {name} cans lists {sorted(cans_sdks)} but scripts "
                f"requires only {sorted(s_fields['sdks'])}"
            )
        checks[0] += 1
        if name != "params":
            want_rs, want_ts, want_py = expected_spellings(name)
            got = (
                c_fields.get("rust"),
                c_fields.get("typescript"),
                c_fields.get("python"),
            )
            want = (
                want_rs if "rust" in cans_sdks else "null",
                want_ts if "ts" in cans_sdks else "null",
                want_py if "py" in cans_sdks else "null",
            )
            if got != want:
                failures.append(
                    f"spelling: {name} cans lists {got} want {want}"
                )
        check_placement(name, s_fields, c_fields, failures, checks)

    if "params_model" in cans:
        for name in ("params_model",):
            checks[0] += 1
            got = (
                cans[name].get("rust"),
                cans[name].get("typescript"),
                cans[name].get("python"),
            )
            if got != ("model", "model", "model"):
                failures.append(f"spelling: {name} cans lists {got} want ('model', 'model', 'model')")
            checks[0] += 1
            if cans[name].get("placement") != "scoped" or cans[name].get("scope") != "params":
                failures.append(
                    f"placement: {name} cans lists "
                    f"{cans[name].get('placement')}/{cans[name].get('scope')} "
                    "want scoped/params"
                )
    if "params" in cans:
        checks[0] += 1
        got = (
            cans["params"].get("rust"),
            cans["params"].get("typescript"),
            cans["params"].get("python"),
        )
        if got != ("params", "params", "params"):
            failures.append(
                f"spelling: params cans lists {got} want ('params', 'params', 'params')"
            )
        checks[0] += 1
        if cans["params"].get("placement") != "both" or cans["params"].get("scope") not in (
            None,
            "null",
        ):
            failures.append(
                f"placement: params cans lists "
                f"{cans['params'].get('placement')}/{cans['params'].get('scope')} "
                "want both/null"
            )
    if "requires" in cans:
        checks[0] += 1
        got = (
            cans["requires"].get("rust"),
            cans["requires"].get("typescript"),
            cans["requires"].get("python"),
        )
        if got != ("requires", "requires", "require"):
            failures.append(
                f"spelling: requires cans lists {got} "
                "want ('requires', 'requires', 'require')"
            )

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print(
        f"OK: builder verbs sync: {checks[0]} checks passed "
        f"({len(scripts)} scripts verbs, {len(cans)} cans verbs)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
