#!/usr/bin/env bash
# see: cans/02-plugin-model.md#Unified-builder
# see: cans/03-sdk-architecture.md#Rust-SDK
# see: cans/03-sdk-architecture.md#TypeScript-SDK
# see: scripts/builder-parity-verbs.yaml
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
YAML="$ROOT/scripts/builder-parity-verbs.yaml"
RS_BUILDER="crates/kelir/src/builder_sdk.rs crates/kelir/src/builder_plugin.rs"
RS_SCOPES="crates/kelir/src/builder_scopes.rs"
TS_BUILDER="packages/ts/core/src/builder.lifecycle.ts"
TS_SCOPES="packages/ts/core/src/hook.registry.ts"
PY_FILE="packages/py/kelir/kelir_builder.py"
GATE_FAIL=0
CHECKS=0
FAILED=0
WORK="$(mktemp -d "${TMPDIR:-/tmp}/kelir-builder-parity.XXXXXX")"
# see: cans/02-plugin-model.md#Unified-builder
trap 'rm -rf "$WORK"' EXIT

check() { CHECKS=$((CHECKS + 1)); }
fail() { echo "FAIL: $1"; GATE_FAIL=1; FAILED=$((FAILED + 1)); }

# see: scripts/builder-parity-verbs.yaml
for f in "$YAML" $RS_BUILDER "$RS_SCOPES" "$TS_BUILDER" "$TS_SCOPES" "$PY_FILE"; do
    if [ ! -f "$f" ]; then echo "FAIL: missing input file: $f"; exit 1; fi
done

# see: scripts/builder-parity-verbs.yaml
snake_to_camel() {
    printf '%s' "$1" | awk -F_ '{ out=$1; for (i=2;i<=NF;i++) out=out toupper(substr($i,1,1)) substr($i,2); print out }'
}
ts_name() {
    case "$1" in
        require) printf '%s' "requires" ;;
        *) snake_to_camel "$1" ;;
    esac
}
scope_type() {
    case "$1" in
        trap) printf '%s' "TrapScope" ;;
        fs) printf '%s' "FsScope" ;;
        net) printf '%s' "NetScope" ;;
        console) printf '%s' "ConsoleScope" ;;
        exec) printf '%s' "ExecScope" ;;
        host) printf '%s' "HostScope" ;;
        lifecycle) printf '%s' "LifecycleScope" ;;
        telemetry) printf '%s' "TelemetryScope" ;;
        *) printf '%s' "" ;;
    esac
}

# see: scripts/builder-parity-verbs.yaml
declare -A PLACEMENT SCOPE RET SDKS STATUS
VERBS=()
while IFS='|' read -r kind a b c; do
    case "$kind" in
        VERB) VERBS+=("$a") ;;
        FIELD)
            case "$b" in
                placement) PLACEMENT["$a"]="$c" ;;
                scope) SCOPE["$a"]="$c" ;;
                return) RET["$a"]="$c" ;;
                sdks) SDKS["$a"]="$c" ;;
                status) STATUS["$a"]="$c" ;;
            esac
            ;;
    esac
done < <(awk '
    /^verbs:/ { v=1; next }
    /^[^ #]/ { v=0 }
    v && /^  [A-Za-z][A-Za-z0-9_]*:/ { sub(/^  /,""); sub(/:$/,""); name=$0; print "VERB|" name; next }
    v && /^    [a-z]+:/ && name != "" {
        line=$0; sub(/^    /,"",line)
        key=line; sub(/:.*$/,"",key)
        val=line; sub(/^[^:]*: */,"",val)
        print "FIELD|" name "|" key "|" val
    }
' "$YAML")
if [ "${#VERBS[@]}" -eq 0 ]; then echo "FAIL: no verbs parsed from $YAML"; exit 1; fi
for verb in "${VERBS[@]}"; do
    check
    if [ -z "${PLACEMENT[$verb]:-}" ] || [ -z "${RET[$verb]:-}" ] || [ -z "${SDKS[$verb]:-}" ]; then
        fail "yaml: verb $verb is missing placement/return/sdks"
    fi
    case "${PLACEMENT[$verb]:-}" in flat|both|registry) ;; *) fail "yaml: verb $verb has bad placement ${PLACEMENT[$verb]:-}" ;; esac
    case "${RET[$verb]:-}" in builder|sealed|terminal|scope|registry) ;; *) fail "yaml: verb $verb has bad return ${RET[$verb]:-}" ;; esac
done

