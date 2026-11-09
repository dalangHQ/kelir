// see: cans/02-plugin-model.md#Auto-generated-manifest-contract
// see: cans/03-sdk-architecture.md#Python-SDK
use quote::ToTokens;

use crate::model_codegen::Ty;
use crate::sdk_codegen::{ProjStruct, Projection, py_kebab};
use syn::parse::Parser;

#[derive(Debug, Clone)]
struct Default {
    ctor: String,
    immutable: bool,
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn legacy_aliases(module: &str) -> Vec<(&'static str, &'static str)> {
    match module {
        "chaos" => vec![
            ("NetworkFaultRule", "NetworkChaos"),
            ("StorageFaultRule", "FsChaos"),
        ],
        _ => Vec::new(),
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
type LegacyField = (&'static str, &'static str, &'static str, &'static str);
type LegacyFields = Vec<(&'static str, Vec<LegacyField>)>;

// see: cans/03-sdk-architecture.md#Python-SDK
fn legacy_fields(module: &str) -> LegacyFields {
    match module {
        "db_proxy" => vec![(
            "EphemeralCredentials",
            vec![(
                "resolve",
                "Callable[[], str] | None",
                "None",
                "TODO (L3): resolve closure injection at the host connection boundary per cans/plugins/db-proxy.md.",
            )],
        )],
        _ => Vec::new(),
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn py_quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn lit_text(lit: &syn::Lit) -> String {
    lit.to_token_stream().to_string()
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn is_const_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn valid_int_lit(text: &str) -> bool {
    let text = text.strip_prefix('-').unwrap_or(text);
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        return !hex.is_empty() && hex.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '_');
    }
    !text.is_empty()
        && text.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        && text.chars().all(|ch| ch.is_ascii_digit() || ch == '_')
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn valid_float_lit(text: &str) -> bool {
    let text = text.strip_prefix('-').unwrap_or(text);
    let Some((head, tail)) = text.split_once('.') else {
        return false;
    };
    !head.is_empty()
        && head.chars().all(|ch| ch.is_ascii_digit())
        && !tail.is_empty()
        && tail.chars().all(|ch| ch.is_ascii_digit())
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn parse_num(bytes: &[u8], pos: &mut usize) -> bool {
    if *pos + 1 < bytes.len()
        && bytes[*pos] == b'0'
        && (bytes[*pos + 1] == b'x' || bytes[*pos + 1] == b'X')
    {
        *pos += 2;
        let start = *pos;
        while *pos < bytes.len() && (bytes[*pos].is_ascii_hexdigit() || bytes[*pos] == b'_') {
            *pos += 1;
        }
        return *pos > start;
    }
    if *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
        while *pos < bytes.len() && (bytes[*pos].is_ascii_digit() || bytes[*pos] == b'_') {
            *pos += 1;
        }
        return true;
    }
    false
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn valid_int_expr(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut pos = 0usize;
    if pos < bytes.len() && bytes[pos] == b'(' {
        pos += 1;
    }
    if pos < bytes.len() && bytes[pos] == b'-' {
        pos += 1;
    }
    if !parse_num(bytes, &mut pos) {
        return false;
    }
    loop {
        let save = pos;
        while pos < bytes.len() && bytes[pos] == b' ' {
            pos += 1;
        }
        if pos < bytes.len() && (bytes[pos] == b'-' || bytes[pos] == b'+' || bytes[pos] == b'*') {
            pos += 1;
            while pos < bytes.len() && bytes[pos] == b' ' {
                pos += 1;
            }
            if pos < bytes.len() && bytes[pos] == b'(' {
                pos += 1;
            }
            if pos < bytes.len() && bytes[pos] == b'-' {
                pos += 1;
            }
            if !parse_num(bytes, &mut pos) {
                return false;
            }
            if pos < bytes.len() && bytes[pos] == b')' {
                pos += 1;
            }
        } else {
            pos = save;
            break;
        }
    }
    if pos < bytes.len() && bytes[pos] == b')' {
        pos += 1;
    }
    pos == bytes.len()
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn render_int_expr(expr: &syn::Expr) -> Result<String, String> {
    match expr {
        syn::Expr::Lit(lit) => {
            if let syn::Lit::Int(number) = &lit.lit {
                if !number.suffix().is_empty() {
                    return Err("integer literal carries a suffix".to_string());
                }
                Ok(lit_text(&lit.lit))
            } else {
                Err("expected an integer literal".to_string())
            }
        }
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => {
            Ok(format!("-{}", render_int_expr(&unary.expr)?))
        }
        syn::Expr::Binary(binary) => {
            let op = match binary.op {
                syn::BinOp::Add(_) => "+",
                syn::BinOp::Sub(_) => "-",
                syn::BinOp::Mul(_) => "*",
                _ => return Err("unsupported operator in integer expression".to_string()),
            };
            Ok(format!(
                "{} {op} {}",
                render_int_expr(&binary.left)?,
                render_int_expr(&binary.right)?
            ))
        }
        syn::Expr::Paren(paren) => Ok(format!("({})", render_int_expr(&paren.expr)?)),
        _ => Err("unsupported integer expression".to_string()),
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn str_content(lit: &syn::LitStr) -> Result<String, String> {
    let text = lit_text(&syn::Lit::Str(lit.clone()));
    if !text.starts_with('"') {
        return Err("raw string literal unsupported".to_string());
    }
    Ok(text[1..text.len() - 1].to_string())
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn resolve_struct_defaults(proj: &Projection, ps: &ProjStruct) -> Result<Vec<Default>, String> {
    let manual = proj.krate.model.default_impl(&ps.rust_name);
    let mut out = Vec::new();
    for field in &ps.fields {
        let value = match manual.and_then(|d| d.fields.iter().find(|(name, _)| name == &field.name))
        {
            Some((_, expr)) => eval_default(proj, expr, &field.ty, &field.name, 0)?,
            None => zero_default(proj, &field.ty)?,
        };
        out.push(value);
    }
    Ok(out)
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn struct_ctor(
    proj: &Projection,
    name: &str,
    overrides: Option<&[(String, syn::Expr)]>,
) -> Result<String, String> {
    let ps = proj.struct_by_name(name)?;
    let own = resolve_struct_defaults(proj, ps)?;
    let Some(overrides) = overrides else {
        return Ok(format!("{name}()"));
    };
    let mut override_ctor: Vec<(String, Default)> = Vec::new();
    for field in &ps.fields {
        if let Some((_, expr)) = overrides.iter().find(|(n, _)| n == &field.name) {
            override_ctor.push((
                field.name.clone(),
                eval_default(proj, expr, &field.ty, &field.name, 0)?,
            ));
        }
    }
    let all_equal = ps.fields.iter().enumerate().all(|(index, field)| {
        let ctor = override_ctor
            .iter()
            .find(|(n, _)| n == &field.name)
            .map(|(_, d)| d.ctor.clone())
            .unwrap_or_else(|| own[index].ctor.clone());
        ctor == own[index].ctor
    });
    if all_equal {
        return Ok(format!("{name}()"));
    }
    let mut args: Vec<String> = Vec::new();
    for (index, field) in ps.fields.iter().enumerate() {
        let ctor = override_ctor
            .iter()
            .find(|(n, _)| n == &field.name)
            .map(|(_, d)| d.ctor.clone())
            .unwrap_or_else(|| own[index].ctor.clone());
        args.push(format!("{}={ctor}", field.name));
    }
    Ok(format!("{name}({})", args.join(", ")))
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn zero_default(proj: &Projection, ty: &Ty) -> Result<Default, String> {
    match ty {
        Ty::Str | Ty::PathBuf => Ok(Default {
            ctor: "\"\"".to_string(),
            immutable: true,
        }),
        Ty::Bool => Ok(Default {
            ctor: "False".to_string(),
            immutable: true,
        }),
        Ty::U8
        | Ty::U16
        | Ty::U32
        | Ty::U64
        | Ty::U128
        | Ty::Usize
        | Ty::I8
        | Ty::I16
        | Ty::I32
        | Ty::I64
        | Ty::Isize => Ok(Default {
            ctor: "0".to_string(),
            immutable: true,
        }),
        Ty::F32 | Ty::F64 => Ok(Default {
            ctor: "0.0".to_string(),
            immutable: true,
        }),
        Ty::Option(_) => Ok(Default {
            ctor: "None".to_string(),
            immutable: true,
        }),
        Ty::VecOf(_) => Ok(Default {
            ctor: "[]".to_string(),
            immutable: false,
        }),
        Ty::ByteArray(len) => Ok(Default {
            ctor: format!("bytes({len})"),
            immutable: true,
        }),
        Ty::Tuple(items) => {
            let mut parts = Vec::new();
            let mut immutable = true;
            for item in items {
                let item_default = zero_default(proj, item)?;
                parts.push(item_default.ctor.clone());
                immutable &= item_default.immutable;
            }
            let mut inner = parts.join(", ");
            if items.len() == 1 {
                inner.push(',');
            }
            Ok(Default {
                ctor: format!("({inner})"),
                immutable,
            })
        }
        Ty::Custom(name) => {
            if let Some((_, _, default)) = proj.enum_literal(name) {
                return Ok(Default {
                    ctor: py_quote(default),
                    immutable: true,
                });
            }
            if proj.structs.iter().any(|s| s.rust_name == *name) {
                return Ok(Default {
                    ctor: struct_ctor(proj, name, None)?,
                    immutable: false,
                });
            }
            Err(format!(
                "{}: no zero default for type {name}",
                proj.krate.dir_name
            ))
        }
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn eval_default(
    proj: &Projection,
    expr: &syn::Expr,
    _ty: &Ty,
    where_: &str,
    depth: u8,
) -> Result<Default, String> {
    if depth > 8 {
        return Err(format!(
            "{}: default expr nesting too deep at {where_}",
            proj.krate.dir_name
        ));
    }
    match expr {
        syn::Expr::Path(path) if path.path.is_ident("None") => Ok(Default {
            ctor: "None".to_string(),
            immutable: true,
        }),
        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Bool(value) => Ok(Default {
                ctor: if value.value { "True" } else { "False" }.to_string(),
                immutable: true,
            }),
            syn::Lit::Int(_) => {
                let text = lit_text(&lit.lit);
                if !valid_int_lit(&text) {
                    return Err(format!(
                        "{}: unsupported default expr for {where_}: {text}",
                        proj.krate.dir_name
                    ));
                }
                Ok(Default {
                    ctor: text,
                    immutable: true,
                })
            }
            syn::Lit::Float(_) => {
                let text = lit_text(&lit.lit);
                if !valid_float_lit(&text) {
                    return Err(format!(
                        "{}: unsupported default expr for {where_}: {text}",
                        proj.krate.dir_name
                    ));
                }
                Ok(Default {
                    ctor: text,
                    immutable: true,
                })
            }
            syn::Lit::Str(value) => Ok(Default {
                ctor: py_quote(&str_content(value)?),
                immutable: true,
            }),
            _ => Err(format!(
                "{}: unsupported default expr for {where_}",
                proj.krate.dir_name
            )),
        },
        syn::Expr::MethodCall(call) => {
            let method = call.method.to_string();
            if !matches!(
                method.as_str(),
                "into" | "to_string" | "to_owned" | "to_lowercase"
            ) {
                return Err(format!(
                    "{}: unsupported default expr for {where_}",
                    proj.krate.dir_name
                ));
            }
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) = &*call.receiver
            {
                return Ok(Default {
                    ctor: py_quote(&str_content(value)?),
                    immutable: true,
                });
            }
            Err(format!(
                "{}: unsupported default expr for {where_}",
                proj.krate.dir_name
            ))
        }
        syn::Expr::Call(call) => {
            if let syn::Expr::Path(path) = &*call.func {
                let parts: Vec<String> = path
                    .path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect();
                if parts.as_slice() == ["String", "new"] && call.args.is_empty() {
                    return Ok(Default {
                        ctor: "\"\"".to_string(),
                        immutable: true,
                    });
                }
                if parts.as_slice() == ["Vec", "new"] && call.args.is_empty() {
                    return Ok(Default {
                        ctor: "[]".to_string(),
                        immutable: false,
                    });
                }
            }
            Err(format!(
                "{}: unsupported default expr for {where_}",
                proj.krate.dir_name
            ))
        }
        syn::Expr::Macro(mac) if mac.mac.path.is_ident("vec") => {
            let elems = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
                .parse2(mac.mac.tokens.clone())
                .map_err(|e| {
                    format!(
                        "{}: vec! macro does not parse for {where_}: {e}",
                        proj.krate.dir_name
                    )
                })?;
            let mut ctors = Vec::new();
            for elem in elems.iter() {
                ctors.push(eval_default(proj, elem, &Ty::Str, where_, depth + 1)?.ctor);
            }
            Ok(Default {
                ctor: format!("[{}]", ctors.join(", ")),
                immutable: false,
            })
        }
        syn::Expr::Path(path) => {
            let parts: Vec<String> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            match parts.as_slice() {
                [name] if is_const_name(name) => match proj.krate.model.const_def(name) {
                    Some(def) => {
                        let expr = def.expr.clone();
                        eval_default(proj, &expr, _ty, where_, depth + 1)
                    }
                    None => Err(format!(
                        "{}: unresolved const {name} in default for {where_}",
                        proj.krate.dir_name
                    )),
                },
                [enum_name, variant] => match proj.krate.enum_def(enum_name) {
                    Some(def) => {
                        if !def.variants.iter().any(|v| &v.name == variant) {
                            return Err(format!(
                                "{}: unknown variant {enum_name}::{variant}",
                                proj.krate.dir_name
                            ));
                        }
                        Ok(Default {
                            ctor: py_quote(&py_kebab(variant)),
                            immutable: true,
                        })
                    }
                    None => Err(format!(
                        "{}: unsupported default expr for {where_}",
                        proj.krate.dir_name
                    )),
                },
                _ => Err(format!(
                    "{}: unsupported default expr for {where_}",
                    proj.krate.dir_name
                )),
            }
        }
        syn::Expr::Struct(item) => {
            let struct_name = item
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
                .unwrap_or_default();
            if !proj.structs.iter().any(|s| s.rust_name == struct_name) {
                return Err(format!(
                    "{}: unsupported default expr for {where_}",
                    proj.krate.dir_name
                ));
            }
            let mut overrides: Vec<(String, syn::Expr)> = Vec::new();
            for field in &item.fields {
                if let syn::Member::Named(ident) = &field.member {
                    overrides.push((ident.to_string(), field.expr.clone()));
                }
            }
            Ok(Default {
                ctor: struct_ctor(proj, &struct_name, Some(&overrides))?,
                immutable: false,
            })
        }
        syn::Expr::Unary(_) | syn::Expr::Binary(_) | syn::Expr::Paren(_) => {
            let text = render_int_expr(expr)?;
            if !valid_int_expr(&text) {
                return Err(format!(
                    "{}: unsupported default expr for {where_}: {text}",
                    proj.krate.dir_name
                ));
            }
            Ok(Default {
                ctor: text,
                immutable: true,
            })
        }
        _ => Err(format!(
            "{}: unsupported default expr for {where_}",
            proj.krate.dir_name
        )),
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn py_type(proj: &Projection, ty: &Ty) -> Result<String, String> {
    match ty {
        Ty::Str | Ty::PathBuf => Ok("str".to_string()),
        Ty::Bool => Ok("bool".to_string()),
        Ty::U8
        | Ty::U16
        | Ty::U32
        | Ty::U64
        | Ty::U128
        | Ty::Usize
        | Ty::I8
        | Ty::I16
        | Ty::I32
        | Ty::I64
        | Ty::Isize => Ok("int".to_string()),
        Ty::F32 | Ty::F64 => Ok("float".to_string()),
        Ty::Option(inner) => Ok(format!("{} | None", py_type(proj, inner)?)),
        Ty::VecOf(inner) => Ok(format!("list[{}]", py_type(proj, inner)?)),
        Ty::Tuple(items) => {
            let parts: Vec<String> = items
                .iter()
                .map(|item| py_type(proj, item))
                .collect::<Result<_, _>>()?;
            Ok(format!("tuple[{}]", parts.join(", ")))
        }
        Ty::ByteArray(_) => Ok("bytes".to_string()),
        Ty::Custom(name) => {
            if let Some((_, lits, _)) = proj.enum_literal(name) {
                let quoted: Vec<String> = lits.iter().map(|lit| py_quote(lit)).collect();
                Ok(format!("Literal[{}]", quoted.join(", ")))
            } else {
                Ok(name.clone())
            }
        }
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
pub fn emit_py_module(proj: &Projection) -> Result<String, String> {
    let krate = &proj.krate;
    let aliases = legacy_aliases(&krate.module_name);
    let legacy = legacy_fields(&krate.module_name);
    let mut needs_factory = false;
    for ps in &proj.structs {
        let defaults = resolve_struct_defaults(proj, ps)?;
        if defaults.iter().any(|d| !d.immutable) {
            needs_factory = true;
        }
    }
    let uses_literal = !proj.enum_literals.is_empty();
    let uses_callable = legacy
        .iter()
        .any(|(_, fields)| fields.iter().any(|lf| lf.1.contains("Callable")));
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("# see: {}", krate.see_ref));
    lines.push("from __future__ import annotations".to_string());
    lines.push(String::new());
    if uses_callable {
        lines.push("from collections.abc import Callable".to_string());
    }
    let mut dc_import = "from dataclasses import dataclass".to_string();
    if needs_factory {
        dc_import.push_str(", field");
    }
    lines.push(dc_import);
    if uses_literal {
        lines.push("from typing import Literal".to_string());
    }
    lines.push(String::new());
    lines.push("from kelir.model_plugin import BasePlugin".to_string());
    lines.push(String::new());
    let mut public: Vec<String> = proj.structs.iter().map(|ps| ps.rust_name.clone()).collect();
    public.push(krate.plugin_class.clone());
    public.extend(aliases.iter().map(|(_, alias)| alias.to_string()));
    lines.push("__all__ = [".to_string());
    let mut sorted_public = public.clone();
    sorted_public.sort();
    for name in &sorted_public {
        lines.push(format!("    \"{name}\","));
    }
    lines.push("]".to_string());
    for ps in &proj.structs {
        lines.push(String::new());
        lines.push(String::new());
        lines.push("@dataclass".to_string());
        lines.push(format!("class {}:", ps.rust_name));
        let defaults = resolve_struct_defaults(proj, ps)?;
        let fields_legacy: Vec<(&str, &str, &str, &str)> = legacy
            .iter()
            .find(|(name, _)| name == &ps.rust_name)
            .map(|(_, fields)| fields.clone())
            .unwrap_or_default();
        if ps.fields.is_empty() && fields_legacy.is_empty() {
            lines.push("    pass".to_string());
            continue;
        }
        for (field, d) in ps.fields.iter().zip(defaults.iter()) {
            let ftype = py_type(proj, &field.ty)?;
            if d.immutable {
                lines.push(format!("    {}: {} = {}", field.name, ftype, d.ctor));
            } else if d.ctor == "[]" {
                lines.push(format!(
                    "    {}: {} = field(default_factory=list)",
                    field.name, ftype
                ));
            } else {
                lines.push(format!(
                    "    {}: {} = field(default_factory=lambda: {})",
                    field.name, ftype, d.ctor
                ));
            }
        }
        for lf in &fields_legacy {
            lines.push(format!("    # {}", lf.3));
            lines.push(format!("    {}: {} = {}", lf.0, lf.1, lf.2));
        }
    }
    let mut sorted_aliases = aliases.clone();
    sorted_aliases.sort();
    for (rust_name, alias) in &sorted_aliases {
        lines.push(String::new());
        lines.push(String::new());
        lines.push(format!("{alias} = {rust_name}"));
    }
    lines.push(String::new());
    lines.push(String::new());
    lines.push(format!("class {}(BasePlugin):", krate.plugin_class));
    lines.push(format!("    name = \"{}\"", krate.plugin_class_name));
    lines.push(String::new());
    lines.push(format!(
        "    def __init__(self, config: {} | None = None) -> None:",
        proj.root.rust_name
    ));
    lines.push(format!(
        "        self.config = config if config is not None else {}()",
        proj.root.rust_name
    ));
    lines.push(String::new());
    Ok(lines.join("\n"))
}

// see: cans/03-sdk-architecture.md#Python-SDK
const PY_BARREL_SEE: &str = "# see: cans/03-sdk-architecture.md#Python-SDK
";

// see: cans/03-sdk-architecture.md#Python-SDK
pub fn emit_py_barrel(projections: &[Projection]) -> String {
    let mut lines: Vec<String> = vec![PY_BARREL_SEE.trim_end().to_string()];
    let mut ordered: Vec<&Projection> = projections.iter().collect();
    ordered.sort_by_key(|proj| proj.krate.module_name.clone());
    for proj in &ordered {
        lines.push(format!(
            "from kelir.plugins.plugin_{} import {} as {}",
            proj.krate.module_name, proj.root.rust_name, proj.root.rust_name
        ));
        lines.push(format!(
            "from kelir.plugins.plugin_{} import {} as {}",
            proj.krate.module_name, proj.krate.plugin_class, proj.krate.plugin_class
        ));
    }
    lines.push(String::new());
    let mut names: Vec<String> = projections
        .iter()
        .flat_map(|proj| [proj.root.rust_name.clone(), proj.krate.plugin_class.clone()])
        .collect();
    names.sort();
    lines.push("__all__ = [".to_string());
    for name in &names {
        lines.push(format!("    \"{name}\","));
    }
    lines.push("]".to_string());
    lines.push(String::new());
    lines.join("\n")
}
