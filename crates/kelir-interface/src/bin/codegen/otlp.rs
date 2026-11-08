// see: cans/14-observability.md#Metrics-pipeline
use std::fmt::Write as _;

use crate::model::{Model, Ty, rust_type};

const RECORDS_SEE: &str = "// see: cans/14-observability.md#Metrics-pipeline
";

// see: cans/14-observability.md#Metrics-pipeline
type RenderedField = (String, String, String);

// see: cans/14-observability.md#Metrics-pipeline
struct OtlpFieldSpec {
    target: &'static str,
    source: Option<&'static str>,
    target_type: Option<&'static str>,
    init: Option<&'static str>,
}

// see: cans/14-observability.md#Metrics-pipeline
struct OtlpRecordSpec {
    struct_name: &'static str,
    source_struct: &'static str,
    constructor: &'static str,
    derives: &'static str,
    fields: &'static [OtlpFieldSpec],
    ignored: &'static [&'static str],
}

const OTLP_SPECS: &[OtlpRecordSpec] = &[
    OtlpRecordSpec {
        struct_name: "OtlpReceiptRecord",
        source_struct: "ExecutionReceipt",
        constructor: "from_receipt",
        derives: "Debug, Clone, PartialEq, Eq",
        fields: &[
            OtlpFieldSpec {
                target: "pid",
                source: Some("pid"),
                target_type: None,
                init: None,
            },
            OtlpFieldSpec {
                target: "binary_path",
                source: Some("binary_path"),
                target_type: None,
                init: None,
            },
            OtlpFieldSpec {
                target: "binary_blake3",
                source: Some("binary_blake3"),
                target_type: None,
                init: None,
            },
            OtlpFieldSpec {
                target: "exit_code",
                source: Some("exit_code"),
                target_type: None,
                init: None,
            },
            OtlpFieldSpec {
                target: "wall_clock_ns",
                source: Some("wall_clock_ns"),
                target_type: None,
                init: None,
            },
            OtlpFieldSpec {
                target: "spans",
                source: None,
                target_type: Some("Vec<crate::OtlpSpan>"),
                init: Some("crate::phase_spans(receipt.phase_timings.as_ref())"),
            },
        ],
        ignored: &[
            "exit_kind",
            "correlation_token",
            "dirty_bytes",
            "syscall_count",
            "phase_timings",
            "egress_bytes",
            "egress_dropped_bytes",
            "extensions",
        ],
    },
    OtlpRecordSpec {
        struct_name: "OtlpPhaseTimingRecord",
        source_struct: "PhaseTimings",
        constructor: "from_timings",
        derives: "Debug, Clone, Copy, PartialEq, Eq",
        fields: &[
            OtlpFieldSpec {
                target: "trap_ingress_ns",
                source: Some("trap_ingress_ns"),
                target_type: None,
                init: None,
            },
            OtlpFieldSpec {
                target: "ffi_hop_ns",
                source: Some("ffi_hop_ns"),
                target_type: None,
                init: None,
            },
            OtlpFieldSpec {
                target: "policy_ns",
                source: Some("policy_ns"),
                target_type: None,
                init: None,
            },
            OtlpFieldSpec {
                target: "verdict_egress_ns",
                source: Some("verdict_egress_ns"),
                target_type: None,
                init: None,
            },
        ],
        ignored: &[],
    },
    OtlpRecordSpec {
        struct_name: "OtlpCausalDeltaRecord",
        source_struct: "CausalDelta",
        constructor: "from_delta",
        derives: "Debug, Clone, PartialEq, Eq",
        fields: &[
            OtlpFieldSpec {
                target: "memory_diff_bytes",
                source: Some("memory_diff_bytes"),
                target_type: None,
                init: None,
            },
            OtlpFieldSpec {
                target: "modified_inodes",
                source: Some("modified_inodes"),
                target_type: None,
                init: None,
            },
            OtlpFieldSpec {
                target: "spawned_pids",
                source: Some("spawned_pids"),
                target_type: None,
                init: None,
            },
            OtlpFieldSpec {
                target: "egress_bytes",
                source: Some("egress_bytes"),
                target_type: None,
                init: None,
            },
            OtlpFieldSpec {
                target: "storage_delta_root",
                source: Some("storage_delta_root"),
                target_type: None,
                init: None,
            },
            OtlpFieldSpec {
                target: "receipt_leaf_hash",
                source: Some("receipt_leaf_hash"),
                target_type: None,
                init: None,
            },
        ],
        ignored: &[],
    },
];

// see: cans/14-observability.md#Metrics-pipeline
fn param_name(spec: &OtlpRecordSpec) -> &'static str {
    match spec.source_struct {
        "ExecutionReceipt" => "receipt",
        "PhaseTimings" => "timings",
        "CausalDelta" => "delta",
        _ => "source",
    }
}

