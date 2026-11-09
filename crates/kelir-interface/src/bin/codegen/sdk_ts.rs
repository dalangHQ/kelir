// see: cans/02-plugin-model.md#Auto-generated-manifest-contract
// see: cans/03-sdk-architecture.md#TypeScript-SDK
use crate::model::Ty;
use crate::sdk::{Projection, sdk_camel};

const TS_HEADER: &str = "// see: cans/03-sdk-architecture.md#TypeScript-SDK
";
const SERVICE_LOOKUP_SEE: &str =
    "// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
";

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
fn context_handle(plugin_name: &str) -> Option<(&'static str, &'static str)> {
    match plugin_name {
        "chaos" => Some(("chaos", "ChaosHandle")),
        "ledger" => Some(("ledger", "LedgerHandle")),
        "mesh" => Some(("mesh", "MeshHandle")),
        _ => None,
    }
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
fn ts_type(proj: &Projection, ty: &Ty) -> Result<String, String> {
    match ty {
        Ty::Str | Ty::PathBuf => Ok("string".to_string()),
        Ty::Bool => Ok("boolean".to_string()),
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
        | Ty::Isize
        | Ty::F32
        | Ty::F64 => Ok("number".to_string()),
        Ty::Option(inner) => ts_type(proj, inner),
        Ty::VecOf(inner) => Ok(wrap_ts_array(&ts_type(proj, inner)?)),
        Ty::Tuple(items) => {
            let parts: Vec<String> = items
                .iter()
                .map(|item| ts_type(proj, item))
                .collect::<Result<_, _>>()?;
            Ok(format!("[{}]", parts.join(", ")))
        }
        Ty::ByteArray(_) => Ok("Uint8Array".to_string()),
        Ty::Custom(name) => {
            if let Some((_, lits, _)) = proj.enum_literal(name) {
                let quoted: Vec<String> = lits.iter().map(|lit| format!("\"{lit}\"")).collect();
                Ok(quoted.join(" | "))
            } else {
                Ok(name.clone())
            }
        }
    }
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
fn wrap_ts_array(inner: &str) -> String {
    if !inner.is_empty()
        && inner
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
    {
        format!("readonly {inner}[]")
    } else {
        format!("readonly ({inner})[]")
    }
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
fn emit_ts_group(proj: &Projection) -> Result<Vec<String>, String> {
    let mut lines: Vec<String> = Vec::new();
    for ps in &proj.structs {
        if ps.rust_name == proj.root.rust_name {
            continue;
        }
        lines.push(format!("export interface {} {{", ps.rust_name));
        for field in &ps.fields {
            lines.push(format!(
                "  readonly {}?: {};",
                sdk_camel(&field.name),
                ts_type(proj, &field.ty)?
            ));
        }
        lines.push("}".to_string());
        lines.push(String::new());
        lines.push(String::new());
    }
    lines.push(format!(
        "export interface {} extends PluginEnablement {{",
        proj.root.rust_name
    ));
    for field in &proj.root.fields {
        lines.push(format!(
            "  readonly {}?: {};",
            sdk_camel(&field.name),
            ts_type(proj, &field.ty)?
        ));
    }
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push(String::new());
    let factory = sdk_camel(&proj.krate.plugin_name);
    match context_handle(&proj.krate.plugin_name) {
        None => lines.push(format!(
            "export function {factory}(config: {}): KelirPlugin {{",
            proj.root.rust_name
        )),
        Some((context_key, handle_type)) => {
            lines.push(SERVICE_LOOKUP_SEE.trim_end().to_string());
            lines.push(format!("export function {factory}("));
            lines.push(format!("  config: {},", proj.root.rust_name));
            lines.push(format!(
                "): KelirPlugin & PluginContext<{{ {context_key}: {handle_type} }}> {{"
            ));
        }
    }
    lines.push(format!(
        "  return {{ name: \"{}\", enabled: config.enabled, config }};",
        proj.krate.plugin_class_name
    ));
    lines.push("}".to_string());
    Ok(lines)
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub fn emit_ts(projections: &[Projection]) -> Result<String, String> {
    let handles: Vec<(&str, &str)> = projections
        .iter()
        .filter_map(|proj| context_handle(&proj.krate.plugin_name))
        .collect();
    let mut lines: Vec<String> = vec![TS_HEADER.trim_end().to_string(), String::new()];
    if !handles.is_empty() {
        let handle_types: Vec<&str> = handles.iter().map(|(_, handle)| *handle).collect();
        lines.push(format!(
            "import type {{ {} }} from \"./context.lifecycle.types.js\";",
            handle_types.join(", ")
        ));
        lines.push(
            "import type { KelirPlugin, PluginContext } from \"./plugin.model.js\";".to_string(),
        );
    } else {
        lines.push("import type { KelirPlugin } from \"./plugin.model.js\";".to_string());
    }
    lines.push(String::new());
    lines.push("export interface PluginEnablement {".to_string());
    lines.push("  readonly enabled?: boolean;".to_string());
    lines.push("}".to_string());
    for proj in projections {
        lines.push(String::new());
        lines.push(String::new());
        lines.extend(emit_ts_group(proj)?);
    }
    lines.push(String::new());
    Ok(lines.join("\n"))
}
