// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/03-sdk-architecture.md#Python-SDK
use std::collections::HashSet;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

// see: cans/02-plugin-model.md#Segregated-ring-traits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrampolineRing {
    Trap,
    TrapDeny,
    ProcessExit,
    FsLookup,
    FsRead,
    FsWrite,
    NetOutbound,
    NetInbound,
    ConsoleInput,
    ConsoleOutput,
    Heartbeat,
    Lifecycle,
    Watchdog,
    CustomAction,
    VsockService,
}

impl TrampolineRing {
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "trap" => Some(Self::Trap),
            "trap-deny" => Some(Self::TrapDeny),
            "process-exit" => Some(Self::ProcessExit),
            "fs-lookup" => Some(Self::FsLookup),
            "fs-read" => Some(Self::FsRead),
            "fs-write" => Some(Self::FsWrite),
            "net-outbound" => Some(Self::NetOutbound),
            "net-inbound" => Some(Self::NetInbound),
            "console-input" => Some(Self::ConsoleInput),
            "console-output" => Some(Self::ConsoleOutput),
            "heartbeat" => Some(Self::Heartbeat),
            "lifecycle" => Some(Self::Lifecycle),
            "watchdog" => Some(Self::Watchdog),
            "custom-action" => Some(Self::CustomAction),
            "vsock-service" => Some(Self::VsockService),
            _ => None,
        }
    }

    pub fn as_tag(&self) -> &'static str {
        match self {
            Self::Trap => "trap",
            Self::TrapDeny => "trap-deny",
            Self::ProcessExit => "process-exit",
            Self::FsLookup => "fs-lookup",
            Self::FsRead => "fs-read",
            Self::FsWrite => "fs-write",
            Self::NetOutbound => "net-outbound",
            Self::NetInbound => "net-inbound",
            Self::ConsoleInput => "console-input",
            Self::ConsoleOutput => "console-output",
            Self::Heartbeat => "heartbeat",
            Self::Lifecycle => "lifecycle",
            Self::Watchdog => "watchdog",
            Self::CustomAction => "custom-action",
            Self::VsockService => "vsock-service",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookStage {
    Pre,
    Post,
    Single,
}

impl HookStage {
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "pre" => Some(Self::Pre),
            "post" => Some(Self::Post),
            "single" => Some(Self::Single),
            _ => None,
        }
    }
}

// see: cans/02-plugin-model.md#Filter-before-FFI-gate
#[derive(Debug, Clone, Default)]
pub struct FilterSnapshot {
    pub binary: Option<String>,
    pub args: Vec<String>,
    pub pattern: Option<String>,
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
#[derive(Debug, Clone)]
pub struct HookEntry {
    pub ordinal: u32,
    pub stage: HookStage,
    pub plugin_name: Option<String>,
    pub filter: Option<FilterSnapshot>,
    pub lifecycle_tag: Option<String>,
    pub custom_action_name: Option<String>,
    pub vsock_kind: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct RingChain {
    pub ring: TrampolineRing,
    pub hooks: Vec<HookEntry>,
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
#[derive(Debug, Clone, Default)]
pub struct FusedTrampolineDescriptor {
    pub rings: Vec<RingChain>,
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
#[derive(Debug, Clone)]
pub struct TrampolineEntry {
    pub ring: TrampolineRing,
    pub hook_count: u32,
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
#[derive(Debug, Clone, Default)]
pub struct TrampolineTable {
    entries: Vec<TrampolineEntry>,
}

impl TrampolineTable {
    pub fn from_descriptor(descriptor: &FusedTrampolineDescriptor) -> PyResult<Self> {
        let mut seen = HashSet::new();
        let mut entries = Vec::new();
        for chain in &descriptor.rings {
            if !seen.insert(chain.ring) {
                return Err(PyValueError::new_err(format!(
                    "fused trampoline declares ring {} more than once",
                    chain.ring.as_tag()
                )));
            }
            entries.push(TrampolineEntry {
                ring: chain.ring,
                hook_count: chain.hooks.len() as u32,
            });
        }
        Ok(Self { entries })
    }

    pub fn entry_count(&self) -> u32 {
        self.entries.len() as u32
    }
}

// see: cans/02-plugin-model.md#Hook-context-restriction
const LIFECYCLE_TAGS: &[&str] = &[
    "vm-init",
    "vm-ready",
    "vm-shutdown",
    "trap-deny",
    "pre-commit",
    "post-commit",
];

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
            if ring_kind == TrampolineRing::CustomAction
                && custom_action_name.as_deref().is_none_or(str::is_empty)
            {
                return Err(PyValueError::new_err(format!(
                    "custom-action hook {ordinal} declares no action name"
                )));
            }
            if ring_kind == TrampolineRing::VsockService {
                match vsock_kind {
                    Some(kind) if kind <= 0xFFFF => {}
                    _ => {
                        return Err(PyValueError::new_err(format!(
                            "vsock-service hook {ordinal} declares kind outside u16"
                        )));
                    }
                }
            }
            if ring_kind == TrampolineRing::Lifecycle {
                match lifecycle_tag.as_deref() {
                    Some(tag) if LIFECYCLE_TAGS.contains(&tag) => {}
                    _ => {
                        return Err(PyValueError::new_err(format!(
                            "lifecycle hook {ordinal} declares unknown tag `{lifecycle_tag:?}`"
                        )));
                    }
                }
            }
            converted.push(HookEntry {
                ordinal,
                stage,
                plugin_name,
                filter: filter_snapshot,
                lifecycle_tag,
                custom_action_name,
                vsock_kind,
            });
        }
        out.rings.push(RingChain {
            ring: ring_kind,
            hooks: converted,
        });
    }
    Ok(out)
}
