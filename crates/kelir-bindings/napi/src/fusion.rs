// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/03-sdk-architecture.md#TypeScript-SDK
use std::collections::HashSet;

use napi::bindgen_prelude::Object;
use napi::{Error, Result};

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
    pub fn from_descriptor(descriptor: &FusedTrampolineDescriptor) -> Result<Self> {
        let mut seen = HashSet::new();
        let mut entries = Vec::new();
        for chain in &descriptor.rings {
            if !seen.insert(chain.ring) {
                return Err(Error::from_reason(format!(
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

    pub fn entry_for(&self, ring: TrampolineRing) -> Option<&TrampolineEntry> {
        self.entries.iter().find(|entry| entry.ring == ring)
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
            if ring_kind == TrampolineRing::CustomAction
                && custom_action_name.as_deref().is_none_or(str::is_empty)
            {
                return Err(Error::from_reason(format!(
                    "custom-action hook {ordinal} declares no action name"
                )));
            }
            if ring_kind == TrampolineRing::VsockService {
                match vsock_kind {
                    Some(kind) if kind <= 0xFFFF => {}
                    _ => {
                        return Err(Error::from_reason(format!(
                            "vsock-service hook {ordinal} declares kind outside u16"
                        )));
                    }
                }
            }
            if ring_kind == TrampolineRing::Lifecycle {
                match lifecycle_tag.as_deref() {
                    Some(tag) if LIFECYCLE_TAGS.contains(&tag) => {}
                    _ => {
                        return Err(Error::from_reason(format!(
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
        descriptor.rings.push(RingChain {
            ring: ring_kind,
            hooks: converted,
        });
    }
    Ok(descriptor)
}
