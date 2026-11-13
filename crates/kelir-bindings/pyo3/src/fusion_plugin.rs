// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/03-sdk-architecture.md#Python-SDK
use std::collections::HashSet;
use std::sync::Arc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use kelir_interface::fusion_plugin::FusedTrampolineCallback;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::Verdict;

pub use kelir_interface::fusion_plugin::{
    FilterSnapshot, FusedTrampolineDescriptor, HookEntry, HookStage, RingChain, TrampolineRing,
    TrampolineTable, validate_hook,
};

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub struct PyFusedCallback {
    callable: Py<PyAny>,
}

impl PyFusedCallback {
    pub fn new(callable: Py<PyAny>) -> Self {
        Self { callable }
    }
}

impl FusedTrampolineCallback for PyFusedCallback {
    fn run(&self, ring: TrampolineRing, ordinal: u32) -> Result<Verdict, PluginError> {
        let outcome = Python::with_gil(|py| {
            self.callable
                .bind(py)
                .call1((ring.as_tag(), ordinal))
                .map(|_| ())
        });
        outcome.map_err(|error| {
            PluginError::Halt(format!(
                "fused trampoline callable on ring {} ordinal {ordinal} failed: {error}",
                ring.as_tag()
            ))
        })?;
        // TODO (L3) [bp_03_python_fused_trampoline_single_crossing_chained_hooks]: map the callable return value into a Verdict and marshal the ring event payload across the GIL boundary; slop: Python hook runs fire-and-forget, its verdict never gates dispatch
        Ok(Verdict::Pass)
    }
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub fn attach_runs(
    descriptor: &mut FusedTrampolineDescriptor,
    runs: &Bound<'_, PyList>,
) -> PyResult<()> {
    for run_value in runs.iter() {
        let run_entry = run_value.downcast::<PyDict>()?;
        let ring_tag = required_str(run_entry, "ring", "run entry")?;
        let ring_kind = TrampolineRing::from_tag(&ring_tag).ok_or_else(|| {
            PyValueError::new_err(format!(
                "fused trampoline run declares unknown ring `{ring_tag}`"
            ))
        })?;
        let ordinal: u32 = run_entry
            .get_item("ordinal")?
            .ok_or_else(|| {
                PyValueError::new_err(format!(
                    "fused trampoline run on ring {ring_tag} declares no `ordinal`"
                ))
            })?
            .extract()?;
        let callable = match run_entry.get_item("run")? {
            Some(value) if !value.is_none() && value.is_callable() => value.unbind(),
            _ => {
                return Err(PyValueError::new_err(format!(
                    "fused trampoline run on ring {ring_tag} ordinal {ordinal} declares no callable `run`"
                )));
            }
        };
        let callback = Arc::new(PyFusedCallback::new(callable));
        let Some(chain) = descriptor
            .rings
            .iter_mut()
            .find(|chain| chain.ring == ring_kind)
        else {
            return Err(PyValueError::new_err(format!(
                "fused trampoline run on ring {ring_tag} ordinal {ordinal} has no matching descriptor hook"
            )));
        };
        let Some(hook) = chain.hooks.iter_mut().find(|hook| hook.ordinal == ordinal) else {
            return Err(PyValueError::new_err(format!(
                "fused trampoline run on ring {ring_tag} ordinal {ordinal} has no matching descriptor hook"
            )));
        };
        hook.run = Some(callback);
    }
    Ok(())
}

fn optional_str(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    match dict.get_item(key)? {
        Some(value) if !value.is_none() => Ok(Some(value.extract()?)),
        _ => Ok(None),
    }
}

fn optional_u32(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<u32>> {
    match dict.get_item(key)? {
        Some(value) if !value.is_none() => Ok(Some(value.extract()?)),
        _ => Ok(None),
    }
}

fn required_str(dict: &Bound<'_, PyDict>, key: &str, what: &str) -> PyResult<String> {
    optional_str(dict, key)?.ok_or_else(|| {
        PyValueError::new_err(format!("fused trampoline {what} declares no `{key}`"))
    })
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub fn convert_descriptor(descriptor: &Bound<'_, PyDict>) -> PyResult<FusedTrampolineDescriptor> {
    let rings_value = descriptor
        .get_item("rings")?
        .ok_or_else(|| PyValueError::new_err("fused trampoline declares no `rings` list"))?;
    let rings = rings_value.downcast::<PyList>()?;
    let mut out = FusedTrampolineDescriptor::default();
    let mut seen_rings = HashSet::new();
    for ring_value in rings.iter() {
        let ring = ring_value.downcast::<PyDict>()?;
        let ring_tag = required_str(ring, "ring", "ring entry")?;
        let ring_kind = TrampolineRing::from_tag(&ring_tag).ok_or_else(|| {
            PyValueError::new_err(format!(
                "fused trampoline declares unknown ring `{ring_tag}`"
            ))
        })?;
        if !seen_rings.insert(ring_kind) {
            return Err(PyValueError::new_err(format!(
                "fused trampoline declares ring {ring_tag} more than once"
            )));
        }
        let hooks_value = ring.get_item("hooks")?.ok_or_else(|| {
            PyValueError::new_err(format!("ring {ring_tag} declares no `hooks` list"))
        })?;
        let hooks = hooks_value.downcast::<PyList>()?;
        let hook_count: u32 = ring
            .get_item("hook_count")?
            .ok_or_else(|| {
                PyValueError::new_err(format!("ring {ring_tag} declares no `hook_count`"))
            })?
            .extract()?;
        if hooks.len() as u32 != hook_count {
            return Err(PyValueError::new_err(format!(
                "ring {ring_tag} declares hook_count {hook_count} but {} hook entries",
                hooks.len()
            )));
        }
        let mut converted = Vec::new();
        let mut seen_ordinals = HashSet::new();
        for hook_value in hooks.iter() {
            let hook = hook_value.downcast::<PyDict>()?;
            let ordinal: u32 = hook
                .get_item("ordinal")?
                .ok_or_else(|| {
                    PyValueError::new_err(format!(
                        "ring {ring_tag} has a hook entry with no `ordinal`"
                    ))
                })?
                .extract()?;
            if !seen_ordinals.insert(ordinal) {
                return Err(PyValueError::new_err(format!(
                    "ring {ring_tag} has duplicate hook ordinal {ordinal}"
                )));
            }
            let stage_tag = required_str(hook, "stage", &format!("hook {ordinal}"))?;
            let stage = HookStage::from_tag(&stage_tag).ok_or_else(|| {
                PyValueError::new_err(format!(
                    "ring {ring_tag} hook {ordinal} declares unknown stage `{stage_tag}`"
                ))
            })?;
            let plugin_name = optional_str(hook, "plugin_name")?;
            let custom_action_name = optional_str(hook, "custom_action_name")?;
            let vsock_kind = optional_u32(hook, "vsock_kind")?;
            let lifecycle_tag = optional_str(hook, "lifecycle_tag")?;
            let filter_snapshot = match hook.get_item("filter")? {
                Some(filter) if !filter.is_none() => {
                    let filter = filter.downcast::<PyDict>()?;
                    Some(FilterSnapshot {
                        binary: optional_str(filter, "binary")?,
                        args: match filter.get_item("args")? {
                            Some(args) if !args.is_none() => args.extract()?,
                            _ => Vec::new(),
                        },
                        pattern: optional_str(filter, "pattern")?,
                    })
                }
                _ => None,
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
            validate_hook(ring_kind, &entry).map_err(PyValueError::new_err)?;
            converted.push(entry);
        }
        out.rings.push(RingChain {
            ring: ring_kind,
            hooks: converted,
        });
    }
    Ok(out)
}
