// see: cans/10-attestation.md#Execution-receipts
use std::fmt::Write as _;

use crate::model::{EnumDef, Model, Ty, rust_type};
use crate::naming::snake;

const MERKLE_SEE: &str = "// see: cans/10-attestation.md#Execution-receipts
";
const MERKLE_SEE_IND: &str = "    // see: cans/10-attestation.md#Execution-receipts
";
const WAL_SEE: &str = "// see: cans/plugins/ledger.md#Merkle-WAL-specification
";
const WAL_SEE_IND: &str = "    // see: cans/plugins/ledger.md#Merkle-WAL-specification
";

// see: cans/10-attestation.md#Execution-receipts
struct DomainLeafSpec {
    struct_name: &'static str,
    domain: &'static str,
    skip: &'static [&'static str],
}

const DOMAIN_LEAVES: &[DomainLeafSpec] = &[DomainLeafSpec {
    struct_name: "ExecutionReceipt",
    domain: "kelir-receipt-leaf-v1",
    skip: &["wall_clock_ns", "phase_timings"],
}];

// see: cans/plugins/ledger.md#Merkle-WAL-specification
struct PackedRecordSpec {
    struct_name: &'static str,
    record_len_const: &'static str,
}

const PACKED_RECORDS: &[PackedRecordSpec] = &[PackedRecordSpec {
    struct_name: "WalRecord",
    record_len_const: "WAL_RECORD_LEN",
}];

#[derive(Default)]
struct HashNeeds {
    u64: bool,
    str: bool,
    opt_str: bool,
    opt_u64: bool,
    enums: Vec<String>,
    structs: Vec<String>,
}

// see: cans/10-attestation.md#Execution-receipts
fn finalize_needs(needs: &mut HashNeeds) {
    if needs.opt_str {
        needs.str = true;
    }
    if needs.str {
        needs.u64 = true;
    }
    if needs.opt_u64 {
        needs.u64 = true;
    }
}

// see: cans/10-attestation.md#Execution-receipts
fn destructure_lines(
    struct_name: &str,
    bindings: &[String],
    target: &str,
    indent: usize,
) -> Vec<String> {
    let pad = " ".repeat(indent);
    let joined = bindings.join(", ");
    let single = format!("{pad}let {struct_name} {{ {joined} }} = {target};");
    if joined.chars().count() <= 18 && single.chars().count() <= 100 {
        return vec![single];
    }
    let mut lines = vec![format!("{pad}let {struct_name} {{")];
    for binding in bindings {
        lines.push(format!("{pad}    {binding},"));
    }
    lines.push(format!("{pad}}} = {target};"));
    lines
}

