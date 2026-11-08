// see: cans/15-operations.md#Multi-language-parity-gate
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    U8,
    U16,
    U32,
    U64,
    I32,
    Str,
    Bool,
    PathBuf,
    Option(Box<Ty>),
    VecOf(Box<Ty>),
    ByteArray(usize),
    Custom(String),
}

impl Ty {
    pub fn scalar(&self) -> bool {
        matches!(
            self,
            Ty::U8 | Ty::U16 | Ty::U32 | Ty::U64 | Ty::I32 | Ty::Bool | Ty::ByteArray(_)
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
}

#[derive(Debug, Clone)]
pub struct VariantDef {
    pub name: String,
    pub payload: Option<Ty>,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<VariantDef>,
    pub serde_camel: bool,
}

pub struct Model {
    pub structs: Vec<StructDef>,
    pub enums: Vec<EnumDef>,
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
                "i32" => Ok(Ty::I32),
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
    for item in file.items {
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
                    });
                }
            }
            syn::Item::Enum(item_enum) => {
                let camel = serde_camel(&item_enum.attrs);
                let mut variants = Vec::new();
                let mut supported = true;
                for variant in &item_enum.variants {
                    let payload = match &variant.fields {
                        syn::Fields::Unit => None,
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
                                Ok(ty) => Some(ty),
                                Err(_) => {
                                    supported = false;
                                    break;
                                }
                            }
                        }
                        syn::Fields::Named(_) => {
                            supported = false;
                            break;
                        }
                    };
                    variants.push(VariantDef {
                        name: variant.ident.to_string(),
                        payload,
                    });
                }
                if supported {
                    enums.push(EnumDef {
                        name: item_enum.ident.to_string(),
                        variants,
                        serde_camel: camel,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(Model { structs, enums })
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
}

// see: cans/14-observability.md#Metrics-pipeline
pub fn rust_type(ty: &Ty) -> String {
    match ty {
        Ty::U8 => "u8".to_string(),
        Ty::U16 => "u16".to_string(),
        Ty::U32 => "u32".to_string(),
        Ty::U64 => "u64".to_string(),
        Ty::I32 => "i32".to_string(),
        Ty::Str => "String".to_string(),
        Ty::Bool => "bool".to_string(),
        Ty::PathBuf => "std::path::PathBuf".to_string(),
        Ty::Option(inner) => format!("Option<{}>", rust_type(inner)),
        Ty::VecOf(inner) => format!("Vec<{}>", rust_type(inner)),
        Ty::ByteArray(len) => format!("[u8; {len}]"),
        Ty::Custom(name) => name.clone(),
    }
}
