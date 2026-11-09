// see: cans/03-sdk-architecture.md#Python-SDK
use std::fmt::Write as _;

use kelir_interface::codec::{
    FLAG_CLOSE, FLAG_DATA, FLAG_OPEN, FLAG_RST, FLAG_WINDOW_UPDATE, MAX_CONTROL_PAYLOAD,
    MAX_FRAME_PAYLOAD, MAX_STREAM_ID,
};
use kelir_interface::protocol::{CHANNELS, EXEC_STREAMS, MSG_TYPES, STREAM_KINDS};

use crate::model::{Model, Ty};
use crate::naming::{py_sdk_name, screaming};

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
        Ty::Custom(name) => format!("Field(default_factory={})", py_sdk_name(name)),
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
pub fn emit_telemetry(model: &Model, order: &[&str]) -> Result<String, String> {
    let mut out = String::new();
    out.push_str(PY_SEE);
    out.push_str("from __future__ import annotations\n\n");
    out.push_str("from pydantic import BaseModel, Field\n\n");
    for name in order {
        if *name == "ExitKind" {
            let def = model.enum_def(name)?;
            let _ = writeln!(out, "class ExitKind(BaseModel):");
            out.push_str("    kind: str = \"normal\"\n");
            let has_signal = def
                .variants
                .iter()
                .any(|v| matches!(v.payload, Some(Ty::U32)));
            let has_message = def
                .variants
                .iter()
                .any(|v| matches!(v.payload, Some(Ty::Str)));
            if has_signal {
                out.push_str("    signal: int | None = None\n");
            }
            if has_message {
                out.push_str("    message: str | None = None\n");
            }
            out.push_str("\n\n");
            continue;
        }
        let def = model.struct_def(name)?;
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
