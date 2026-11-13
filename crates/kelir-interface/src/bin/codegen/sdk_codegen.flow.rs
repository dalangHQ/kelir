// see: cans/02-plugin-model.md#Auto-generated-manifest-contract
// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/03-sdk-architecture.md#Python-SDK
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::model_codegen::{EnumDef, FieldDef, Model, StructDef, Ty, parse_model};

pub struct Crate {
    pub dir_name: String,
    pub plugin_name: String,
    pub module_name: String,
    pub model: Model,
    pub plugin_class: String,
    pub plugin_class_name: String,
    pub see_ref: String,
}

impl Crate {
    pub fn struct_def(&self, name: &str) -> Option<&StructDef> {
        self.model.structs.iter().find(|s| s.name == name)
    }

    pub fn enum_def(&self, name: &str) -> Option<&EnumDef> {
        self.model.enums.iter().find(|e| e.name == name)
    }
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
#[derive(Clone)]
pub struct ProjStruct {
    pub rust_name: String,
    pub fields: Vec<FieldDef>,
    pub order: i64,
}

pub struct Projection {
    pub krate: Crate,
    pub root: ProjStruct,
    pub structs: Vec<ProjStruct>,
    pub enum_literals: Vec<(String, Vec<String>, String)>,
}

impl Projection {
    pub fn enum_literal(&self, name: &str) -> Option<&(String, Vec<String>, String)> {
        self.enum_literals.iter().find(|(n, _, _)| n == name)
    }