# see: cans/02-plugin-model.md#Unified-builder
# see: cans/artifacts/config/builder-verbs.yaml
rust_fn_pat() { if [ "$1" = "use" ]; then printf '%s' "r#use"; else printf '%s' "$1"; fi }
rust_flat_present() { grep -qE "pub (async )?fn $(rust_fn_pat "$1")[<\(]" $RS_BUILDER; }
ts_flat_present() { grep -qE "^  (async )?$(ts_name "$1")(<|\\()" "$TS_BUILDER"; }
ts_registry_present() { grep -qE "^  $1\\(" "$TS_SCOPES"; }
py_flat_present() {
    [ "$(awk -v verb="$1" '
        /^class KelirBuilder:/ { in_class=1; next }
        in_class && ($0 ~ "^    (async )?def " verb "\\(" || $0 ~ "^    " verb " = ") { found=1 }
        END { print found+0 }
    ' "$PY_FILE")" = "1" ]
}
# see: cans/artifacts/config/builder-verbs.yaml
py_is_descriptor() {
    [ "$(awk -v verb="$1" '
        /^class KelirBuilder:/ { in_class=1; next }
        in_class && $0 ~ "^    " verb " = " { found=1 }
        END { print found+0 }
    ' "$PY_FILE")" = "1" ]
}
rust_scoped_present() {
    [ "$(awk -v scope="$2" -v verb="$1" '
        /^pub struct [A-Za-z]+Scope</ { s=$3; sub(/<.*/,"",s); cur=s; next }
        /^impl/ { in_impl=1; next }
        in_impl && /^    [A-Za-z]+Scope</ { s=$1; sub(/<.*/,"",s); cur=s; next }
        in_impl && /^}/ { in_impl=0; cur=""; next }
        in_impl && cur == scope && $0 ~ "pub (async )?fn " verb "[<\\(]" { found=1 }
        END { print found+0 }
    ' "$RS_SCOPES")" = "1" ]
}
ts_scoped_present() {
    [ "$(awk -v scope="$2" -v verb="$1" '
        /^export interface [A-Za-z]+Scope/ { s=$3; sub(/<.*/,"",s); cur=s; next }
        cur != "" && /^}/ { cur=""; next }
        cur == scope && $0 ~ "^[[:space:]]+" verb "\\(" { found=1 }
        END { print found+0 }
    ' "$TS_SCOPES")" = "1" ]
}
py_scoped_present() {
    [ "$(awk -v scope="$2" -v verb="$1" '
        /_SCOPE_VERBS: dict/ { in_map=1; next }
        in_map && /^}/ { in_map=0; next }
        in_map && /": \(/ { k=$0; sub(/^[[:space:]]*"/,"",k); sub(/".*/,"",k); cur=k }
        in_map && cur == scope && $0 ~ "\"" verb "\"" { found=1 }
        END { print found+0 }
    ' "$PY_FILE")" = "1" ]
}

# see: cans/02-plugin-model.md#Unified-builder
classify_rust_sig() {
    local sig="$1"
    if printf '%s' "$sig" | grep -q -- "Result<Session"; then printf '%s' "terminal";
    elif printf '%s' "$sig" | grep -q -- "Result<KelirBuilder<Host, Sealed"; then printf '%s' "sealed";
    elif printf '%s' "$sig" | grep -q -- "Result<Self"; then printf '%s' "result";
    elif printf '%s' "$sig" | grep -q -- "Result<KelirBuilder"; then printf '%s' "result";
    elif printf '%s' "$sig" | grep -q -- "-> Self"; then printf '%s' "builder";
    elif printf '%s' "$sig" | grep -q -- "-> KelirBuilder"; then printf '%s' "builder";
    else printf '%s' "void"; fi
}
classify_ts_sig() {
    local sig="$1"
    if printf '%s' "$sig" | grep -q -- "Promise<Session"; then printf '%s' "terminal";
    elif printf '%s' "$sig" | grep -q -- ": this"; then printf '%s' "builder";
    elif printf '%s' "$sig" | grep -q -- ": KelirBuilder"; then printf '%s' "builder";
    elif printf '%s' "$sig" | grep -qE -- ": [A-Za-z]+Scope<Ctx>"; then printf '%s' "scope";
    elif printf '%s' "$sig" | grep -q -- ": void"; then printf '%s' "void";
    elif printf '%s' "$sig" | grep -q -- ": HookRegistry"; then printf '%s' "registry";
    else printf '%s' "other"; fi
}
classify_py_sig() {
    local sig="$1"
    if printf '%s' "$sig" | grep -q -- "-> Self"; then printf '%s' "builder";
    elif printf '%s' "$sig" | grep -q -- "-> KelirBuilder"; then printf '%s' "builder";
    elif printf '%s' "$sig" | grep -q -- "-> Session"; then printf '%s' "terminal";
    elif printf '%s' "$sig" | grep -q -- "-> None"; then printf '%s' "void";
    else printf '%s' "void"; fi
}
# see: cans/02-plugin-model.md#Unified-builder
rust_accept() {
    case "$1:$2" in
        builder:builder|sealed:sealed|terminal:terminal|scope:builder) return 0 ;;
        *) return 1 ;;
    esac
}
ts_accept() {
    case "$1:$2" in
        builder:builder|sealed:builder|terminal:terminal|scope:scope|registry:builder|registry:registry) return 0 ;;
        *) return 1 ;;
    esac
}
py_accept() {
    case "$1:$2" in
        builder:builder|sealed:builder|terminal:terminal|scope:builder) return 0 ;;
        *) return 1 ;;
    esac
}
join_sig() {
    # see: cans/02-plugin-model.md#Unified-builder
    sed -n "$2,$(( $2 + $3 ))p" "$1" | awk -v pat="$4" '
        {
            line=line $0 " "
            n=split($0, chars, "")
            for (i=1; i<=n; i++) {
                if (chars[i] == "(") depth++
                else if (chars[i] == ")") depth--
            }
            if (depth <= 0 && $0 ~ pat) exit
        }
        END { print line }
    '
}