// see: cans/10-attestation.md#Execution-receipts
fn hash_stmts(
    field: &str,
    ty: &Ty,
    model: &Model,
    needs: &mut HashNeeds,
    indent: usize,
) -> Result<Vec<String>, String> {
    let pad = " ".repeat(indent);
    match ty {
        Ty::U8 | Ty::U16 | Ty::U32 => {
            needs.u64 = true;
            Ok(vec![format!(
                "{pad}hash_u64(&mut hasher, u64::from(*{field}));"
            )])
        }
        Ty::Bool => {
            needs.u64 = true;
            Ok(vec![format!(
                "{pad}hash_u64(&mut hasher, *{field} as u64);"
            )])
        }
        Ty::U64 => {
            needs.u64 = true;
            Ok(vec![format!("{pad}hash_u64(&mut hasher, *{field});")])
        }
        Ty::I32 => Ok(vec![format!("{pad}hasher.update(&{field}.to_le_bytes());")]),
        Ty::Str => {
            needs.str = true;
            Ok(vec![format!("{pad}hash_str(&mut hasher, {field});")])
        }
        Ty::PathBuf => {
            needs.str = true;
            Ok(vec![format!(
                "{pad}hash_str(&mut hasher, &{field}.to_string_lossy());"
            )])
        }
        Ty::ByteArray(_) => {
            needs.u64 = true;
            Ok(vec![
                format!("{pad}hash_u64(&mut hasher, {field}.len() as u64);"),
                format!("{pad}hasher.update({field});"),
            ])
        }
        Ty::Option(inner) => match &**inner {
            Ty::Str => {
                needs.opt_str = true;
                Ok(vec![format!("{pad}hash_opt_str(&mut hasher, {field});")])
            }
            Ty::U64 => {
                needs.opt_u64 = true;
                Ok(vec![format!("{pad}hash_opt_u64(&mut hasher, {field});")])
            }
            other => Err(format!(
                "merkle: field `{field}` of type Option<{}> has no hash rule; extend the merkle rules in crates/kelir-interface/src/bin/codegen/merkle.rs",
                rust_type(other)
            )),
        },
        Ty::VecOf(inner) => match &**inner {
            Ty::U8 => {
                needs.u64 = true;
                Ok(vec![
                    format!("{pad}hash_u64(&mut hasher, {field}.len() as u64);"),
                    format!("{pad}hasher.update({field});"),
                ])
            }
            Ty::Custom(name) => {
                let nested = model.struct_def(name).map_err(|_| {
                    format!("merkle: field `{field}` references unknown struct {name}")
                })?;
                needs.u64 = true;
                if !needs.structs.iter().any(|s| s == name) {
                    needs.structs.push(name.clone());
                }
                let mut lines = vec![
                    format!("{pad}hash_u64(&mut hasher, {field}.len() as u64);"),
                    format!("{pad}for item in {field} {{"),
                ];
                let bindings = nested
                    .fields
                    .iter()
                    .map(|f| f.name.clone())
                    .collect::<Vec<_>>();
                lines.extend(destructure_lines(name, &bindings, "item", indent + 4));
                for nested_field in &nested.fields {
                    lines.extend(hash_stmts(
                        &nested_field.name,
                        &nested_field.ty,
                        model,
                        needs,
                        indent + 4,
                    )?);
                }
                lines.push(format!("{pad}}}"));
                Ok(lines)
            }
            other => Err(format!(
                "merkle: field `{field}` of type Vec<{}> has no hash rule; extend the merkle rules in crates/kelir-interface/src/bin/codegen/merkle.rs",
                rust_type(other)
            )),
        },
        Ty::Custom(name) => {
            if model.enum_def(name).is_ok() {
                if !needs.enums.iter().any(|e| e == name) {
                    needs.enums.push(name.clone());
                }
                let hasher_fn = format!("hash_{}", snake(name));
                Ok(vec![format!("{pad}{hasher_fn}(&mut hasher, {field});")])
            } else {
                Err(format!(
                    "merkle: field `{field}` of type {name} has no hash rule; extend the merkle rules in crates/kelir-interface/src/bin/codegen/merkle.rs"
                ))
            }
        }
    }
}

// see: cans/10-attestation.md#Execution-receipts
fn emit_hash_helpers(needs: &HashNeeds) -> String {
    let mut out = String::new();
    if needs.u64 {
        out.push_str(MERKLE_SEE);
        out.push_str("fn hash_u64(hasher: &mut blake3::Hasher, value: u64) {\n");
        out.push_str("    hasher.update(&value.to_le_bytes());\n");
        out.push_str("}\n\n");
    }
    if needs.str {
        out.push_str(MERKLE_SEE);
        out.push_str("fn hash_str(hasher: &mut blake3::Hasher, value: &str) {\n");
        out.push_str("    hash_u64(hasher, value.len() as u64);\n");
        out.push_str("    hasher.update(value.as_bytes());\n");
        out.push_str("}\n\n");
    }
    if needs.opt_str {
        out.push_str(MERKLE_SEE);
        out.push_str("fn hash_opt_str(hasher: &mut blake3::Hasher, value: &Option<String>) {\n");
        out.push_str("    match value {\n");
        out.push_str("        Some(text) => {\n");
        out.push_str("            hasher.update(&[1u8]);\n");
        out.push_str("            hash_str(hasher, text);\n");
        out.push_str("        }\n");
        out.push_str("        None => {\n");
        out.push_str("            hasher.update(&[0u8]);\n");
        out.push_str("        }\n");
        out.push_str("    }\n");
        out.push_str("}\n\n");
    }
    if needs.opt_u64 {
        out.push_str(MERKLE_SEE);
        out.push_str("fn hash_opt_u64(hasher: &mut blake3::Hasher, value: &Option<u64>) {\n");
        out.push_str("    match value {\n");
        out.push_str("        Some(number) => {\n");
        out.push_str("            hasher.update(&[1u8]);\n");
        out.push_str("            hash_u64(hasher, *number);\n");
        out.push_str("        }\n");
        out.push_str("        None => {\n");
        out.push_str("            hasher.update(&[0u8]);\n");
        out.push_str("        }\n");
        out.push_str("    }\n");
        out.push_str("}\n\n");
    }
    out
}

