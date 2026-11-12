// see: cans/02-plugin-model.md#Unified-interposition-pipeline
use super::*;

impl PipelineEngine {
    // see: cans/08-lifecycle.md#Lifecycle-vetoes
    pub async fn dispatch_lifecycle(
        &self,
        event: LifecycleEvent,
        ctx: &mut LifecycleCtx<'_>,
    ) -> Result<(), PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::Lifecycle)?;
        for participant in &self.lifecycle_interceptors {
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let outcome = quarantine_unit(async {
                match event {
                    LifecycleEvent::VmInit => participant.on_vm_init(&mut *ctx).await,
                    LifecycleEvent::VmReady => participant.on_vm_ready(&mut *ctx).await,
                    LifecycleEvent::VmShutdown => participant.on_vm_shutdown(&mut *ctx).await,
                    LifecycleEvent::VmPreempt => participant.on_host_preempt(&mut *ctx).await,
                }
            })
            .await;
            if fused.is_none() {
                self.foreign_submit_complete();
            }
            outcome?;
        }
        drop(fused);
        Ok(())
    }

    // see: cans/08-lifecycle.md#Pre-teardown-flush
    pub async fn dispatch_pre_commit(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::Lifecycle)?;
        for participant in &self.lifecycle_interceptors {
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let outcome = quarantine_unit(participant.on_pre_commit(&mut *ctx)).await;
            if fused.is_none() {
                self.foreign_submit_complete();
            }
            outcome?;
        }
        drop(fused);
        Ok(())
    }

    // see: cans/08-lifecycle.md#Pre-teardown-flush
    pub async fn dispatch_post_commit(
        &self,
        ctx: &mut LifecycleCtx<'_>,
        committed: bool,
    ) -> Result<(), PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::Lifecycle)?;
        for participant in &self.lifecycle_interceptors {
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let outcome = quarantine_unit(participant.on_post_commit(&mut *ctx, committed)).await;
            if fused.is_none() {
                self.foreign_submit_complete();
            }
            outcome?;
        }
        drop(fused);
        Ok(())
    }

    // TODO (L3) [telemetry_audit]: no TrampolineRing tag exists for memory-pressure telemetry, so fused crossings cannot bind this ring yet; slop: telemetry ring bound without tag, crossings unattributed
    // see: cans/14-observability.md#Metrics-pipeline
    pub async fn dispatch_memory_pressure(
        &self,
        available_mb: u32,
        total_mb: u32,
    ) -> Result<(), PluginError> {
        for participant in &self.telemetry_interceptors {
            let outcome =
                quarantine_unit(participant.on_memory_pressure(available_mb, total_mb)).await;
            outcome?;
        }
        Ok(())
    }

    // see: cans/14-observability.md#Metrics-pipeline
    pub async fn dispatch_heartbeat(&self, event: &Heartbeat) -> Result<(), PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::Heartbeat)?;
        for participant in &self.telemetry_interceptors {
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let outcome = quarantine_unit(participant.on_heartbeat(event)).await;
            if fused.is_none() {
                self.foreign_submit_complete();
            }
            outcome?;
        }
        drop(fused);
        Ok(())
    }

    // see: cans/10-attestation.md#Step-evaluation-loop
    // TODO (L3) [lc_03_step_evaluation_taint_rollback]: the step-evaluation loop calls this per committed step per cans/10-attestation.md#Step-evaluation-loop; slop: step loop runs without per-step evaluation, taint unrevoked
    pub async fn dispatch_post_exec(
        &self,
        event: &TrapEvent,
        ctx: &HandlerCtx,
    ) -> Result<StepVerdict, PluginError> {
        for gate in &self.post_exec_gates {
            let action = quarantine_unit(gate.check(event, ctx)).await?;
            if action != PostExecAction::Proceed {
                return Ok(action.to_step_verdict());
            }
        }
        Ok(StepVerdict::Commit)
    }

    // see: cans/08-lifecycle.md#Watchdog
    pub async fn dispatch_watchdog(
        &self,
        event: &WatchdogEvent,
    ) -> Result<WatchdogVerdict, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::Watchdog)?;
        let mut verdict = WatchdogVerdict::ExecuteDefault;
        for participant in &self.telemetry_interceptors {
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let outcome = Quarantined::new(participant.on_watchdog(event)).await;
            if fused.is_none() {
                self.foreign_submit_complete();
            }
            match outcome? {
                Ok(WatchdogVerdict::ExecuteDefault) => {}
                Ok(WatchdogVerdict::Suppress) => return Ok(WatchdogVerdict::Suppress),
                Ok(WatchdogVerdict::Mitigate) => verdict = WatchdogVerdict::Mitigate,
                Err(err) => return Err(err),
            }
        }
        drop(fused);
        Ok(verdict)
    }
}
