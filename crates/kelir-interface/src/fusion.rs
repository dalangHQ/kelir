// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/03-sdk-architecture.md#Python-SDK
use std::collections::HashSet;

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
    pub fn from_descriptor(descriptor: &FusedTrampolineDescriptor) -> Result<Self, String> {
        let mut seen = HashSet::new();
        let mut entries = Vec::new();
        for chain in &descriptor.rings {
            if !seen.insert(chain.ring) {
                return Err(format!(
                    "fused trampoline declares ring {} more than once",
                    chain.ring.as_tag()
                ));
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

pub fn validate_hook(ring: TrampolineRing, hook: &HookEntry) -> Result<(), String> {
    if ring == TrampolineRing::CustomAction
        && hook.custom_action_name.as_deref().is_none_or(str::is_empty)
    {
        return Err(format!(
            "custom-action hook {} declares no action name",
            hook.ordinal
        ));
    }
    if ring == TrampolineRing::VsockService {
        match hook.vsock_kind {
            Some(kind) if kind <= 0xFFFF => {}
            _ => {
                return Err(format!(
                    "vsock-service hook {} declares kind outside u16",
                    hook.ordinal
                ));
            }
        }
    }
    if ring == TrampolineRing::Lifecycle {
        match hook.lifecycle_tag.as_deref() {
            Some(tag) if LIFECYCLE_TAGS.contains(&tag) => {}
            _ => {
                return Err(format!(
                    "lifecycle hook {} declares unknown tag `{:?}`",
                    hook.ordinal, hook.lifecycle_tag
                ));
            }
        }
    }
    Ok(())
}
