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
    chain: Vec<(u32, ThreadsafeFunction<(String, u32)>)>,
}

impl NapiFusedCallback {
    // see: cans/02-plugin-model.md#Foreign-plugin-proxy
    pub fn new(chain: Vec<(u32, JsFunction)>) -> Result<Self> {
        let mut fused = Vec::with_capacity(chain.len());
        for (ordinal, function) in chain {
            let call = function.create_threadsafe_function(
                0,
                |context: ThreadSafeCallContext<(String, u32)>| {
                    Ok(vec![context.value.0, context.value.1.to_string()])
                },
            )?;
            fused.push((ordinal, call));
        }
        Ok(Self { chain: fused })
    }

    // see: cans/02-plugin-model.md#Foreign-plugin-proxy
    // TODO (L3) [bt_03_typescript_fused_trampoline_single_crossing_chained_hooks]: compile the fused chain into the single flat closure so the traversal enqueues exactly once per ring event and marshal the ring event payload into that single crossing; slop: chain still enqueues per ordinal, ring event payload stays unmarshaled
    fn marshal_chain_event(
        &self,
        _ring: TrampolineRing,
    ) -> std::result::Result<Verdict, PluginError> {
        Ok(Verdict::Pass)
    }

    // see: cans/02-plugin-model.md#Foreign-plugin-proxy
    // TODO (L3) [bt_03_typescript_fused_trampoline_single_crossing_chained_hooks]: copy back the mutable Uint8Array Buffer TS write hooks mutate during the traversal and rendezvous the enqueued run's Verdict into the fold; slop: JS hook runs fire-and-forget, its verdict never gates dispatch
    fn copyback_chain_buffers(
        &self,
        _ring: TrampolineRing,
        _ordinal: u32,
    ) -> std::result::Result<Verdict, PluginError> {
        Ok(Verdict::Pass)
    }
}

impl FusedTrampolineCallback for NapiFusedCallback {
    // see: cans/02-plugin-model.md#Foreign-plugin-proxy
    fn run(
        &self,
        ring: TrampolineRing,
        _ordinal: u32,
    ) -> std::result::Result<Verdict, PluginError> {
        self.marshal_chain_event(ring)?;
        let mut verdict = Verdict::Pass;
        for (hook_ordinal, call) in &self.chain {
            let status = call.call(
                Ok((ring.as_tag().to_string(), *hook_ordinal)),
                ThreadsafeFunctionCallMode::NonBlocking,
            );
            if status != Status::Ok {
                return Err(PluginError::Halt(format!(
                    "fused trampoline single crossing to ring {} failed at ordinal {hook_ordinal}: {status:?}",
                    ring.as_tag()
                )));
            }
            verdict = self.copyback_chain_buffers(ring, *hook_ordinal)?;
            if !matches!(verdict, Verdict::Pass) {
                break;
            }
        }
        Ok(verdict)
    }
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub fn attach_runs(descriptor: &mut FusedTrampolineDescriptor, runs: Vec<Object>) -> Result<()> {
    let mut grouped: Vec<(TrampolineRing, Vec<(u32, JsFunction)>)> = Vec::new();
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
        match grouped.iter().position(|(ring, _)| *ring == ring_kind) {
            Some(index) => {
                if grouped[index].1.iter().any(|(known, _)| *known == ordinal) {
                    return Err(Error::from_reason(format!(
                        "fused trampoline declares duplicate run ordinal {ordinal} on ring {ring_tag}"
                    )));
                }
                grouped[index].1.push((ordinal, function));
            }
            None => grouped.push((ring_kind, vec![(ordinal, function)])),
        }
    }
    for (ring_kind, mut fused) in grouped {
        let Some(chain) = descriptor
            .rings
            .iter_mut()
            .find(|chain| chain.ring == ring_kind)
        else {
            return Err(Error::from_reason(format!(
                "fused trampoline run on ring {} has no matching descriptor ring",
                ring_kind.as_tag()
            )));
        };
        fused.sort_by_key(|(ordinal, _)| *ordinal);
        for (ordinal, _) in &fused {
            if !chain.hooks.iter().any(|hook| hook.ordinal == *ordinal) {
                return Err(Error::from_reason(format!(
                    "fused trampoline run on ring {} ordinal {ordinal} has no matching descriptor hook",
                    ring_kind.as_tag()
                )));
            }
        }
        let head = fused[0].0;
        let dispatcher = Arc::new(NapiFusedCallback::new(fused)?);
        if let Some(hook) = chain.hooks.iter_mut().find(|hook| hook.ordinal == head) {
            hook.run = Some(dispatcher);
        }
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
                        cwd: None,
                        env: Vec::new(),
                        uid: None,
                        argv: None,
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
                priority: 0,
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
