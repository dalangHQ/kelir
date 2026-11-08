// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/03-sdk-architecture.md#TypeScript-SDK
use std::collections::HashSet;

use napi::bindgen_prelude::Object;
use napi::{Error, Result};

pub use kelir_interface::fusion::{
    FilterSnapshot, FusedTrampolineDescriptor, HookEntry, HookStage, RingChain, TrampolineRing,
    TrampolineTable, validate_hook,
};

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub fn convert_descriptor(rings: Vec<Object>) -> Result<FusedTrampolineDescriptor> {
    let mut descriptor = FusedTrampolineDescriptor::default();
    let mut seen_rings = HashSet::new();
    for ring in rings {
        let ring_tag: String = ring.get_named_property::<String>("ring").map_err(|_| {
            Error::from_reason("fused trampoline ring entry declares no `ring` tag")
        })?;
        let ring_kind = TrampolineRing::from_tag(&ring_tag).ok_or_else(|| {
            Error::from_reason(format!(
                "fused trampoline declares unknown ring `{ring_tag}`"
            ))
        })?;
        if !seen_rings.insert(ring_kind) {
            return Err(Error::from_reason(format!(
                "fused trampoline declares ring {ring_tag} more than once"
            )));
        }
        let hooks = ring
            .get_named_property::<Vec<Object>>("hooks")
            .map_err(|_| {
                Error::from_reason(format!("ring {ring_tag} declares no `hooks` array"))
            })?;
        let hook_count: u32 = ring
            .get_named_property("hookCount")
            .map_err(|_| Error::from_reason(format!("ring {ring_tag} declares no `hookCount`")))?;
        if hooks.len() as u32 != hook_count {
            return Err(Error::from_reason(format!(
                "ring {ring_tag} declares hookCount {hook_count} but {} hook entries",
                hooks.len()
            )));
        }
        let mut converted = Vec::new();
        let mut seen_ordinals = HashSet::new();
        for hook in hooks {
            let ordinal: u32 = hook.get_named_property("ordinal").map_err(|_| {
                Error::from_reason(format!(
                    "ring {ring_tag} has a hook entry with no `ordinal`"
                ))
            })?;
            if !seen_ordinals.insert(ordinal) {
                return Err(Error::from_reason(format!(
                    "ring {ring_tag} has duplicate hook ordinal {ordinal}"
                )));
            }
            let stage_tag: String = hook.get_named_property("stage").map_err(|_| {
                Error::from_reason(format!(
                    "ring {ring_tag} hook {ordinal} declares no `stage`"
                ))
            })?;
            let stage = HookStage::from_tag(&stage_tag).ok_or_else(|| {
                Error::from_reason(format!(
                    "ring {ring_tag} hook {ordinal} declares unknown stage `{stage_tag}`"
                ))
            })?;
            let plugin_name: Option<String> = hook
                .get_named_property::<Option<String>>("pluginName")
                .unwrap_or(None);
            let custom_action_name: Option<String> = hook
                .get_named_property::<Option<String>>("customActionName")
                .unwrap_or(None);
            let vsock_kind: Option<u32> = hook
                .get_named_property::<Option<u32>>("vsockKind")
                .unwrap_or(None);
            let lifecycle_tag: Option<String> = hook
                .get_named_property::<Option<String>>("lifecycleTag")
                .unwrap_or(None);
            let filter: Option<Object> = hook
                .get_named_property::<Option<Object>>("filter")
                .unwrap_or(None);
            let filter_snapshot = match filter {
                Some(filter) => {
                    let binary: Option<String> = filter
                        .get_named_property::<Option<String>>("binary")
                        .unwrap_or(None);
                    let pattern: Option<String> = filter
                        .get_named_property::<Option<String>>("pattern")
                        .unwrap_or(None);
                    let args: Vec<String> = filter
                        .get_named_property::<Vec<String>>("args")
                        .unwrap_or_default();
                    Some(FilterSnapshot {
                        binary,
                        args,
                        pattern,
                    })
                }
                None => None,
            };
            let entry = HookEntry {
                ordinal,
                stage,
                plugin_name,
                filter: filter_snapshot,
                lifecycle_tag,
                custom_action_name,
                vsock_kind,
            };
            validate_hook(ring_kind, &entry).map_err(Error::from_reason)?;
            converted.push(entry);
        }
        descriptor.rings.push(RingChain {
            ring: ring_kind,
            hooks: converted,
        });
    }
    Ok(descriptor)
}