# see: cans/02-plugin-model.md#Unified-builder
check_rust_return() {
    local verb="$1" expected="$2" files="$3" where="$4" class sig n f
    for f in $files; do
        while IFS=':' read -r n _; do
            [ -n "$n" ] || continue
            sig="$(join_sig "$f" "$n" 12 '->|[{]')"
            class="$(classify_rust_sig "$sig")"
            check
            if rust_accept "$expected" "$class"; then
                if [ "$class" = "terminal" ]; then printf '%s\n' "$verb" >> "$WORK/terminal.rust"; fi
            else
                fail "rust: .$verb $where returns $class, want $expected ($f:$n)"
            fi
        done < <(grep -nE "pub (async )?fn $(rust_fn_pat "$verb")[<\(]" "$f" || true)
    done
}
check_ts_return() {
    local verb="$1" expected="$2" tname="$3" class sig n
    while IFS=':' read -r n _; do
        [ -n "$n" ] || continue
        sig="$(join_sig "$TS_BUILDER" "$n" 20 '[{;]')"
        case "$sig" in *'; ') continue ;; esac
        class="$(classify_ts_sig "$sig")"
        check
        if ts_accept "$expected" "$class"; then
            if [ "$class" = "terminal" ]; then printf '%s\n' "$verb" >> "$WORK/terminal.ts"; fi
        else
            fail "ts: .$verb returns $class, want $expected ($TS_BUILDER:$n)"
        fi
    done < <(grep -nE "^  (async )?${tname}(<|\\()" "$TS_BUILDER" || true)
}
check_ts_registry_return() {
    local verb="$1" class sig n
    while IFS=':' read -r n _; do
        [ -n "$n" ] || continue
        sig="$(join_sig "$TS_SCOPES" "$n" 20 '[{;]')"
        case "$sig" in *'; ') continue ;; esac
        class="$(classify_ts_sig "$sig")"
        check
        if ! ts_accept "registry" "$class"; then
            fail "ts: HookRegistry.$verb returns $class, want registry (this) ($TS_SCOPES:$n)"
        fi
    done < <(grep -nE "^  ${verb}\\(" "$TS_SCOPES" || true)
}
check_py_return() {
    local verb="$1" expected="$2" range_start="$3" range_end="$4" class sig n
    while IFS=':' read -r n _; do
        [ -n "$n" ] || continue
        sig="$(join_sig "$PY_FILE" "$n" 12 '->.*:|\)')"
        class="$(classify_py_sig "$sig")"
        check
        if py_accept "$expected" "$class"; then
            if [ "$class" = "terminal" ]; then printf '%s\n' "$verb" >> "$WORK/terminal.py"; fi
        else
            fail "py: .$verb returns $class, want $expected ($PY_FILE:$n)"
        fi
    done < <(awk -v verb="$verb" -v from="$range_start" -v to="$range_end" '
        NR >= from && (to == 0 || NR < to) && $0 ~ "^    (async )?def " verb "\\(" { print NR ":" $0 }
    ' "$PY_FILE" || true)
}
check_ts_scoped_return() {
    local verb="$1" tname="$2" stype="$3" block
    block="$(awk -v scope="$stype" '
        $0 ~ "^export interface " scope "<" { in_block=1 }
        in_block { print }
        in_block && /^}/ { exit }
    ' "$TS_SCOPES" | tr '\n' ' ')"
    check
    if ! printf '%s' "$block" | awk -v RS=';' -v verb="$tname" -v scope="$stype" '
        $0 ~ verb && $0 ~ "\\): " scope "<Ctx" { found=1 }
        END { exit !found }
    '; then
        fail "ts: .$verb in $stype does not return $stype (hook.registry.ts)"
    fi
}

