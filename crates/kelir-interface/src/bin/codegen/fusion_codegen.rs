// see: cans/02-plugin-model.md#Segregated-ring-traits
// see: cans/02-plugin-model.md#Hook-context-restriction
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::naming_codegen::camel;

pub struct TagEnum {
    pub name: String,
    pub variants: Vec<(String, String)>,
}

pub struct HookRing {
    pub method: String,
    pub ring: String,
    pub lifecycle_tag: Option<String>,
}

pub struct FusionModel {
    pub rings: TagEnum,
    pub stages: TagEnum,
    pub lifecycle_tags: Vec<String>,
    pub hook_rings: Vec<HookRing>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
struct TagEnumInput {
    name: String,
    variants: Vec<(String, String)>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
impl syn::parse::Parse for TagEnumInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let _vis: syn::Visibility = input.parse()?;
        let _enum_token: syn::Token![enum] = input.parse()?;
        let name: syn::Ident = input.parse()?;
        let content;
        let _brace = syn::braced!(content in input);
        let mut variants = Vec::new();
        while !content.is_empty() {
            let variant: syn::Ident = content.parse()?;
            let _arrow: syn::Token![=>] = content.parse()?;
            let tag: syn::LitStr = content.parse()?;
            variants.push((variant.to_string(), tag.value()));
            if content.peek(syn::Token![,]) {
                let _comma: syn::Token![,] = content.parse()?;
            }
        }
        Ok(TagEnumInput {
            name: name.to_string(),
            variants,
        })
    }
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
fn parse_tag_enum(mac: &syn::Macro) -> Result<TagEnum, String> {
    let input: TagEnumInput = syn::parse2(mac.tokens.clone())
        .map_err(|e| format!("tag_enum! body does not parse: {e}"))?;
    let mut seen_variants = HashSet::new();
    let mut seen_tags = HashSet::new();
    for (variant, tag) in &input.variants {
        if !seen_variants.insert(variant.clone()) {
            return Err(format!("duplicate variant {variant}"));
        }
        if !seen_tags.insert(tag.clone()) {
            return Err(format!("duplicate tag {tag}"));
        }
    }
    if input.variants.is_empty() {
        return Err(format!("enum {} declares no variants", input.name));
    }
    Ok(TagEnum {
        name: input.name,
        variants: input.variants,
    })
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
fn deref(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Reference(reference) => deref(&reference.expr),
        other => other,
    }
}

// see: cans/02-plugin-model.md#Hook-context-restriction
fn string_array(expr: &syn::Expr) -> Result<Vec<String>, String> {
    let syn::Expr::Array(array) = deref(expr) else {
        return Err("expected an array literal".to_string());
    };
    let mut out = Vec::new();
    for elem in array.elems.iter() {
        let syn::Expr::Lit(lit) = elem else {
            return Err("expected a string literal".to_string());
        };
        let syn::Lit::Str(value) = &lit.lit else {
            return Err("expected a string literal".to_string());
        };
        out.push(value.value());
    }
    Ok(out)
}

// see: cans/02-plugin-model.md#Hook-context-restriction
fn parse_hook_rings(expr: &syn::Expr) -> Result<Vec<HookRing>, String> {
    let syn::Expr::Array(array) = deref(expr) else {
        return Err("HOOK_RINGS is not an array literal".to_string());
    };
    let mut bindings = Vec::new();
    let mut seen_methods = HashSet::new();
    for elem in array.elems.iter() {
        let syn::Expr::Struct(item) = elem else {
            return Err("HOOK_RINGS entry is not a struct literal".to_string());
        };
        let mut method: Option<String> = None;
        let mut ring: Option<String> = None;
        let mut lifecycle_tag: Option<Option<String>> = None;
        for field in &item.fields {
            let syn::Member::Named(ident) = &field.member else {
                continue;
            };
            match ident.to_string().as_str() {
                "method" => {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(value),
                        ..
                    }) = &field.expr
                    {
                        method = Some(value.value());
                    }
                }
                "ring" => {
                    if let syn::Expr::Path(path) = &field.expr {
                        ring = path
                            .path
                            .segments
                            .last()
                            .map(|segment| segment.ident.to_string());
                    }
                }
                "lifecycle_tag" => match &field.expr {
                    syn::Expr::Path(path) if path.path.is_ident("None") => {
                        lifecycle_tag = Some(None);
                    }
                    syn::Expr::Call(call) => {
                        if matches!(&*call.func, syn::Expr::Path(path) if path.path.is_ident("Some"))
                        {
                            let Some(arg) = call.args.first() else {
                                return Err("Some() with no argument".to_string());
                            };
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Str(value),
                                ..
                            }) = arg
                            {
                                lifecycle_tag = Some(Some(value.value()));
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        let Some(method) = method else {
            return Err("HOOK_RINGS entry declares no method".to_string());
        };
        let Some(ring) = ring else {
            return Err("HOOK_RINGS entry declares no ring".to_string());
        };
        let Some(lifecycle_tag) = lifecycle_tag else {
            return Err("HOOK_RINGS entry declares no lifecycle_tag".to_string());
        };
        if !seen_methods.insert(method.clone()) {
            return Err(format!("duplicate hook method {method}"));
        }
        bindings.push(HookRing {
            method,
            ring,
            lifecycle_tag,
        });
    }
    if bindings.is_empty() {
        return Err("HOOK_RINGS declares no bindings".to_string());
    }
    Ok(bindings)
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub fn parse_fusion(path: &Path) -> Result<FusionModel, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let file =
        syn::parse_file(&text).map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
    let mut enums = Vec::new();
    let mut lifecycle_tags: Option<Vec<String>> = None;
    let mut hook_rings: Option<Vec<HookRing>> = None;
    for item in file.items {
        match item {
            syn::Item::Macro(item_macro) if item_macro.mac.path.is_ident("tag_enum") => {
                enums.push(parse_tag_enum(&item_macro.mac)?);
            }
            syn::Item::Const(item_const) => {
                let name = item_const.ident.to_string();
                if name == "LIFECYCLE_TAGS" {
                    lifecycle_tags = Some(string_array(&item_const.expr)?);
                }
                if name == "HOOK_RINGS" {
                    hook_rings = Some(parse_hook_rings(&item_const.expr)?);
                }
            }
            _ => {}
        }
    }
    if enums.len() != 2 {
        return Err(format!(
            "expected 2 tag_enum definitions, found {}",
            enums.len()
        ));
    }
    for def in &enums {
        if def.name != "TrampolineRing" && def.name != "HookStage" {
            return Err(format!(
                "unknown tag_enum {}; teach the generator",
                def.name
            ));
        }
    }
    let Some(lifecycle_tags) = lifecycle_tags else {
        return Err("const LIFECYCLE_TAGS not found".to_string());
    };
    if lifecycle_tags.is_empty() {
        return Err("LIFECYCLE_TAGS is empty".to_string());
    }
    if lifecycle_tags.iter().collect::<HashSet<_>>().len() != lifecycle_tags.len() {
        return Err("duplicate LIFECYCLE_TAGS entry".to_string());
    }
    let Some(ring_pos) = enums.iter().position(|def| def.name == "TrampolineRing") else {
        return Err("tag_enum TrampolineRing not found".to_string());
    };
    let rings = enums.remove(ring_pos);
    let Some(stage_pos) = enums.iter().position(|def| def.name == "HookStage") else {
        return Err("tag_enum HookStage not found".to_string());
    };
    let stages = enums.remove(stage_pos);
    let Some(hook_rings) = hook_rings else {
        return Err("const HOOK_RINGS not found".to_string());
    };
    let ring_variants: HashSet<&str> = rings
        .variants
        .iter()
        .map(|(variant, _)| variant.as_str())
        .collect();
    for binding in &hook_rings {
        if !ring_variants.contains(binding.ring.as_str()) {
            return Err(format!(
                "HOOK_RINGS references unknown ring {}",
                binding.ring
            ));
        }
        if let Some(tag) = &binding.lifecycle_tag {
            if binding.ring != "Lifecycle" {
                return Err(format!(
                    "HOOK_RINGS pins lifecycle tag on ring {}",
                    binding.ring
                ));
            }
            if !lifecycle_tags.iter().any(|known| known == tag) {
                return Err(format!("HOOK_RINGS references unknown lifecycle tag {tag}"));
            }
        }
    }
    Ok(FusionModel {
        rings,
        stages,
        lifecycle_tags,
        hook_rings,
    })
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
fn py_member(variant: &str) -> String {
    let chars: Vec<char> = variant.chars().collect();
    let mut out = String::new();
    for (index, ch) in chars.iter().enumerate() {
        if index > 0
            && ch.is_ascii_uppercase()
            && (chars[index - 1].is_ascii_lowercase() || chars[index - 1].is_ascii_digit())
        {
            out.push('_');
        }
        out.push(*ch);
    }
    out.to_uppercase()
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
const FUSION_TS_SEE: &str = "// see: cans/02-plugin-model.md#Segregated-ring-traits
// see: cans/02-plugin-model.md#Hook-context-restriction
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
";

// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub fn emit_typescript(model: &FusionModel) -> String {
    let mut lines: Vec<String> = FUSION_TS_SEE.lines().map(str::to_string).collect();
    lines.push(String::new());
    lines.push("export enum RingEvent {".to_string());
    for (variant, tag) in &model.rings.variants {
        lines.push(format!("  {variant} = \"{tag}\","));
    }
    lines.push("}".to_string());
    lines.push(String::new());
    let stage_union: Vec<String> = model
        .stages
        .variants
        .iter()
        .map(|(_, tag)| format!("\"{tag}\""))
        .collect();
    let single = format!("export type HookStage = {};", stage_union.join(" | "));
    if single.len() <= 80 {
        lines.push(single);
    } else {
        lines.push("export type HookStage =".to_string());
        for (index, tag) in model.stages.variants.iter().enumerate() {
            let mut line = format!("  | \"{}\"", tag.1);
            if index + 1 == model.stages.variants.len() {
                line.push(';');
            }
            lines.push(line);
        }
    }
    lines.push(String::new());
    lines.push("export type LifecycleTag =".to_string());
    for (index, tag) in model.lifecycle_tags.iter().enumerate() {
        let mut line = format!("  | \"{tag}\"");
        if index + 1 == model.lifecycle_tags.len() {
            line.push(';');
        }
        lines.push(line);
    }
    lines.push(String::new());
    lines.push("export const LIFECYCLE_TAGS: readonly string[] = [".to_string());
    for tag in &model.lifecycle_tags {
        lines.push(format!("  \"{tag}\","));
    }
    lines.push("];".to_string());
    lines.push(String::new());
    lines.push("export interface HookRingBinding {".to_string());
    lines.push("  readonly method: string;".to_string());
    lines.push("  readonly ring: RingEvent;".to_string());
    lines.push("  readonly lifecycleTag?: LifecycleTag;".to_string());
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push("export const HOOK_RINGS: readonly HookRingBinding[] = [".to_string());
    for binding in &model.hook_rings {
        let extra = match &binding.lifecycle_tag {
            None => String::new(),
            Some(tag) => format!(", lifecycleTag: \"{tag}\""),
        };
        lines.push(format!(
            "  {{ method: \"{}\", ring: RingEvent.{}{extra} }},",
            camel(&binding.method),
            binding.ring
        ));
    }
    lines.push("];".to_string());
    lines.push(String::new());
    lines.push("export interface PluginBuilderVerb {".to_string());
    lines.push("  readonly verb: string;".to_string());
    lines.push("  readonly ring: RingEvent;".to_string());
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push("export const PLUGIN_BUILDER_VERBS: readonly PluginBuilderVerb[] = [".to_string());
    for binding in &model.hook_rings {
        lines.push(format!(
            "  {{ verb: \"{}\", ring: RingEvent.{} }},",
            camel(&binding.method),
            binding.ring
        ));
    }
    lines.push("];".to_string());
    format!("{}\n", lines.join("\n"))
}

// see: cans/03-sdk-architecture.md#Python-SDK
const FUSION_PY_SEE: &str = "# see: cans/02-plugin-model.md#Segregated-ring-traits
# see: cans/02-plugin-model.md#Hook-context-restriction
# see: cans/02-plugin-model.md#Foreign-plugin-proxy
";

// see: cans/03-sdk-architecture.md#Python-SDK
pub fn emit_python(model: &FusionModel) -> String {
    let mut lines: Vec<String> = FUSION_PY_SEE.lines().map(str::to_string).collect();
    lines.push("from __future__ import annotations".to_string());
    lines.push(String::new());
    lines.push("from enum import Enum".to_string());
    lines.push("from typing import Literal".to_string());
    lines.push(String::new());
    lines.push(String::new());
    lines.push("class RingEvent(str, Enum):".to_string());
    for (variant, tag) in &model.rings.variants {
        lines.push(format!("    {} = \"{tag}\"", py_member(variant)));
    }
    lines.push(String::new());
    lines.push(String::new());
    lines.push("class HookStage(str, Enum):".to_string());
    for (variant, tag) in &model.stages.variants {
        lines.push(format!("    {} = \"{tag}\"", py_member(variant)));
    }
    lines.push(String::new());
    lines.push(String::new());
    lines.push("LifecycleTag = Literal[".to_string());
    for tag in &model.lifecycle_tags {
        lines.push(format!("    \"{tag}\","));
    }
    lines.push("]".to_string());
    lines.push(String::new());
    lines.push("LIFECYCLE_TAGS: tuple[str, ...] = (".to_string());
    for tag in &model.lifecycle_tags {
        lines.push(format!("    \"{tag}\","));
    }
    lines.push(")".to_string());
    lines.push(String::new());
    lines.push("HOOK_RINGS: dict[str, tuple[RingEvent, LifecycleTag | None]] = {".to_string());
    for binding in &model.hook_rings {
        let tag = match &binding.lifecycle_tag {
            None => "None".to_string(),
            Some(tag) => format!("\"{tag}\""),
        };
        lines.push(format!(
            "    \"{}\": (RingEvent.{}, {tag}),",
            binding.method,
            py_member(&binding.ring)
        ));
    }
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push("PLUGIN_BUILDER_VERBS: dict[str, RingEvent] = {".to_string());
    for binding in &model.hook_rings {
        lines.push(format!(
            "    \"{}\": RingEvent.{},",
            binding.method,
            py_member(&binding.ring)
        ));
    }
    lines.push("}".to_string());
    format!("{}\n", lines.join("\n"))
}
