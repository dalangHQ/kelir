// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/03-sdk-architecture.md#Python-SDK
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use crate::plugin_model::PluginError;
use crate::protocol_wire::Verdict;

// see: cans/02-plugin-model.md#Segregated-ring-traits
macro_rules! tag_enum {
    ($(#[$attr:meta])* $vis:vis enum $name:ident { $($variant:ident => $tag:literal),+ $(,)? }) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $name {
            $($variant),+
        }

        impl $name {
            pub fn from_tag(tag: &str) -> Option<Self> {
                match tag {
                    $($tag => Some(Self::$variant),)+
                    _ => None,
                }
            }

            pub fn as_tag(&self) -> &'static str {
                match self {
                    $(Self::$variant => $tag,)+
                }
            }
        }
    };
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
tag_enum! {
    pub enum TrampolineRing {
        Trap => "trap",
        TrapDeny => "trap-deny",
        ProcessExit => "process-exit",
        FsLookup => "fs-lookup",
        FsRead => "fs-read",
        FsWrite => "fs-write",
        NetOutbound => "net-outbound",
        NetInbound => "net-inbound",
        ConsoleInput => "console-input",
        ConsoleOutput => "console-output",
        ExecStdout => "exec-stdout",
        ExecStderr => "exec-stderr",
        Heartbeat => "heartbeat",
        Lifecycle => "lifecycle",
        Watchdog => "watchdog",
        CustomAction => "custom-action",
        VsockService => "vsock-service",
        FsLock => "fs-lock",
        HostLsmExec => "host-lsm-exec",
        HostPressure => "host-pressure",
        HostKeyringAccess => "host-keyring-access",
        HostPreempt => "host-preempt",
        MemoryPressure => "memory-pressure",
    }
}

tag_enum! {
    pub enum HookStage {
        Pre => "pre",
        Post => "post",
        Single => "single",
    }
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub trait FusedTrampolineCallback: Send + Sync + 'static {
    fn run(&self, ring: TrampolineRing, ordinal: u32) -> Result<Verdict, PluginError>;
}

// see: cans/02-plugin-model.md#Filter-before-FFI-gate
#[derive(Debug, Clone, Default)]
pub struct FilterSnapshot {
    pub binary: Option<String>,
    pub args: Vec<String>,
    pub pattern: Option<String>,
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
#[derive(Clone)]
pub struct HookEntry {
    pub ordinal: u32,
    pub stage: HookStage,
    pub plugin_name: Option<String>,
    pub filter: Option<FilterSnapshot>,
    pub lifecycle_tag: Option<String>,
    pub custom_action_name: Option<String>,
    pub vsock_kind: Option<u32>,
    pub run: Option<Arc<dyn FusedTrampolineCallback>>,
}

impl fmt::Debug for HookEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HookEntry")
            .field("ordinal", &self.ordinal)
            .field("stage", &self.stage)
            .field("plugin_name", &self.plugin_name)
            .field("filter", &self.filter)
            .field("lifecycle_tag", &self.lifecycle_tag)
            .field("custom_action_name", &self.custom_action_name)
            .field("vsock_kind", &self.vsock_kind)
            .field("run", &self.run.as_ref().map(|_| "FusedTrampolineCallback"))
            .finish()
    }
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
pub const LIFECYCLE_TAGS: &[&str] = &[
    "vm-init",
    "vm-ready",
    "vm-shutdown",
    "trap-deny",
    "pre-commit",
    "post-commit",
];

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
#[derive(Debug, Clone, Copy)]
pub struct HookRingBinding {
    pub method: &'static str,
    pub ring: TrampolineRing,
    pub lifecycle_tag: Option<&'static str>,
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub const HOOK_RINGS: &[HookRingBinding] = &[
    HookRingBinding {
        method: "on_trap",
        ring: TrampolineRing::Trap,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_fs_lookup",
        ring: TrampolineRing::FsLookup,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_fs_read",
        ring: TrampolineRing::FsRead,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_fs_write",
        ring: TrampolineRing::FsWrite,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_net_outbound",
        ring: TrampolineRing::NetOutbound,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_console_input",
        ring: TrampolineRing::ConsoleInput,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_console_output",
        ring: TrampolineRing::ConsoleOutput,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_exec_stdout",
        ring: TrampolineRing::ExecStdout,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_exec_stderr",
        ring: TrampolineRing::ExecStderr,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_heartbeat",
        ring: TrampolineRing::Heartbeat,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_process_exit",
        ring: TrampolineRing::ProcessExit,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_vm_init",
        ring: TrampolineRing::Lifecycle,
        lifecycle_tag: Some("vm-init"),
    },
    HookRingBinding {
        method: "on_vm_ready",
        ring: TrampolineRing::Lifecycle,
        lifecycle_tag: Some("vm-ready"),
    },
    HookRingBinding {
        method: "on_vm_shutdown",
        ring: TrampolineRing::Lifecycle,
        lifecycle_tag: Some("vm-shutdown"),
    },
    HookRingBinding {
        method: "on_watchdog",
        ring: TrampolineRing::Watchdog,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_fs_lock",
        ring: TrampolineRing::FsLock,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_net_inbound",
        ring: TrampolineRing::NetInbound,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_host_lsm_exec",
        ring: TrampolineRing::HostLsmExec,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_host_pressure",
        ring: TrampolineRing::HostPressure,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_host_keyring_access",
        ring: TrampolineRing::HostKeyringAccess,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_pre_commit",
        ring: TrampolineRing::Lifecycle,
        lifecycle_tag: Some("pre-commit"),
    },
    HookRingBinding {
        method: "on_post_commit",
        ring: TrampolineRing::Lifecycle,
        lifecycle_tag: Some("post-commit"),
    },
    HookRingBinding {
        method: "on_host_preempt",
        ring: TrampolineRing::HostPreempt,
        lifecycle_tag: None,
    },
    HookRingBinding {
        method: "on_memory_pressure",
        ring: TrampolineRing::MemoryPressure,
        lifecycle_tag: None,
    },
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
