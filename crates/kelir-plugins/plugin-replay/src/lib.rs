// see: cans/plugins/replay.md
#![forbid(unsafe_code)]

use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope};

#[derive(Clone)]
pub struct ReplayService;

#[derive(Clone)]
pub struct ReplayPlugin {
    pub service: ReplayService,
}

impl ReplayPlugin {
    pub fn new() -> Self {
        Self {
            service: ReplayService,
        }
    }
}

impl Default for ReplayPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for ReplayPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "replay"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl LifecycleParticipant for ReplayPlugin {}