# see: cans/02-plugin-model.md#Unified-builder
PY_CLASS_LINE="$(grep -n '^class KelirBuilder:' "$PY_FILE" | cut -d: -f1)"
PY_SCOPE_END="$(grep -n '^class _PluginMount:' "$PY_FILE" | cut -d: -f1)"
PY_SCOPE_START="$(grep -n '^class _Scope:' "$PY_FILE" | cut -d: -f1)"
for verb in "${VERBS[@]}"; do
    placement="${PLACEMENT[$verb]}"
    expected="${RET[$verb]}"
    scope="${SCOPE[$verb]:-}"
    stype="$(scope_type "$scope")"
    case ",${SDKS[$verb]}," in
        *",rust,"*)
            if [ "$placement" = "registry" ]; then :; else
                if [ "$placement" = "flat" ] || [ "$placement" = "both" ]; then
                    check
                    if rust_flat_present "$verb"; then
                        check_rust_return "$verb" "$expected" "$RS_BUILDER" "flat"
                    else
                        fail "rust: .$verb missing from builder (flat)"
                    fi
                fi
                if [ "$placement" = "both" ] || [ "$placement" = "scoped" ]; then
                    check
                    if rust_scoped_present "$verb" "$stype"; then
                        check_rust_return "$verb" "scope" "$RS_SCOPES" "scoped in $stype"
                    else
                        fail "rust: .$verb missing from $stype (scoped)"
                    fi
                fi
            fi
            ;;
    esac
    case ",${SDKS[$verb]}," in
        *",ts,"*)
            tname="$(ts_name "$verb")"
            if [ "$placement" = "registry" ]; then
                check
                if ts_registry_present "$verb"; then
                    check_ts_registry_return "$verb"
                else
                    fail "ts: HookRegistry.$verb missing"
                fi
            else
                if [ "$placement" = "flat" ] || [ "$placement" = "both" ]; then
                    check
                    if ts_flat_present "$verb"; then
                        check_ts_return "$verb" "$expected" "$tname"
                    else
                        fail "ts: .$verb missing from builder (flat)"
                    fi
                fi
                if [ "$placement" = "both" ] || [ "$placement" = "scoped" ]; then
                    check
                    if ts_scoped_present "$tname" "$stype"; then
                        check_ts_scoped_return "$verb" "$tname" "$stype"
                    else
                        fail "ts: .$verb missing from $stype interface (scoped)"
                    fi
                fi
            fi
            ;;
    esac
    case ",${SDKS[$verb]}," in
        *",py,"*)
            if [ "$placement" = "registry" ]; then :; else
                if [ "$placement" = "flat" ] || [ "$placement" = "both" ]; then
                    check
                    if py_flat_present "$verb"; then
                        if py_is_descriptor "$verb"; then
                            check
                        else
                            check_py_return "$verb" "$expected" "$PY_CLASS_LINE" 0
                        fi
                    else
                        fail "py: .$verb missing from builder (flat)"
                    fi
                fi
                if [ "$placement" = "both" ] || [ "$placement" = "scoped" ]; then
                    check
                    if py_scoped_present "$verb" "$scope"; then
                        check_py_return "$verb" "scope" "$PY_SCOPE_START" "$PY_SCOPE_END"
                    else
                        fail "py: .$verb missing from _SCOPE_VERBS[$scope] (scoped)"
                    fi
                fi
            fi
            ;;
    esac
done

# see: cans/02-plugin-model.md#Unified-builder
check_terminal_set() {
    local sdk="$1" file="$2" got
    check
    if [ ! -f "$file" ]; then fail "$sdk: no terminal verb found (want boot only)"; return; fi
    got="$(sort -u "$file" | tr '\n' ' ')"
    if [ "$got" != "boot " ]; then fail "$sdk: terminal verbs are [$got], want [boot] only"; fi
}
touch "$WORK/terminal.rust" "$WORK/terminal.ts" "$WORK/terminal.py"
check_terminal_set "rust" "$WORK/terminal.rust"
check_terminal_set "ts" "$WORK/terminal.ts"
check_terminal_set "py" "$WORK/terminal.py"

