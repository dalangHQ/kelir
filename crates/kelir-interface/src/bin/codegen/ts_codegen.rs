// see: cans/03-sdk-architecture.md#TypeScript-SDK
use std::fmt::Write as _;

use kelir_interface::codec_protocol::{
    FLAG_CLOSE, FLAG_DATA, FLAG_OPEN, FLAG_RST, FLAG_WINDOW_UPDATE, MAX_CONTROL_PAYLOAD,
    MAX_FRAME_PAYLOAD, MAX_STREAM_ID,
};
use kelir_interface::protocol_wire::{CHANNELS, EXEC_STREAMS, MSG_TYPES, STREAM_KINDS};

use crate::model_codegen::{EnumDef, Model, StructDef, Ty};
use crate::naming_codegen::{camel, kebab, screaming, ts_native_record_name, ts_sdk_name};

pub const TS_SEE: &str = "// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/15-operations.md#Multi-language-parity-gate
// see: cans/10-attestation.md#Execution-receipts
";
const PROTOCOL_TS_SEE: &str = "// see: cans/11-protocol.md#Wire-framing
// see: cans/11-protocol.md#Control-channel
// see: cans/11-protocol.md#Binary-exec-channel
";

// see: cans/03-sdk-architecture.md#TypeScript-SDK
fn ts_type(ty: &Ty) -> String {
    match ty {
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
        | Ty::F64 => "number".to_string(),
        Ty::Str | Ty::PathBuf => "string".to_string(),
        Ty::Bool => "boolean".to_string(),
        Ty::Option(inner) => format!("{} | null", ts_type(inner)),
        Ty::Tuple(items) => format!(
            "[{}]",
            items.iter().map(ts_type).collect::<Vec<_>>().join(", ")
        ),
        Ty::VecOf(inner) => match &**inner {
            Ty::U8 => "Uint8Array".to_string(),
            Ty::Str | Ty::PathBuf => "readonly string[]".to_string(),
            Ty::U32 => "readonly number[]".to_string(),
            Ty::Custom(name) => format!("readonly {}[]", ts_sdk_name(name)),
            other => format!("readonly {}[]", ts_type(other)),
        },
        Ty::ByteArray(_) => "string".to_string(),
        Ty::Custom(name) => ts_sdk_name(name),
    }
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
fn ts_native_type(ty: &Ty) -> String {
    match ty {
        Ty::Custom(name) => ts_native_record_name(name),
        Ty::Option(inner) => format!("{} | null", ts_native_type(inner)),
        other => ts_type(other),
    }
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
fn emit_struct_interface(def: &StructDef, name: &str, native: bool) -> String {
    let mut out = format!("export interface {name} {{\n");
    for field in &def.fields {
        let field_name = if native && !def.serde_camel {
            field.name.clone()
        } else {
            camel(&field.name)
        };
        let ty = if native {
            ts_native_type(&field.ty)
        } else {
            ts_type(&field.ty)
        };
        if let Ty::Option(inner) = &field.ty {
            let inner_ty = if native {
                ts_native_type(inner)
            } else {
                ts_type(inner)
            };
            let _ = writeln!(out, "  readonly {field_name}?: {inner_ty} | null;");
        } else {
            let _ = writeln!(out, "  readonly {field_name}: {ty};");
        }
    }
    out.push_str("}\n\n");
    out
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
fn ts_convert_expr(ty: &Ty, native_field: &str) -> String {
    match ty {
        Ty::Option(inner) => match &**inner {
            Ty::Custom(name) if name != "TelemetryExtension" => {
                format!("raw.{native_field} ? fromNative{name}(raw.{native_field}) : null")
            }
            _ => format!("raw.{native_field} ?? null"),
        },
        Ty::Custom(name) if name == "ExitKind" => {
            format!("fromNativeExitKind(raw.{native_field})")
        }
        _ => format!("raw.{native_field}"),
    }
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
fn emit_exit_kind(def: &EnumDef) -> Result<String, String> {
    let mut out = String::from("export type ExitKind =\n");
    let mut lines = Vec::new();
    for variant in &def.variants {
        let tag = kebab(&variant.name);
        match &variant.payload {
            None => lines.push(format!("  | {{ readonly kind: \"{tag}\" }}")),
            Some(Ty::U32) => lines.push(format!(
                "  | {{ readonly kind: \"{tag}\"; readonly signal: number }}"
            )),
            Some(Ty::Str) => lines.push(format!(
                "  | {{ readonly kind: \"{tag}\"; readonly message: string }}"
            )),
            Some(other) => {
                return Err(format!("ExitKind::{}: {other:?}", variant.name));
            }
        };
    }
    out.push_str(&lines.join("\n"));
    out.push_str(";\n\n");
    Ok(out)
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
fn emit_native_exit_kind(def: &EnumDef) -> Result<String, String> {
    let native_tag = |name: &str| -> String {
        if def.serde_camel {
            camel(name)
        } else {
            name.to_string()
        }
    };
    let mut out = String::from("export type NativeExitKindRecord =\n");
    let mut lines = Vec::new();
    for variant in &def.variants {
        let tag = native_tag(&variant.name);
        match &variant.payload {
            None => lines.push(format!("  | \"{tag}\"")),
            Some(Ty::U32) => lines.push(format!("  | {{ readonly {tag}: number }}")),
            Some(Ty::Str) => lines.push(format!("  | {{ readonly {tag}: string }}")),
            Some(other) => {
                return Err(format!("ExitKind::{}: {other:?}", variant.name));
            }
        }
    }
    out.push_str(&lines.join("\n"));
    out.push_str(";\n\n");
    let _ = writeln!(
        out,
        "export function fromNativeExitKind(raw: NativeExitKindRecord): ExitKind {{"
    );
    let mut payload_variants = Vec::new();
    for variant in &def.variants {
        match &variant.payload {
            None => {
                let tag = native_tag(&variant.name);
                let kind = kebab(&variant.name);
                let _ = writeln!(
                    out,
                    "  if (raw === \"{tag}\") return {{ kind: \"{kind}\" }};"
                );
            }
            Some(_) => payload_variants.push(variant),
        }
    }
    for (index, variant) in payload_variants.iter().enumerate() {
        let tag = native_tag(&variant.name);
        let kind = kebab(&variant.name);
        let payload_name = match variant.payload.as_ref() {
            Some(Ty::U32) => "signal",
            _ => "message",
        };
        if index + 1 == payload_variants.len() {
            let _ = writeln!(
                out,
                "  return {{ kind: \"{kind}\", {payload_name}: raw.{tag} }};"
            );
        } else {
            let _ = writeln!(
                out,
                "  if (typeof raw === \"object\" && \"{tag}\" in raw) {{"
            );
            let _ = writeln!(
                out,
                "    return {{ kind: \"{kind}\", {payload_name}: raw.{tag} }};"
            );
            out.push_str("  }\n");
        }
    }
    out.push_str("}\n\n");
    Ok(out)
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub fn emit_telemetry(
    model: &Model,
    order: &[&str],
    native_skip: &[&str],
) -> Result<String, String> {
    let mut out = String::new();
    out.push_str(TS_SEE);
    out.push('\n');
    for name in order {
        if *name == "ExitKind" {
            let def = model.enum_def(name)?;
            out.push_str(&emit_exit_kind(def)?);
            continue;
        }
        let def = model.struct_def(name)?;
        out.push_str(&emit_struct_interface(def, &ts_sdk_name(name), false));
    }
    for name in order {
        if *name == "ExitKind" {
            let def = model.enum_def(name)?;
            out.push_str(&emit_native_exit_kind(def)?);
            continue;
        }
        if native_skip.contains(name) {
            continue;
        }
        let def = model.struct_def(name)?;
        out.push_str(&emit_struct_interface(
            def,
            &ts_native_record_name(name),
            true,
        ));
    }
    for name in order {
        if *name == "ExitKind" || native_skip.contains(name) {
            continue;
        }
        let def = model.struct_def(name)?;
        let sdk_name = ts_sdk_name(name);
        let native_name = ts_native_record_name(name);
        let _ = writeln!(
            out,
            "export function fromNative{name}(raw: {native_name}): {sdk_name} {{"
        );
        out.push_str("  return {\n");
        for field in &def.fields {
            let native_field = if def.serde_camel {
                camel(&field.name)
            } else {
                field.name.clone()
            };
            let _ = writeln!(
                out,
                "    {}: {},",
                camel(&field.name),
                ts_convert_expr(&field.ty, &native_field)
            );
        }
        out.push_str("  };\n}\n\n");
    }
    Ok(out)
}

// see: cans/11-protocol.md#Wire-framing
pub fn emit_protocol() -> String {
    let mut out = String::new();
    out.push_str(PROTOCOL_TS_SEE);
    out.push('\n');
    for (name, value) in CHANNELS {
        let _ = writeln!(
            out,
            "export const CHANNEL_{} = {value:#06x};",
            screaming(name)
        );
    }
    out.push('\n');
    for (name, value) in MSG_TYPES {
        let _ = writeln!(
            out,
            "export const MSG_TYPE_{} = {value:#06x};",
            screaming(name)
        );
    }
    out.push('\n');
    for (name, value) in STREAM_KINDS {
        let _ = writeln!(
            out,
            "export const STREAM_KIND_{} = {value:#06x};",
            screaming(name)
        );
    }
    out.push('\n');
    for (name, value) in EXEC_STREAMS {
        let _ = writeln!(out, "export const {name} = {value:#04x};");
    }
    out.push('\n');
    for (name, value) in [
        ("FLAG_OPEN", FLAG_OPEN),
        ("FLAG_DATA", FLAG_DATA),
        ("FLAG_CLOSE", FLAG_CLOSE),
        ("FLAG_RST", FLAG_RST),
        ("FLAG_WINDOW_UPDATE", FLAG_WINDOW_UPDATE),
    ] {
        let _ = writeln!(out, "export const {name} = {value:#04x};");
    }
    out.push('\n');
    let _ = writeln!(out, "export const MAX_STREAM_ID = {MAX_STREAM_ID:#x};");
    let _ = writeln!(out, "export const MAX_FRAME_PAYLOAD = {MAX_FRAME_PAYLOAD};");
    let _ = writeln!(
        out,
        "export const MAX_CONTROL_PAYLOAD = {MAX_CONTROL_PAYLOAD};"
    );
    out
}
