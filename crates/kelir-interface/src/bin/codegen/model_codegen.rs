// see: cans/15-operations.md#Multi-language-parity-gate
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    I8,
    I16,
    I32,
    I64,
    Isize,
    F32,
    F64,
    Str,
    Bool,
    PathBuf,
    Option(Box<Ty>),
    VecOf(Box<Ty>),
    Tuple(Vec<Ty>),
    ByteArray(usize),
    Custom(String),
}

impl Ty {
    pub fn scalar(&self) -> bool {
        matches!(
            self,
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
                | Ty::F64
                | Ty::Bool
                | Ty::ByteArray(_)
        )
    }
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub ty: Ty,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub serde_camel: bool,
    pub order: i64,
}

#[derive(Debug, Clone)]
pub struct VariantDef {
    pub name: String,
    pub payload: Option<Ty>,
    pub fields: Vec<FieldDef>,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<VariantDef>,
    pub serde_camel: bool,
    pub order: i64,
}

// see: cans/03-sdk-architecture.md#Python-SDK
#[derive(Clone)]
pub struct ConstDef {
    pub name: String,
    pub expr: syn::Expr,
}

// see: cans/03-sdk-architecture.md#Python-SDK
#[derive(Clone)]
pub struct DefaultImpl {
    pub name: String,
    pub fields: Vec<(String, syn::Expr)>,
}

// see: cans/03-sdk-architecture.md#Python-SDK
#[derive(Debug, Clone)]
pub struct PluginImpl {
    pub name: String,
    pub plugin_name: String,
}

pub struct Model {
    pub structs: Vec<StructDef>,
    pub enums: Vec<EnumDef>,
    pub consts: Vec<ConstDef>,
    pub defaults: Vec<DefaultImpl>,
    pub plugin_impls: Vec<PluginImpl>,
    pub see_refs: Vec<String>,
}

// see: cans/10-attestation.md#Execution-receipts
fn classify(ty: &syn::Type) -> Result<Ty, String> {
    match ty {
        syn::Type::Path(type_path) => {
            let Some(segment) = type_path.path.segments.last() else {
                return Err("type path with no segments".to_string());
            };
            let name = segment.ident.to_string();
            match name.as_str() {
                "u8" => Ok(Ty::U8),
                "u16" => Ok(Ty::U16),
                "u32" => Ok(Ty::U32),
                "u64" => Ok(Ty::U64),
                "u128" => Ok(Ty::U128),
                "usize" => Ok(Ty::Usize),
                "i8" => Ok(Ty::I8),
                "i16" => Ok(Ty::I16),
                "i32" => Ok(Ty::I32),
                "i64" => Ok(Ty::I64),
                "isize" => Ok(Ty::Isize),
                "f32" => Ok(Ty::F32),
                "f64" => Ok(Ty::F64),
                "bool" => Ok(Ty::Bool),
                "String" => Ok(Ty::Str),
                "PathBuf" => Ok(Ty::PathBuf),
                "Option" | "Vec" => {
                    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
                        return Err(format!("{name} without type arguments"));
                    };
                    let Some(first) = args.args.first() else {
                        return Err(format!("{name} with no type argument"));
                    };
                    let syn::GenericArgument::Type(inner) = first else {
                        return Err(format!("{name} with non-type argument"));
                    };
                    let inner_ty = classify(inner)?;
                    if name == "Option" {
                        Ok(Ty::Option(Box::new(inner_ty)))
                    } else {
                        Ok(Ty::VecOf(Box::new(inner_ty)))
                    }
                }
                _ => Ok(Ty::Custom(name)),
            }
        }
        syn::Type::Array(array) => {
            if !matches!(*array.elem, syn::Type::Path(ref p) if p.path.is_ident("u8")) {
                return Err("array of non-u8 elements".to_string());
            }
            let syn::Expr::Lit(lit) = &array.len else {
                return Err("array length is not a literal".to_string());
            };
            let syn::Lit::Int(len) = &lit.lit else {
                return Err("array length is not an integer literal".to_string());
            };
            let len = len
                .base10_parse::<usize>()
                .map_err(|_| "array length out of range".to_string())?;
            Ok(Ty::ByteArray(len))
        }
        syn::Type::Tuple(tuple) => {
            let mut items = Vec::new();
            for elem in tuple.elems.iter() {
                items.push(classify(elem)?);
            }
            Ok(Ty::Tuple(items))
        }
        syn::Type::Paren(paren) => Ok(Ty::Tuple(vec![classify(&paren.elem)?])),
        syn::Type::Reference(reference) => match &*reference.elem {
            syn::Type::Path(path) if path.path.is_ident("str") => Ok(Ty::Str),
            syn::Type::Path(path) if path.path.is_ident("Path") => Ok(Ty::PathBuf),
            _ => Err("unsupported reference type".to_string()),
        },
        _ => Err("unsupported field type shape".to_string()),
    }
}

