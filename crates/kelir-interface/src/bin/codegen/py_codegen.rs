// see: cans/03-sdk-architecture.md#Python-SDK
use std::fmt::Write as _;

use kelir_interface::codec_protocol::{
    FLAG_CLOSE, FLAG_DATA, FLAG_OPEN, FLAG_RST, FLAG_WINDOW_UPDATE, MAX_CONTROL_PAYLOAD,
    MAX_FRAME_PAYLOAD, MAX_STREAM_ID,
};
use kelir_interface::protocol_wire::{CHANNELS, EXEC_STREAMS, MSG_TYPES, STREAM_KINDS};

use crate::model_codegen::{EnumDef, Model, Ty};
use crate::naming_codegen::{camel, kebab, py_sdk_name, screaming};

pub const PY_SEE: &str = "# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/15-operations.md#Multi-language-parity-gate
# see: cans/10-attestation.md#Execution-receipts
";
const PROTOCOL_PY_SEE: &str = "# see: cans/11-protocol.md#Wire-framing
# see: cans/11-protocol.md#Control-channel
# see: cans/11-protocol.md#Binary-exec-channel
";

// see: cans/03-sdk-architecture.md#Python-SDK
fn py_type(ty: &Ty) -> String {
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
        | Ty::Isize => "int".to_string(),
        Ty::F32 | Ty::F64 => "float".to_string(),
        Ty::Str | Ty::PathBuf => "str".to_string(),
        Ty::Bool => "bool".to_string(),
        Ty::Option(inner) => format!("{} | None", py_type(inner)),
        Ty::Tuple(items) => format!(
            "tuple[{}]",
            items.iter().map(py_type).collect::<Vec<_>>().join(", ")
        ),
        Ty::VecOf(inner) => match &**inner {
            Ty::U8 => "bytes".to_string(),
            Ty::Str | Ty::PathBuf => "list[str]".to_string(),
            Ty::U32 => "list[int]".to_string(),
            Ty::Custom(name) => format!("list[{}]", py_sdk_name(name)),
            other => format!("list[{}]", py_type(other)),
        },
        Ty::ByteArray(_) => "str".to_string(),
        Ty::Custom(name) => py_sdk_name(name),
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn py_default(ty: &Ty) -> String {
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
        | Ty::Isize => "0".to_string(),
        Ty::F32 | Ty::F64 => "0.0".to_string(),
        Ty::Str | Ty::ByteArray(_) | Ty::PathBuf => "\"\"".to_string(),
        Ty::Bool => "False".to_string(),
        Ty::Option(_) => "None".to_string(),
        Ty::Tuple(_) => "()".to_string(),
        Ty::VecOf(inner) => {
            if matches!(**inner, Ty::U8) {
                "b\"\"".to_string()
            } else {
                "Field(default_factory=list)".to_string()
            }
        }
        Ty::Custom(name) if name == "ExitKind" => {
            "Field(default_factory=ExitKindNormal)".to_string()
        }
        Ty::Custom(name) => format!("Field(default_factory={})", py_sdk_name(name)),
    }
}

// see: cans/10-attestation.md#Execution-receipts
fn emit_exit_kind_union(def: &EnumDef) -> Result<String, String> {
    let mut out = String::new();
    let mut union = Vec::new();
    for variant in &def.variants {
        let tag = kebab(&variant.name);
        let class = format!("ExitKind{}", variant.name);
        let _ = writeln!(out, "class {class}(BaseModel):");
        let _ = writeln!(out, "    kind: Literal[\"{tag}\"] = \"{tag}\"");
        match &variant.payload {
            None => {}
            Some(Ty::U32) => out.push_str("    signal: int = 0\n"),
            Some(Ty::Str) => out.push_str("    message: str = \"\"\n"),
            Some(other) => {
                return Err(format!("ExitKind::{}: {other:?}", variant.name));
            }
        }
        out.push_str("\n\n");
        union.push(class);
    }
    out.push_str("ExitKind = (\n");
    for (index, class) in union.iter().enumerate() {
        if index == 0 {
            let _ = writeln!(out, "    {class}");
        } else {
            let _ = writeln!(out, "    | {class}");
        }
    }
    out.push_str(")\n\n");
    Ok(out)
}

// see: cans/10-attestation.md#Execution-receipts
fn emit_native_exit_kind(def: &EnumDef) -> Result<String, String> {
    let native_tag = |name: &str| -> String {
        if def.serde_camel {
            camel(name)
        } else {
            name.to_string()
        }
    };
    let mut out = String::from("NativeExitKindRecord = str | dict[str, int | str]\n\n\n");
    let _ = writeln!(
        out,
        "def from_native_exit_kind(raw: NativeExitKindRecord) -> ExitKind:"
    );
    out.push_str("    if isinstance(raw, str):\n");
    let mut payload_variants = Vec::new();
    for variant in &def.variants {
        if variant.payload.is_none() {
            let tag = native_tag(&variant.name);
            let class = format!("ExitKind{}", variant.name);
            let _ = writeln!(out, "        if raw == \"{tag}\":");
            let _ = writeln!(out, "            return {class}()");
        } else {
            payload_variants.push(variant);
        }
    }
    out.push_str("        raise ValueError(f\"unknown native exit kind tag: {raw}\")\n");
    for (index, variant) in payload_variants.iter().enumerate() {
        let tag = native_tag(&variant.name);
        let class = format!("ExitKind{}", variant.name);
        let (payload_name, payload_py) = match variant.payload.as_ref() {
            Some(Ty::U32) => ("signal", "int"),
            _ => ("message", "str"),
        };
        if index + 1 == payload_variants.len() {
            let _ = writeln!(
                out,
                "    return {class}({payload_name}=cast({payload_py}, raw[\"{tag}\"]))"
            );
        } else {
            let _ = writeln!(out, "    if \"{tag}\" in raw:");
            let _ = writeln!(
                out,
                "        return {class}({payload_name}=cast({payload_py}, raw[\"{tag}\"]))"
            );
        }
    }
    out.push_str("\n\n");
    Ok(out)
}

