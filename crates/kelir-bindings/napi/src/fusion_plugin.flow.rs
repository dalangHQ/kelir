// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/03-sdk-architecture.md#TypeScript-SDK
use std::collections::HashSet;
use std::sync::Arc;

use napi::bindgen_prelude::Object;
use napi::threadsafe_function::{
    ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use napi::{Error, JsFunction, Result, Status};

use kelir_interface::fusion_plugin::FusedTrampolineCallback;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::Verdict;

pub use kelir_interface::fusion_plugin::{
    FilterSnapshot, FusedTrampolineDescriptor, HookEntry, HookStage, RingChain, TrampolineRing,
    TrampolineTable, validate_hook,
};

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub struct NapiFusedCallback {
    call: ThreadsafeFunction<(String, u32)>,
}

impl NapiFusedCallback {
    pub fn new(function: JsFunction) -> Result<Self> {
        let call = function.create_threadsafe_function(
            0,
            |context: ThreadSafeCallContext<(String, u32)>| {
                Ok(vec![context.value.0, context.value.1.to_string()])
            },
        )?;
        Ok(Self { call })
    }
}

impl FusedTrampolineCallback for NapiFusedCallback {
    fn run(&self, ring: TrampolineRing, ordinal: u32) -> std::result::Result<Verdict, PluginError> {
        let status = self.call.call(
            Ok((ring.as_tag().to_string(), ordinal)),
            ThreadsafeFunctionCallMode::NonBlocking,
        );
        if status != Status::Ok {
            return Err(PluginError::Halt(format!(
                "fused trampoline threadsafe call to ring {} ordinal {ordinal} failed: {status:?}",
                ring.as_tag()
            )));
        }
        // TODO (L3) [bt_03_typescript_fused_trampoline_single_crossing_chained_hooks]: rendezvous the JS return value into a Verdict and marshal the ring event payload across the threadsafe boundary; slop: JS hook runs fire-and-forget, its verdict never gates dispatch
        Ok(Verdict::Pass)
    }
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub fn attach_runs(descriptor: &mut FusedTrampolineDescriptor, runs: Vec<Object>) -> Result<()> {
    for run_entry in runs {
        let ring_tag: String = run_entry
            .get_named_property::<String>("ring")
            .map_err(|_| Error::from_reason("fused trampoline run entry declares no `ring` tag"))?;
        let ring_kind = TrampolineRing::from_tag(&ring_tag).ok_or_else(|| {
            Error::from_reason(format!(
                "fused trampoline run declares unknown ring `{ring_tag}`"
            ))
        })?;
        let ordinal: u32 = run_entry.get_named_property("ordinal").map_err(|_| {
            Error::from_reason(format!(
                "fused trampoline run on ring {ring_tag} declares no `ordinal`"
            ))
        })?;
        let function: JsFunction = run_entry.get_named_property("run").map_err(|_| {
            Error::from_reason(format!(
                "fused trampoline run on ring {ring_tag} ordinal {ordinal} declares no `run` function"
            ))
        })?;
        let callback = Arc::new(NapiFusedCallback::new(function)?);
        let Some(chain) = descriptor
            .rings
            .iter_mut()
            .find(|chain| chain.ring == ring_kind)
        else {
            return Err(Error::from_reason(format!(
                "fused trampoline run on ring {ring_tag} ordinal {ordinal} has no matching descriptor hook"
            )));
        };
        let Some(hook) = chain.hooks.iter_mut().find(|hook| hook.ordinal == ordinal) else {
            return Err(Error::from_reason(format!(
                "fused trampoline run on ring {ring_tag} ordinal {ordinal} has no matching descriptor hook"
            )));
        };
        hook.run = Some(callback);
    }
    Ok(())
}

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
                run: None,
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