// see: cans/10-attestation.md#Execution-receipts
fn serde_camel(attrs: &[syn::Attribute]) -> bool {
    let mut camel = false;
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename_all") {
                match meta.value().and_then(|v| v.parse::<syn::LitStr>()) {
                    Ok(value) if value.value() == "camelCase" => camel = true,
                    _ => {}
                }
            }
            Ok(())
        });
    }
    camel
}

// see: cans/15-operations.md#Multi-language-parity-gate
pub fn parse_model(path: &Path) -> Result<Model, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let file =
        syn::parse_file(&text).map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut defaults = Vec::new();
    let mut plugin_impls = Vec::new();
    for item in &file.items {
        match item {
            syn::Item::Struct(item_struct) => {
                let camel = serde_camel(&item_struct.attrs);
                let mut fields = Vec::new();
                let mut supported = true;
                for field in item_struct.fields.iter() {
                    let Some(name) = field.ident.as_ref().map(|i| i.to_string()) else {
                        continue;
                    };
                    match classify(&field.ty) {
                        Ok(ty) => fields.push(FieldDef { name, ty }),
                        Err(_) => {
                            supported = false;
                            break;
                        }
                    }
                }
                if supported {
                    structs.push(StructDef {
                        name: item_struct.ident.to_string(),
                        fields,
                        serde_camel: camel,
                        order: structs.len() as i64,
                    });
                }
            }
            syn::Item::Enum(item_enum) => {
                let camel = serde_camel(&item_enum.attrs);
                let mut variants = Vec::new();
                let mut supported = true;
                for variant in &item_enum.variants {
                    let is_default = variant
                        .attrs
                        .iter()
                        .any(|attr| attr.path().is_ident("default"));
                    let (payload, fields) = match &variant.fields {
                        syn::Fields::Unit => (None, Vec::new()),
                        syn::Fields::Unnamed(unnamed) => {
                            if unnamed.unnamed.len() != 1 {
                                supported = false;
                                break;
                            }
                            let Some(first) = unnamed.unnamed.first() else {
                                supported = false;
                                break;
                            };
                            match classify(&first.ty) {
                                Ok(ty) => (Some(ty), Vec::new()),
                                Err(_) => {
                                    supported = false;
                                    break;
                                }
                            }
                        }
                        syn::Fields::Named(named) => {
                            let mut vfields = Vec::new();
                            let mut named_ok = true;
                            for field in named.named.iter() {
                                let Some(name) = field.ident.as_ref().map(|i| i.to_string()) else {
                                    named_ok = false;
                                    break;
                                };
                                match classify(&field.ty) {
                                    Ok(ty) => vfields.push(FieldDef { name, ty }),
                                    Err(_) => {
                                        named_ok = false;
                                        break;
                                    }
                                }
                            }
                            if !named_ok {
                                supported = false;
                                break;
                            }
                            (None, vfields)
                        }
                    };
                    variants.push(VariantDef {
                        name: variant.ident.to_string(),
                        payload,
                        fields,
                        is_default,
                    });
                }
                if supported {
                    enums.push(EnumDef {
                        name: item_enum.ident.to_string(),
                        variants,
                        serde_camel: camel,
                        order: enums.len() as i64,
                    });
                }
            }
            syn::Item::Impl(item_impl) => {
                let Some((_, trait_path, _)) = &item_impl.trait_ else {
                    continue;
                };
                let Some(trait_name) = trait_path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
                else {
                    continue;
                };
                let syn::Type::Path(self_path) = &*item_impl.self_ty else {
                    continue;
                };
                let Some(self_name) = self_path
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
                else {
                    continue;
                };
                for item in &item_impl.items {
                    let syn::ImplItem::Fn(method) = item else {
                        continue;
                    };
                    if trait_name == "Default" && method.sig.ident == "default" {
                        let Some(fields) = find_struct_literal(&method.block, &self_name) else {
                            continue;
                        };
                        defaults.push(DefaultImpl {
                            name: self_name.clone(),
                            fields,
                        });
                    }
                    if trait_name == "KelirPlugin" && method.sig.ident == "name" {
                        let Some(plugin_name) = find_string_literal(&method.block) else {
                            continue;
                        };
                        plugin_impls.push(PluginImpl {
                            name: self_name.clone(),
                            plugin_name,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(Model {
        structs,
        enums,
        consts: collect_consts(&file),
        defaults,
        plugin_impls,
        see_refs: scan_see_refs(&text),
    })
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn scan_see_refs(text: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut search = text;
    while let Some(at) = search.find("see: cans") {
        let start = at + "see: ".len();
        let tail = &search[start..];
        let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
        let raw = &tail[..end];
        if raw.starts_with("cans/") {
            refs.push(raw.trim_end_matches(['`', '.']).to_string());
        }
        search = &search[start + end..];
    }
    refs
}

// see: cans/03-sdk-architecture.md#Python-SDK
struct ConstCollector {
    consts: Vec<ConstDef>,
}

// see: cans/03-sdk-architecture.md#Python-SDK
impl<'ast> syn::visit::Visit<'ast> for ConstCollector {
    fn visit_item_const(&mut self, node: &'ast syn::ItemConst) {
        self.consts.push(ConstDef {
            name: node.ident.to_string(),
            expr: (*node.expr).clone(),
        });
        syn::visit::visit_item_const(self, node);
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn collect_consts(file: &syn::File) -> Vec<ConstDef> {
    let mut collector = ConstCollector { consts: Vec::new() };
    syn::visit::visit_file(&mut collector, file);
    collector.consts
}

// see: cans/03-sdk-architecture.md#Python-SDK
struct StructLiteralFinder<'a> {
    name: &'a str,
    found: Option<Vec<(String, syn::Expr)>>,
}

// see: cans/03-sdk-architecture.md#Python-SDK
impl<'ast> syn::visit::Visit<'ast> for StructLiteralFinder<'ast> {
    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if self.found.is_none() {
            let path_name = node
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string());
            if path_name.as_deref() == Some("Self") || path_name.as_deref() == Some(self.name) {
                let mut fields = Vec::new();
                for field in &node.fields {
                    if let syn::Member::Named(ident) = &field.member {
                        fields.push((ident.to_string(), field.expr.clone()));
                    }
                }
                self.found = Some(fields);
            }
        }
        syn::visit::visit_expr_struct(self, node);
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn find_struct_literal(block: &syn::Block, name: &str) -> Option<Vec<(String, syn::Expr)>> {
    let mut finder = StructLiteralFinder { name, found: None };
    syn::visit::visit_block(&mut finder, block);
    finder.found
}

// see: cans/03-sdk-architecture.md#Python-SDK
struct StringLiteralFinder {
    found: Option<String>,
}

// see: cans/03-sdk-architecture.md#Python-SDK
impl<'ast> syn::visit::Visit<'ast> for StringLiteralFinder {
    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        if self.found.is_none() {
            self.found = Some(node.value());
        }
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn find_string_literal(block: &syn::Block) -> Option<String> {
    let mut finder = StringLiteralFinder { found: None };
    syn::visit::visit_block(&mut finder, block);
    finder.found
}

impl Model {
    pub fn struct_def(&self, name: &str) -> Result<&StructDef, String> {
        self.structs
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| format!("struct {name} not found"))
    }

    pub fn enum_def(&self, name: &str) -> Result<&EnumDef, String> {
        self.enums
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| format!("enum {name} not found"))
    }

    pub fn const_def(&self, name: &str) -> Option<&ConstDef> {
        self.consts.iter().find(|c| c.name == name)
    }

    pub fn default_impl(&self, name: &str) -> Option<&DefaultImpl> {
        self.defaults.iter().find(|d| d.name == name)
    }

    pub fn merge(&mut self, other: Model) {
        for def in other.structs {
            replace_named(&mut self.structs, def, |s| s.name.clone());
        }
        for def in other.enums {
            replace_named(&mut self.enums, def, |e| e.name.clone());
        }
        for def in other.consts {
            replace_named(&mut self.consts, def, |c| c.name.clone());
        }
        for def in other.defaults {
            replace_named(&mut self.defaults, def, |d| d.name.clone());
        }
        for def in other.plugin_impls {
            replace_named(&mut self.plugin_impls, def, |p| p.name.clone());
        }
        self.see_refs.extend(other.see_refs);
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn replace_named<T>(items: &mut Vec<T>, def: T, name_of: impl Fn(&T) -> String) {
    let name = name_of(&def);
    if let Some(slot) = items.iter_mut().find(|item| name_of(item) == name) {
        *slot = def;
    } else {
        items.push(def);
    }
}

// see: cans/14-observability.md#Metrics-pipeline
pub fn rust_type(ty: &Ty) -> String {
    match ty {
        Ty::U8 => "u8".to_string(),
        Ty::U16 => "u16".to_string(),
        Ty::U32 => "u32".to_string(),
        Ty::U64 => "u64".to_string(),
        Ty::U128 => "u128".to_string(),
        Ty::Usize => "usize".to_string(),
        Ty::I8 => "i8".to_string(),
        Ty::I16 => "i16".to_string(),
        Ty::I32 => "i32".to_string(),
        Ty::I64 => "i64".to_string(),
        Ty::Isize => "isize".to_string(),
        Ty::F32 => "f32".to_string(),
        Ty::F64 => "f64".to_string(),
        Ty::Str => "String".to_string(),
        Ty::Bool => "bool".to_string(),
        Ty::PathBuf => "std::path::PathBuf".to_string(),
        Ty::Option(inner) => format!("Option<{}>", rust_type(inner)),
        Ty::VecOf(inner) => format!("Vec<{}>", rust_type(inner)),
        Ty::Tuple(items) => {
            let inner: Vec<String> = items.iter().map(rust_type).collect();
            if inner.len() == 1 {
                format!("({},)", inner[0])
            } else {
                format!("({})", inner.join(", "))
            }
        }
        Ty::ByteArray(len) => format!("[u8; {len}]"),
        Ty::Custom(name) => name.clone(),
    }
}