# see: cans/02-plugin-model.md#Unified-builder
check
rust_session_verbs="$(grep -hE "pub (async )?fn [a-z_]+.*Result<Session" $RS_BUILDER | sed -E 's/.*fn ([a-z_]+).*/\1/' | sort -u | tr '\n' ' ')"
if [ "$rust_session_verbs" != "boot " ]; then fail "rust: Session-returning verbs are [$rust_session_verbs], want [boot] only"; fi
check
ts_session_verbs="$(grep -hE "^  (async )?[a-zA-Z_]+\(.*Promise<Session" "$TS_BUILDER" | sed -E 's/^  (async )?([a-zA-Z_]+).*/\2/' | sort -u | tr '\n' ' ')"
if [ "$ts_session_verbs" != "boot " ]; then fail "ts: Session-returning verbs are [$ts_session_verbs], want [boot] only"; fi
check
py_session_verbs="$(grep -hE "^    (async )?def [a-z_]+.*-> Session" "$PY_FILE" | sed -E 's/^    (async )?def ([a-z_]+).*/\2/' | grep -v '^_' | sort -u | tr '\n' ' ')"
if [ "$py_session_verbs" != "boot " ]; then fail "py: Session-returning verbs are [$py_session_verbs], want [boot] only"; fi

# see: cans/02-plugin-model.md#Unified-builder
check
if grep -qE "pub (async )?fn (fs|net|console|exec|host|lifecycle|telemetry)[<\(]" "$RS_SCOPES"; then
    fail "rust: scope-entry verb defined inside a scope type (nesting depth > 1)"
fi
check
ts_scope_methods="$(awk '
    /^export interface [A-Za-z]+Scope/ { s=$3; sub(/<.*/,"",s); cur=s; next }
    cur != "" && /^}/ { cur=""; next }
    cur != "" && $0 ~ /^[[:space:]]+on[A-Z][A-Za-z]*\(/ { m=$0; sub(/^[[:space:]]+/,"",m); sub(/\(.*/,"",m); print m }
    cur != "" && $0 ~ /^[[:space:]]+[a-z][A-Za-z]*\(/ { m=$0; sub(/^[[:space:]]+/,"",m); sub(/\(.*/,"",m); print m }
' "$TS_SCOPES")"
bad_ts_scope="$(printf '%s\n' "$ts_scope_methods" | grep -vE '^on[A-Z]|^end$' || true)"
if [ -n "$bad_ts_scope" ]; then fail "ts: non-leaf method inside a scope interface (nesting depth > 1): $bad_ts_scope"; fi
check
py_scope_defs="$(awk -v from="$PY_SCOPE_START" -v to="$PY_SCOPE_END" '
    NR >= from && NR < to && $0 ~ /^    def [a-z_]+/ { m=$0; sub(/^    def /,"",m); sub(/\(.*/,"",m); print m }
' "$PY_FILE")"
bad_py_scope="$(printf '%s\n' "$py_scope_defs" | grep -vE '^(on_[a-z_]+|__init__|_scoped_on|end)$' || true)"
if [ -n "$bad_py_scope" ]; then fail "py: non-leaf method inside _Scope (nesting depth > 1): $bad_py_scope"; fi

# see: cans/artifacts/config/builder-verbs.yaml
check
rust_end_count="$(grep -cE '^    pub fn end\(self\) -> KelirBuilder<' "$RS_SCOPES" || true)"
if [ "$rust_end_count" != "8" ]; then fail "rust: scope .end() defined $rust_end_count times, want 8 (one per scope)"; fi
check
ts_end_count="$(grep -cE '^  end\(\): Owner;$' "$TS_SCOPES" || true)"
if [ "$ts_end_count" != "8" ]; then fail "ts: scope .end() declared $ts_end_count times, want 8 (one per scope)"; fi
check
py_end_count="$(awk -v from="$PY_SCOPE_START" -v to="$PY_SCOPE_END" 'NR >= from && (to == 0 || NR < to) && $0 ~ "^    def end\(" { n++ } END { print n+0 }' "$PY_FILE")"
if [ "$py_end_count" != "1" ]; then fail "py: _Scope.end() defined $py_end_count times, want 1"; fi

if [ "$GATE_FAIL" -eq 0 ]; then
    echo "OK: builder parity: $CHECKS checks passed (${#VERBS[@]} verbs, rust/ts/py)"
fi
exit "$GATE_FAIL"
