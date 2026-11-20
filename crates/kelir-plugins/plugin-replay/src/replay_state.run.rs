// see: cans/plugins/replay.md#Plugin-replay
use async_trait::async_trait;
use kelir_interface::plugin_model::{LifecycleCtx, LifecycleParticipant, PluginError};

use crate::replay_state::ReplayPlugin;

// see: cans/plugins/replay.md#Invariants-and-policy-(replay)
#[async_trait]
impl LifecycleParticipant for ReplayPlugin {
    async fn on_vm_init(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        let _ = ctx;
        // TODO (L3) [lifecycle_checkpoint]: validate the replay preconditions, install the seeded entropy backend and PMU sampler, and capture the seed in the snapshot manifest.; slop: replay runs on unseeded entropy, runs are not reproducible, the snapshot manifest records no seed.
        Err(PluginError::Halt(
            "TODO (L3): validate the replay preconditions, install the seeded entropy backend and PMU sampler".into(),
        ))
    }

    async fn on_vm_ready(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        let _ = ctx;
        // TODO (L3) [lifecycle_checkpoint]: save the RNG stream position.; slop: the RNG stream position is never saved, rollback resumes from the wrong stream position, replay diverges silently.
        Err(PluginError::Halt(
            "TODO (L3): save the RNG stream position".into(),
        ))
    }

    async fn on_host_preempt(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        let _ = ctx;
        // TODO (L3) [lifecycle_checkpoint]: step the due step windows.; slop: due step windows never step, replay stalls with no progress while reporting success.
        Err(PluginError::Halt(
            "TODO (L3): step the due step windows".into(),
        ))
    }

    async fn on_pre_commit(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        let _ = ctx;
        // TODO (L3) [lifecycle_checkpoint]: step the due step windows and save the RNG stream position.; slop: due step windows never step and the RNG position is never saved, replay stalls and rollback diverges.
        Err(PluginError::Halt(
            "TODO (L3): step the due step windows and save the RNG stream position".into(),
        ))
    }

    async fn on_post_commit(
        &self,
        ctx: &mut LifecycleCtx<'_>,
        committed: bool,
    ) -> Result<(), PluginError> {
        let _ = (ctx, committed);
        // TODO (L3) [lifecycle_checkpoint]: restore the RNG stream position on rollback.; slop: rollback restores nothing, the RNG stream resumes from the wrong position, replayed execution diverges from the recorded run.
        Err(PluginError::Halt(
            "TODO (L3): restore the RNG stream position on rollback".into(),
        ))
    }
}