    pub fn struct_by_name(&self, name: &str) -> Result<&ProjStruct, String> {
        self.structs
            .iter()
            .find(|s| s.rust_name == name)
            .ok_or_else(|| format!("unknown projected struct {name}"))
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
pub fn named_refs(ty: &Ty) -> Vec<String> {
    let mut out = Vec::new();
    match ty {
        Ty::Custom(name) => out.push(name.clone()),
        Ty::Option(inner) | Ty::VecOf(inner) => out.extend(named_refs(inner)),
        Ty::Tuple(items) => {
            for item in items {
                out.extend(named_refs(item));
            }
        }
        _ => {}
    }
    out
}

// see: cans/03-sdk-architecture.md#Python-SDK
pub fn pascal_split(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut parts = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_ascii_uppercase() {
            let mut end = index + 1;
            while end < chars.len() && chars[end].is_ascii_lowercase() {
                end += 1;
            }
            parts.push(chars[index..end].iter().collect());
            index = end;
        } else if ch.is_ascii_digit() {
            let mut end = index;
            while end < chars.len() && chars[end].is_ascii_digit() {
                end += 1;
            }
            parts.push(chars[index..end].iter().collect());
            index = end;
        } else if ch.is_ascii_lowercase() {
            let mut end = index;
            while end < chars.len() && chars[end].is_ascii_lowercase() {
                end += 1;
            }
            parts.push(chars[index..end].iter().collect());
            index = end;
        } else {
            index += 1;
        }
    }
    parts
}

// see: cans/03-sdk-architecture.md#Python-SDK
pub fn py_kebab(name: &str) -> String {
    pascal_split(name)
        .iter()
        .map(|part| part.to_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub fn sdk_camel(name: &str) -> String {
    let mut out = String::new();
    for (index, part) in name.split(['-', '_']).enumerate() {
        if index == 0 {
            out.push_str(part);
        } else {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.push_str(chars.as_str());
            }
        }
    }
    out
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub fn pascal(name: &str) -> String {
    let mut out = String::new();
    for part in name.split('-') {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn sorted_rs_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let read = fs::read_dir(&current)
            .map_err(|e| format!("cannot list {}: {e}", current.display()))?;
        let mut entries: Vec<PathBuf> = Vec::new();
        for entry in read {
            let entry = entry.map_err(|e| format!("cannot list {}: {e}", current.display()))?;
            entries.push(entry.path());
        }
        entries.sort();
        for path in entries {
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn collect_consts(root: &Path) -> Result<Model, String> {
    let mut model = Model {
        structs: Vec::new(),
        enums: Vec::new(),
        consts: Vec::new(),
        defaults: Vec::new(),
        plugin_impls: Vec::new(),
        see_refs: Vec::new(),
    };
    for file in sorted_rs_files(root)? {
        model.merge(parse_model(&file)?);
    }
    Ok(model)
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn parse_crate(crate_dir: &Path, interface_consts: &Model) -> Result<Crate, String> {
    let files = sorted_rs_files(&crate_dir.join("src"))?;
    if files.is_empty() {
        return Err(format!("no sources under {}", crate_dir.display()));
    }
    let dir_name = crate_dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let plugin_name = dir_name
        .strip_prefix("plugin-")
        .unwrap_or(&dir_name)
        .to_string();
    // see: cans/15-operations.md#Multi-language-parity-gate
    let mut model = Model {
        structs: Vec::new(),
        enums: interface_consts.enums.clone(),
        consts: interface_consts.consts.clone(),
        defaults: Vec::new(),
        plugin_impls: Vec::new(),
        see_refs: Vec::new(),
    };
    let mut order: i64 = 0;
    for file in &files {
        let parsed = parse_model(file)?;
        for mut def in parsed.structs {
            if let Some(slot) = model.structs.iter_mut().find(|s| s.name == def.name) {
                let kept = slot.order;
                *slot = def;
                slot.order = kept;
            } else {
                def.order = order;
                order += 1;
                model.structs.push(def);
            }
        }
        for mut def in parsed.enums {
            if let Some(slot) = model.enums.iter_mut().find(|e| e.name == def.name) {
                let kept = slot.order;
                *slot = def;
                slot.order = kept;
            } else {
                def.order = order;
                order += 1;
                model.enums.push(def);
            }
        }
        model.merge(Model {
            structs: Vec::new(),
            enums: Vec::new(),
            consts: parsed.consts,
            defaults: parsed.defaults,
            plugin_impls: parsed.plugin_impls,
            see_refs: parsed.see_refs,
        });
    }
    if model.plugin_impls.len() != 1 {
        let mut names: Vec<String> = model
            .plugin_impls
            .iter()
            .map(|item| item.name.clone())
            .collect();
        names.sort();
        return Err(format!(
            "{dir_name}: expected exactly one KelirPlugin impl, got {names:?}"
        ));
    }
    let plugin_class = model.plugin_impls[0].name.clone();
    let plugin_class_name = model.plugin_impls[0].plugin_name.clone();
    let preferred = format!("cans/plugins/{plugin_name}.md#Plugin-{plugin_name}");
    let see_ref = if model.see_refs.iter().any(|r| r == &preferred) {
        preferred
    } else {
        let prefix = format!("cans/plugins/{plugin_name}.md#");
        match model.see_refs.iter().find(|r| r.starts_with(&prefix)) {
            Some(candidate) => candidate.clone(),
            None => preferred,
        }
    };
    Ok(Crate {
        dir_name,
        plugin_name: plugin_name.clone(),
        module_name: plugin_name.replace('-', "_"),
        model,
        plugin_class,
        plugin_class_name,
        see_ref,
    })
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn add_named(
    krate: &Crate,
    structs: &mut Vec<ProjStruct>,
    enum_literals: &mut Vec<(String, Vec<String>, String)>,
    name: &str,
) -> Result<(), String> {
    if structs.iter().any(|s| s.rust_name == name)
        || enum_literals.iter().any(|(n, _, _)| n == name)
    {
        return Ok(());
    }
    if let Some(def) = krate.struct_def(name) {
        let order = def.order;
        let fields = def.fields.clone();
        structs.push(ProjStruct {
            rust_name: name.to_string(),
            fields: fields.clone(),
            order,
        });
        for field in &fields {
            collect_type(krate, structs, enum_literals, &field.ty)?;
        }
        return Ok(());
    }
    if let Some(def) = krate.enum_def(name) {
        let data_variants: Vec<_> = def
            .variants
            .iter()
            .filter(|v| !v.fields.is_empty())
            .collect();
        if !data_variants.is_empty() {
            if def.variants.len() > 1 {
                return Err(format!(
                    "{}: enum {name} mixes data and unit variants",
                    krate.dir_name
                ));
            }
            let variant = data_variants[0];
            let fields = variant.fields.clone();
            structs.push(ProjStruct {
                rust_name: name.to_string(),
                fields: fields.clone(),
                order: def.order,
            });
            for field in &fields {
                collect_type(krate, structs, enum_literals, &field.ty)?;
            }
            return Ok(());
        }
        let literals: Vec<String> = def.variants.iter().map(|v| py_kebab(&v.name)).collect();
        if literals.is_empty() {
            return Err(format!("{}: enum {name} has no variants", krate.dir_name));
        }
        let default_v = def
            .variants
            .iter()
            .find(|v| v.is_default)
            .unwrap_or(&def.variants[0]);
        enum_literals.push((name.to_string(), literals, py_kebab(&default_v.name)));
        return Ok(());
    }
    Err(format!(
        "{}: type {name} is not defined in the plugin crate",
        krate.dir_name
    ))
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn collect_type(
    krate: &Crate,
    structs: &mut Vec<ProjStruct>,
    enum_literals: &mut Vec<(String, Vec<String>, String)>,
    ty: &Ty,
) -> Result<(), String> {
    for name in named_refs(ty) {
        add_named(krate, structs, enum_literals, &name)?;
    }
    Ok(())
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn topo_sort(items: Vec<ProjStruct>) -> Result<Vec<ProjStruct>, String> {
    let mut remaining = items;
    remaining.sort_by_key(|item| item.order);
    let names: HashSet<String> = remaining
        .iter()
        .map(|item| item.rust_name.clone())
        .collect();
    let mut emitted: Vec<ProjStruct> = Vec::new();
    let mut done: HashSet<String> = HashSet::new();
    while !remaining.is_empty() {
        let mut progressed = false;
        let mut index = 0;
        while index < remaining.len() {
            let deps: HashSet<String> = remaining[index]
                .fields
                .iter()
                .flat_map(|field| named_refs(&field.ty))
                .filter(|name| names.contains(name))
                .collect();
            if deps.iter().all(|name| done.contains(name)) {
                let item = remaining.remove(index);
                done.insert(item.rust_name.clone());
                emitted.push(item);
                progressed = true;
            } else {
                index += 1;
            }
        }
        if !progressed {
            let stuck: Vec<String> = remaining
                .iter()
                .map(|item| item.rust_name.clone())
                .collect();
            return Err(format!("cyclic struct dependency among {stuck:?}"));
        }
    }
    Ok(emitted)
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn build_projection(krate: Crate) -> Result<Projection, String> {
    let root_name = format!("{}Config", pascal(&krate.plugin_name));
    let mut structs: Vec<ProjStruct> = Vec::new();
    let mut enum_literals: Vec<(String, Vec<String>, String)> = Vec::new();
    if krate.struct_def(&root_name).is_some() {
        add_named(&krate, &mut structs, &mut enum_literals, &root_name)?;
    } else {
        structs.push(ProjStruct {
            rust_name: root_name.clone(),
            fields: Vec::new(),
            order: -1,
        });
    }
    let root = structs
        .iter()
        .find(|s| s.rust_name == root_name)
        .ok_or_else(|| format!("unknown projected struct {root_name}"))?
        .clone();
    let ordered = topo_sort(structs)?;
    Ok(Projection {
        krate,
        root,
        structs: ordered,
        enum_literals,
    })
}

// see: cans/03-sdk-architecture.md#Python-SDK
pub fn load_projections(root: &Path) -> Result<Vec<Projection>, String> {
    let interface_consts = collect_consts(&root.join("crates/kelir-interface/src"))?;
    let plugins_src = root.join("crates/kelir-plugins");
    let entries = fs::read_dir(&plugins_src)
        .map_err(|e| format!("cannot list {}: {e}", plugins_src.display()))?;
    let mut crate_dirs: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot list {}: {e}", plugins_src.display()))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() && name.starts_with("plugin-") {
            crate_dirs.push(path);
        }
    }
    crate_dirs.sort();
    let mut projections = Vec::new();
    for dir in crate_dirs {
        projections.push(build_projection(parse_crate(&dir, &interface_consts)?)?);
    }
    Ok(projections)
}