// see: cans/03-sdk-architecture.md#Python-SDK
pub fn emit_telemetry(model: &Model, order: &[&str]) -> Result<String, String> {
    let mut out = String::new();
    out.push_str(PY_SEE);
    out.push_str("from __future__ import annotations\n\n");
    out.push_str("from typing import Literal, cast\n\n");
    out.push_str("from pydantic import BaseModel, Field\n\n");
    for name in order {
        if *name == "ExitKind" {
            let def = model.enum_def(name)?;
            out.push_str(&emit_exit_kind_union(def)?);
            out.push_str(&emit_native_exit_kind(def)?);
            continue;
        }
        let def = model.struct_def(name)?;
        if *name == "ExecutionReceipt" {
            out.push_str("# TODO (L3): no FFI producer populates ExecutionReceipt.exit_kind yet\n");
        }
        let _ = writeln!(out, "class {}(BaseModel):", py_sdk_name(name));
        for field in &def.fields {
            let _ = writeln!(
                out,
                "    {}: {} = {}",
                field.name,
                py_type(&field.ty),
                py_default(&field.ty)
            );
        }
        out.push_str("\n\n");
    }
    Ok(out)
}

// see: cans/11-protocol.md#Wire-framing
pub fn emit_protocol() -> String {
    let mut out = String::new();
    out.push_str(PROTOCOL_PY_SEE);
    out.push_str("from enum import IntEnum\n\n");
    let _ = writeln!(out, "class Channel(IntEnum):");
    for (name, value) in CHANNELS {
        let _ = writeln!(out, "    {} = {value:#06x}", screaming(name));
    }
    out.push('\n');
    let _ = writeln!(out, "class MsgType(IntEnum):");
    for (name, value) in MSG_TYPES {
        let _ = writeln!(out, "    {} = {value:#06x}", screaming(name));
    }
    out.push('\n');
    let _ = writeln!(out, "class StreamKind(IntEnum):");
    for (name, value) in STREAM_KINDS {
        let _ = writeln!(out, "    {} = {value:#06x}", screaming(name));
    }
    out.push('\n');
    let _ = writeln!(out, "class ExecStream(IntEnum):");
    for (name, value) in EXEC_STREAMS {
        let short = name.strip_prefix("EXEC_STREAM_").unwrap_or(name);
        let _ = writeln!(out, "    {short} = {value:#04x}", short = short);
    }
    out.push('\n');
    for (name, value) in [
        ("FLAG_OPEN", FLAG_OPEN),
        ("FLAG_DATA", FLAG_DATA),
        ("FLAG_CLOSE", FLAG_CLOSE),
        ("FLAG_RST", FLAG_RST),
        ("FLAG_WINDOW_UPDATE", FLAG_WINDOW_UPDATE),
    ] {
        let _ = writeln!(out, "{name} = {value:#04x}");
    }
    let _ = writeln!(out, "MAX_STREAM_ID = {MAX_STREAM_ID:#x}");
    let _ = writeln!(out, "MAX_FRAME_PAYLOAD = {MAX_FRAME_PAYLOAD}");
    let _ = writeln!(out, "MAX_CONTROL_PAYLOAD = {MAX_CONTROL_PAYLOAD}");
    out
}

const PARITY_PY_SEE: &str = "# see: cans/15-operations.md#Multi-language-parity-gate
";
const FUSION_PY_SEE: &str = "# see: cans/02-plugin-model.md#Segregated-ring-traits
# see: cans/02-plugin-model.md#Foreign-plugin-proxy
";

// see: cans/15-operations.md#Multi-language-parity-gate
pub fn emit_package_init(order: &[&str]) -> String {
    let mut out = String::from(FUSION_PY_SEE);
    out.push_str(PARITY_PY_SEE);
    out.push_str("from kelir.generated.fusion_tables import (\n");
    out.push_str("    HOOK_RINGS,\n");
    out.push_str("    LIFECYCLE_TAGS,\n");
    out.push_str("    HookStage,\n");
    out.push_str("    LifecycleTag,\n");
    out.push_str("    RingEvent,\n");
    out.push_str(")\n");
    out.push_str(
        "from kelir.generated.wire_protocol import Channel, ExecStream, MsgType, StreamKind\n",
    );
    out.push_str("from kelir.generated.attest_telemetry import (\n");
    for name in order {
        let _ = writeln!(out, "    {},", py_sdk_name(name));
    }
    out.push_str(")\n");
    out
}