// see: cans/10-attestation.md#Execution-receipts
fn emit_enum_hasher(def: &EnumDef, needs: &mut HashNeeds) -> Result<String, String> {
    let fn_name = format!("hash_{}", snake(&def.name));
    let mut out = String::new();
    out.push_str(MERKLE_SEE);
    let _ = writeln!(
        out,
        "fn {fn_name}(hasher: &mut blake3::Hasher, value: &{}) {{",
        def.name
    );
    out.push_str("    match value {\n");
    for (index, variant) in def.variants.iter().enumerate() {
        match &variant.payload {
            None => {
                let _ = writeln!(out, "        {}::{} => {{", def.name, variant.name);
                let _ = writeln!(out, "            hasher.update(&[{index}u8]);");
                out.push_str("        }\n");
            }
            Some(Ty::U32) => {
                let _ = writeln!(out, "        {}::{}(code) => {{", def.name, variant.name);
                let _ = writeln!(out, "            hasher.update(&[{index}u8]);");
                let _ = writeln!(out, "            hasher.update(&code.to_le_bytes());");
                out.push_str("        }\n");
            }
            Some(Ty::Str) => {
                needs.str = true;
                let _ = writeln!(out, "        {}::{}(line) => {{", def.name, variant.name);
                let _ = writeln!(out, "            hasher.update(&[{index}u8]);");
                let _ = writeln!(out, "            hash_str(hasher, line);");
                out.push_str("        }\n");
            }
            Some(other) => {
                return Err(format!(
                    "{}::{}: payload {} has no merkle hash rule; extend the merkle rules in crates/kelir-interface/src/bin/codegen/merkle.rs",
                    def.name,
                    variant.name,
                    rust_type(other)
                ));
            }
        }
    }
    out.push_str("    }\n");
    out.push_str("}\n\n");
    Ok(out)
}

// see: cans/10-attestation.md#Execution-receipts
pub fn emit_domain_leaves(model: &Model) -> Result<String, String> {
    let mut out = String::new();
    for spec in DOMAIN_LEAVES {
        let def = model.struct_def(spec.struct_name)?;
        for skipped in spec.skip {
            if !def.fields.iter().any(|f| f.name == *skipped) {
                return Err(format!(
                    "{}: skip field `{skipped}` not found; update DOMAIN_LEAVES in crates/kelir-interface/src/bin/codegen/merkle.rs",
                    spec.struct_name
                ));
            }
        }
        let mut needs = HashNeeds {
            str: true,
            ..HashNeeds::default()
        };
        let mut body = Vec::new();
        for field in &def.fields {
            if spec.skip.contains(&field.name.as_str()) {
                continue;
            }
            body.extend(hash_stmts(&field.name, &field.ty, model, &mut needs, 8)?);
        }
        let enum_names = needs.enums.clone();
        let mut enum_hashers = Vec::new();
        for enum_name in &enum_names {
            let enum_def = model.enum_def(enum_name)?;
            enum_hashers.push(emit_enum_hasher(enum_def, &mut needs)?);
        }
        finalize_needs(&mut needs);
        let mut imports: Vec<String> = vec![spec.struct_name.to_string()];
        imports.extend(needs.enums.iter().cloned());
        imports.extend(needs.structs.iter().cloned());
        imports.sort();
        imports.dedup();
        out.push_str(MERKLE_SEE);
        let _ = writeln!(out, "use super::{{{}}};", imports.join(", "));
        out.push('\n');
        out.push_str(&emit_hash_helpers(&needs));
        for hasher in &enum_hashers {
            out.push_str(hasher);
        }
        out.push_str(MERKLE_SEE);
        let _ = writeln!(out, "impl {} {{", spec.struct_name);
        out.push_str(MERKLE_SEE_IND);
        out.push_str("    pub fn leaf_hash(&self) -> [u8; 32] {\n");
        let mut bindings = Vec::new();
        for field in &def.fields {
            if spec.skip.contains(&field.name.as_str()) {
                bindings.push(format!("{}: _", field.name));
            } else {
                bindings.push(field.name.clone());
            }
        }
        for line in destructure_lines("Self", &bindings, "self", 8) {
            let _ = writeln!(out, "{line}");
        }
        out.push_str("        let mut hasher = blake3::Hasher::new();\n");
        let _ = writeln!(out, "        hash_str(&mut hasher, \"{}\");", spec.domain);
        for line in &body {
            let _ = writeln!(out, "{line}");
        }
        out.push_str("        *hasher.finalize().as_bytes()\n");
        out.push_str("    }\n");
        out.push_str("}\n");
    }
    Ok(out)
}