// see: cans/14-observability.md#Metrics-pipeline
pub fn emit_records(model: &Model) -> Result<String, String> {
    let mut needs_hex = false;
    let mut needs_paths = false;
    let mut rendered: Vec<(&OtlpRecordSpec, Vec<RenderedField>)> = Vec::new();
    for spec in OTLP_SPECS {
        let source = model.struct_def(spec.source_struct)?;
        let mut covered: Vec<&str> = Vec::new();
        let mut fields: Vec<RenderedField> = Vec::new();
        for field_spec in spec.fields {
            if let Some(init) = field_spec.init {
                let target_type = field_spec.target_type.ok_or_else(|| {
                    format!(
                        "{}: init field `{}` needs target_type",
                        spec.struct_name, field_spec.target
                    )
                })?;
                covered.push(field_spec.target);
                fields.push((
                    field_spec.target.to_string(),
                    target_type.to_string(),
                    init.to_string(),
                ));
                continue;
            }
            let source_name = field_spec.source.ok_or_else(|| {
                format!(
                    "{}: field `{}` needs source or init",
                    spec.struct_name, field_spec.target
                )
            })?;
            let field = source
                .fields
                .iter()
                .find(|f| f.name == source_name)
                .ok_or_else(|| {
                    format!(
                        "{}: source field `{source_name}` missing on {}",
                        spec.struct_name, spec.source_struct
                    )
                })?;
            covered.push(source_name);
            let param = param_name(spec);
            let (target_type, expr) = if let Ty::VecOf(inner) = &field.ty {
                if matches!(**inner, Ty::PathBuf) {
                    needs_paths = true;
                    (
                        "Vec<String>".to_string(),
                        format!("paths_to_strings(&{param}.{source_name})"),
                    )
                } else {
                    (
                        rust_type(&field.ty),
                        format!("{param}.{source_name}.clone()"),
                    )
                }
            } else if matches!(field.ty, Ty::ByteArray(32)) {
                needs_hex = true;
                (
                    "String".to_string(),
                    format!("hex_lower(&{param}.{source_name})"),
                )
            } else if field.ty.scalar() {
                (rust_type(&field.ty), format!("{param}.{source_name}"))
            } else {
                (
                    rust_type(&field.ty),
                    format!("{param}.{source_name}.clone()"),
                )
            };
            fields.push((field_spec.target.to_string(), target_type, expr));
        }
        for ignored in spec.ignored {
            if !source.fields.iter().any(|f| f.name == *ignored) {
                return Err(format!(
                    "{}: ignored field `{ignored}` missing on {}",
                    spec.struct_name, spec.source_struct
                ));
            }
            covered.push(ignored);
        }
        for field in &source.fields {
            if !covered.contains(&field.name.as_str()) {
                return Err(format!(
                    "{}: source field `{}` of {} is neither projected nor ignored; update OTLP_SPECS in crates/kelir-interface/src/bin/codegen/otlp.rs",
                    spec.struct_name, field.name, spec.source_struct
                ));
            }
        }
        rendered.push((spec, fields));
    }
    let mut out = String::new();
    out.push_str(RECORDS_SEE);
    out.push_str(
        "use kelir_interface::telemetry::{CausalDelta, ExecutionReceipt, PhaseTimings};\n\n",
    );
    if needs_hex {
        out.push_str(RECORDS_SEE);
        out.push_str("fn hex_lower(bytes: &[u8]) -> String {\n");
        out.push_str("    const HEX: &[u8; 16] = b\"0123456789abcdef\";\n");
        out.push_str("    let mut out = String::with_capacity(bytes.len() * 2);\n");
        out.push_str("    for byte in bytes {\n");
        out.push_str("        out.push(HEX[usize::from(*byte >> 4)] as char);\n");
        out.push_str("        out.push(HEX[usize::from(*byte & 0x0f)] as char);\n");
        out.push_str("    }\n");
        out.push_str("    out\n");
        out.push_str("}\n\n");
    }
    if needs_paths {
        out.push_str(RECORDS_SEE);
        out.push_str("fn paths_to_strings(paths: &[std::path::PathBuf]) -> Vec<String> {\n");
        out.push_str("    paths\n");
        out.push_str("        .iter()\n");
        out.push_str("        .map(|path| path.to_string_lossy().into_owned())\n");
        out.push_str("        .collect()\n");
        out.push_str("}\n\n");
    }
    for (spec, fields) in &rendered {
        out.push_str(RECORDS_SEE);
        let _ = writeln!(out, "#[derive({})]", spec.derives);
        let _ = writeln!(out, "pub struct {} {{", spec.struct_name);
        for (name, target_type, _) in fields {
            let _ = writeln!(out, "    pub {name}: {target_type},");
        }
        out.push_str("}\n\n");
        out.push_str(RECORDS_SEE);
        let param = param_name(spec);
        let _ = writeln!(out, "impl {} {{", spec.struct_name);
        let _ = writeln!(
            out,
            "    pub fn {}({}: &{}) -> Self {{",
            spec.constructor, param, spec.source_struct
        );
        out.push_str("        Self {\n");
        for (name, _, expr) in fields {
            let _ = writeln!(out, "            {name}: {expr},");
        }
        out.push_str("        }\n");
        out.push_str("    }\n");
        out.push_str("}\n\n");
    }
    out.truncate(out.trim_end_matches('\n').len());
    out.push('\n');
    Ok(out)
}