// see: cans/plugins/ledger.md#Merkle-WAL-specification
pub fn emit_packed_records(model: &Model) -> Result<String, String> {
    let mut out = String::new();
    for spec in PACKED_RECORDS {
        let def = model.struct_def(spec.struct_name)?;
        let packed_size = |ty: &Ty| -> Result<usize, String> {
            match ty {
                Ty::U64 => Ok(8),
                Ty::U32 | Ty::I32 => Ok(4),
                Ty::U16 => Ok(2),
                Ty::U8 | Ty::Bool => Ok(1),
                Ty::ByteArray(len) => Ok(*len),
                other => Err(format!(
                    "{}: field type {} has no fixed pack layout; extend the pack rules in crates/kelir-interface/src/bin/codegen/merkle.rs",
                    spec.struct_name,
                    rust_type(other)
                )),
            }
        };
        let mut offset = 0usize;
        let mut pack_lines = Vec::new();
        let mut bindings = Vec::new();
        for field in &def.fields {
            let size = packed_size(&field.ty)?;
            let end = offset + size;
            bindings.push(field.name.clone());
            match &field.ty {
                Ty::Bool => pack_lines.push(format!(
                    "        bytes[{offset}] = u8::from(*{});",
                    field.name
                )),
                Ty::ByteArray(_) => pack_lines.push(format!(
                    "        bytes[{offset}..{end}].copy_from_slice({});",
                    field.name
                )),
                _ => pack_lines.push(format!(
                    "        bytes[{offset}..{end}].copy_from_slice(&{}.to_le_bytes());",
                    field.name
                )),
            }
            offset = end;
        }
        let total = offset;
        out.push_str(WAL_SEE);
        let _ = writeln!(
            out,
            "use super::{{{}, {}, merkle_leaf}};",
            spec.record_len_const, spec.struct_name
        );
        out.push('\n');
        out.push_str(WAL_SEE);
        let _ = writeln!(
            out,
            "const _: () = assert!({} == {total});",
            spec.record_len_const
        );
        out.push('\n');
        out.push_str(WAL_SEE);
        let _ = writeln!(out, "impl {} {{", spec.struct_name);
        out.push_str(WAL_SEE_IND);
        let _ = writeln!(
            out,
            "    pub fn pack(&self) -> [u8; {}] {{",
            spec.record_len_const
        );
        for line in destructure_lines("Self", &bindings, "self", 8) {
            let _ = writeln!(out, "{line}");
        }
        out.push_str("        let mut bytes = [0u8; ");
        out.push_str(spec.record_len_const);
        out.push_str("];\n");
        for line in &pack_lines {
            let _ = writeln!(out, "{line}");
        }
        out.push_str("        bytes\n");
        out.push_str("    }\n");
        out.push_str(WAL_SEE_IND);
        out.push_str("    pub fn leaf_hash(&self) -> [u8; 32] {\n");
        out.push_str("        merkle_leaf(&self.pack())\n");
        out.push_str("    }\n");
        out.push_str("}\n");
    }
    Ok(out)
}
