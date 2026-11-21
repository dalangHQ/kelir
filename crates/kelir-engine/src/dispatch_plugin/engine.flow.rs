// see: cans/02-plugin-model.md#Static-dispatch
// see: cans/02-plugin-model.md#Unified-interposition-pipeline
use super::*;
use kelir_interface::telemetry::MetricRegistry;

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub(super) struct FusedCrossing<'a> {
    engine: &'a PipelineEngine,
    verdict: Verdict,
}

impl FusedCrossing<'_> {
    pub(crate) fn non_pass_verdict(&self) -> Option<Verdict> {
        (!matches!(self.verdict, Verdict::Pass)).then(|| self.verdict.clone())
    }
}

impl Drop for FusedCrossing<'_> {
    fn drop(&mut self) {
        self.engine.foreign_submit_complete();
    }
}

// see: cans/02-plugin-model.md#Hook-ordering
#[derive(Clone, Copy, Default)]
pub(super) struct RingRegistration {
    pub(super) priority: i32,
    pub(super) timeout: Option<Duration>,
}

pub struct PipelineEngine {
    pub(super) trap_interceptors: Vec<Arc<dyn TrapInterposer>>,
    pub(super) trap_scopes: Vec<MountScope>,
    pub(super) trap_regs: Vec<RingRegistration>,
    pub(super) gate: TrapGateEvaluator,
    pub(super) fs_interceptors: Vec<Arc<dyn FsInterposer>>,
    pub(super) fs_scopes: Vec<MountScope>,
    pub(super) fs_regs: Vec<RingRegistration>,
    pub(super) net_interceptors: Vec<Arc<dyn NetInterposer>>,
    pub(super) net_scopes: Vec<MountScope>,
    pub(super) net_regs: Vec<RingRegistration>,
    pub(super) console_interceptors: Vec<Arc<dyn ConsoleInterposer>>,
    pub(super) console_scopes: Vec<MountScope>,
    pub(super) console_regs: Vec<RingRegistration>,
    pub(super) terminal: Option<Arc<crate::planes::terminal::TerminalPlane>>,
    pub(super) host_interceptors: Vec<Arc<dyn HostInterposer>>,
    pub(super) host_scopes: Vec<MountScope>,
    pub(super) host_regs: Vec<RingRegistration>,
    pub(super) vsock_services: Vec<(VsockServiceSpec, Arc<dyn VsockServiceHandler>, MountScope)>,
    pub(super) lifecycle_interceptors: Vec<Arc<dyn LifecycleParticipant>>,
    pub(super) lifecycle_scopes: Vec<MountScope>,
    pub(super) lifecycle_regs: Vec<RingRegistration>,
    pub(super) telemetry_interceptors: Vec<Arc<dyn SystemTelemetryParticipant>>,
    pub(super) telemetry_scopes: Vec<MountScope>,
    pub(super) telemetry_regs: Vec<RingRegistration>,
    pub(super) stream_sinks: Vec<(u16, Arc<dyn StreamFrameSink>, MountScope)>,
    pub(super) lock_broker: Option<Arc<dyn LockBrokerCoordination>>,
    pub(super) parked_reset: Option<Arc<dyn ParkedConnectionReset>>,
    pub(super) token_revoker: Option<Arc<dyn TokenRevocation>>,
    pub(super) metrics: Arc<MetricRegistry>,
    pub(super) trampoline: Option<kelir_interface::fusion_plugin::FusedTrampolineDescriptor>,
    pub(super) checkpoint: Option<Arc<CheckpointPlane>>,
    pub(super) on_trap_deny: Option<Arc<TrapDenyCallback>>,
    pub(super) custom_actions: Vec<Arc<CustomActionBinding>>,
    pub(super) receipt_store: ReceiptStore,
    pub(super) fused_callbacks: Vec<(TrampolineRing, u32, Arc<dyn FusedTrampolineCallback>)>,
    pub(super) foreign_queue: FfiDispatchQueue,
    pub(super) ffi_rate_per_sec: u32,
    pub(super) ffi_window_start_ms: AtomicU64,
    pub(super) ffi_window_ops: AtomicU64,
    pub(super) active_scope: Option<MountScope>,
    pub(super) sealed: bool,
}

impl Default for PipelineEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineBinder for PipelineEngine {
    fn bind_fs_scoped(&mut self, stage: Arc<dyn FsInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.fs_interceptors.push(stage);
        self.fs_scopes.push(scope);
        self.fs_regs.push(RingRegistration::default());
    }
    fn bind_net_scoped(&mut self, stage: Arc<dyn NetInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.net_interceptors.push(stage);
        self.net_scopes.push(scope);
        self.net_regs.push(RingRegistration::default());
    }
    fn bind_trap_scoped(&mut self, stage: Arc<dyn TrapInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.trap_interceptors.push(stage);
        self.trap_scopes.push(scope);
        self.trap_regs.push(RingRegistration::default());
    }
    fn bind_console_scoped(&mut self, stage: Arc<dyn ConsoleInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.console_interceptors.push(stage);
        self.console_scopes.push(scope);
        self.console_regs.push(RingRegistration::default());
    }
    fn bind_host_scoped(&mut self, stage: Arc<dyn HostInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.host_interceptors.push(stage);
        self.host_scopes.push(scope);
        self.host_regs.push(RingRegistration::default());
    }
    fn bind_vsock_service_scoped(
        &mut self,
        spec: VsockServiceSpec,
        service: Arc<dyn VsockServiceHandler>,
        scope: MountScope,
    ) {
        // TODO (L3) [plugin_engine]: scope is stored; dispatch-time enforcement lands with vsock dispatch; slop: scope stored never enforced at vsock dispatch
        let scope = self.effective_scope(scope);
        self.vsock_services.push((spec, service, scope));
    }
    // see: cans/02-plugin-model.md#Segregated-ring-traits
    // TODO (L3) [plugin_engine]: scopes are stored; dispatch-time enforcement for the lifecycle and telemetry rings lands with the veto gate; slop: lifecycle/telemetry rings bypass veto gate
    fn bind_lifecycle_scoped(&mut self, stage: Arc<dyn LifecycleParticipant>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.lifecycle_interceptors.push(stage);
        self.lifecycle_scopes.push(scope);
        self.lifecycle_regs.push(RingRegistration::default());
    }
    fn bind_telemetry_scoped(
        &mut self,
        stage: Arc<dyn SystemTelemetryParticipant>,
        scope: MountScope,
    ) {
        let scope = self.effective_scope(scope);
        self.telemetry_interceptors.push(stage);
        self.telemetry_scopes.push(scope);
        self.telemetry_regs.push(RingRegistration::default());
    }
    // see: cans/11-protocol.md#Kind-registration
    fn bind_stream_kind_scoped(
        &mut self,
        kind: u16,
        sink: Arc<dyn StreamFrameSink>,
        scope: MountScope,
    ) {
        let scope = self.effective_scope(scope);
        self.stream_sinks.push((kind, sink, scope));
    }
    fn bind_parked_connection_reset(&mut self, reset: Arc<dyn ParkedConnectionReset>) {
        self.parked_reset = Some(reset);
    }
    // see: cans/08-lifecycle.md#Rollback
    fn bind_token_revoker(&mut self, revoker: Arc<dyn TokenRevocation>) {
        self.token_revoker = Some(revoker);
    }
    fn set_active_scope(&mut self, scope: MountScope) {
        self.active_scope = Some(scope);
    }
}

impl PipelineEngine {
    pub fn new() -> Self {
        Self {
            trap_interceptors: Vec::new(),
            trap_scopes: Vec::new(),
            trap_regs: Vec::new(),
            gate: TrapGateEvaluator::new(
                crate::trap_gate::TRUSTED_PREFIXES
                    .iter()
                    .map(|prefix| (*prefix).to_string())
                    .collect(),
            ),
            fs_interceptors: Vec::new(),
            fs_scopes: Vec::new(),
            fs_regs: Vec::new(),
            net_interceptors: Vec::new(),
            net_scopes: Vec::new(),
            net_regs: Vec::new(),
            console_interceptors: Vec::new(),
            console_scopes: Vec::new(),
            console_regs: Vec::new(),
            terminal: None,
            host_interceptors: Vec::new(),
            host_scopes: Vec::new(),
            host_regs: Vec::new(),
            vsock_services: Vec::new(),
            lifecycle_interceptors: Vec::new(),
            lifecycle_scopes: Vec::new(),
            lifecycle_regs: Vec::new(),
            telemetry_interceptors: Vec::new(),
            telemetry_scopes: Vec::new(),
            telemetry_regs: Vec::new(),
            stream_sinks: Vec::new(),
            lock_broker: None,
            parked_reset: None,
            token_revoker: None,
            metrics: Arc::new(MetricRegistry::new()),
            trampoline: None,
            checkpoint: None,
            on_trap_deny: None,
            custom_actions: Vec::new(),
            receipt_store: ReceiptStore::new(),
            fused_callbacks: Vec::new(),
            foreign_queue: FfiDispatchQueue::default(),
            ffi_rate_per_sec: 0,
            ffi_window_start_ms: AtomicU64::new(0),
            ffi_window_ops: AtomicU64::new(0),
            active_scope: None,
            sealed: false,
        }
    }

    // see: cans/01-architecture.md#FFI-boundary-economics
    pub fn with_ffi_rate_limit(mut self, ops_per_sec: u32) -> Self {
        self.ffi_rate_per_sec = ops_per_sec;
        self
    }

    pub fn foreign_queue(&self) -> &FfiDispatchQueue {
        &self.foreign_queue
    }

    // see: cans/01-architecture.md#FFI-boundary-economics
    // see: cans/02-plugin-model.md#Foreign-plugin-proxy
    pub fn try_foreign_submit(&self) -> Result<(), PluginError> {
        if self.ffi_rate_per_sec != 0 {
            self.ffi_rate_window_check()?;
        }
        self.foreign_queue.try_submit()
    }

    // see: cans/01-architecture.md#FFI-boundary-economics
    fn ffi_rate_window_check(&self) -> Result<(), PluginError> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or(0);
        let start_ms = self.ffi_window_start_ms.load(Ordering::Acquire);
        if now_ms.saturating_sub(start_ms) >= 1000 {
            self.ffi_window_start_ms.store(now_ms, Ordering::Release);
            self.ffi_window_ops.store(1, Ordering::Release);
            return Ok(());
        }
        // TODO (L3) [plugin_engine]: exact token-bucket accounting; the 1s window below is the pre-insertion rate gate; slop: rate gate counts window only, bucket never exact
        let ops = self.ffi_window_ops.fetch_add(1, Ordering::AcqRel);
        if ops >= u64::from(self.ffi_rate_per_sec) {
            return Err(PluginError::Halt(format!(
                "FFI rate limit exceeded: {} ops/s native cap before queue insertion",
                self.ffi_rate_per_sec
            )));
        }
        Ok(())
    }

    pub fn foreign_submit_complete(&self) {
        self.foreign_queue.complete();
    }

    pub fn with_metrics(mut self, metrics: Arc<MetricRegistry>) -> Self {
        self.metrics = metrics;
        self
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    pub fn with_terminal(mut self, terminal: Arc<crate::planes::terminal::TerminalPlane>) -> Self {
        self.terminal = Some(terminal);
        self
    }

    pub fn metrics(&self) -> &Arc<MetricRegistry> {
        &self.metrics
    }

    // see: cans/02-plugin-model.md#Foreign-plugin-proxy
    pub fn with_trampoline(
        mut self,
        descriptor: kelir_interface::fusion_plugin::FusedTrampolineDescriptor,
    ) -> Self {
        self.trampoline = Some(descriptor);
        self
    }

    pub fn trampoline(&self) -> Option<&kelir_interface::fusion_plugin::FusedTrampolineDescriptor> {
        self.trampoline.as_ref()
    }

    // see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
    pub fn with_checkpoint(mut self, plane: Arc<CheckpointPlane>) -> Self {
        self.checkpoint = Some(plane);
        self
    }

    pub fn checkpoint_plane(&self) -> Option<&Arc<CheckpointPlane>> {
        self.checkpoint.as_ref()
    }

    // see: cans/03-sdk-architecture.md#Rust-SDK
    pub async fn dispatch_custom_action(
        &self,
        name: &str,
        payload: ciborium::value::Value,
    ) -> Result<(), PluginError> {
        let Some(action) = self
            .custom_actions
            .iter()
            .find(|action| action.name == name)
        else {
            return Err(PluginError::Halt(format!("unknown custom action: {name}")));
        };
        (action.handler)(payload)
            .await
            .map_err(|error| PluginError::Halt(format!("custom action {name} failed: {error}")))
    }

    // see: cans/02-plugin-model.md#Foreign-plugin-proxy
    // TODO (L3) [plugin_engine]: fused descriptor ordinals bind 1:1 onto the native interposer chain; ring-level matching is the L2 handoff boundary; slop: descriptor ordinals unbound to interposer chain
    fn fused_ring(&self, ring: TrampolineRing) -> bool {
        self.trampoline.as_ref().is_some_and(|descriptor| {
            descriptor
                .rings
                .iter()
                .any(|chain| chain.ring == ring && !chain.hooks.is_empty())
        })
    }

    // see: cans/02-plugin-model.md#Foreign-plugin-proxy
    // TODO (L3) [plugin_engine]: marshal the ring event payload through the fused dispatcher's single traversal and rendezvous its folded foreign verdict into the engine fold; slop: fused traversal verdict stays Pass, foreign verdict never gates dispatch
    pub(super) fn fused_crossing(
        &self,
        ring: TrampolineRing,
    ) -> Result<Option<FusedCrossing<'_>>, PluginError> {
        let mut callbacks: Vec<_> = self
            .fused_callbacks
            .iter()
            .filter(|(callback_ring, _, _)| *callback_ring == ring)
            .collect();
        callbacks.sort_by_key(|(_, ordinal, _)| *ordinal);
        if callbacks.is_empty() && !self.fused_ring(ring) {
            return Ok(None);
        }
        self.try_foreign_submit()?;
        let mut verdict = Verdict::Pass;
        for (_, ordinal, callback) in callbacks {
            let outcome = quarantine_sync(
                || callback.run(ring, *ordinal),
                || {
                    Ok(Verdict::Drop {
                        errno: Errno::Efault,
                        reason: "Plugin panicked".into(),
                        guidance: None,
                    })
                },
            );
            verdict = outcome?;
            if !matches!(verdict, Verdict::Pass) {
                break;
            }
        }
        Ok(Some(FusedCrossing {
            engine: self,
            verdict,
        }))
    }

    pub fn vsock_service(&self, port: u32) -> Option<Arc<dyn VsockServiceHandler>> {
        self.vsock_services
            .iter()
            .find(|(spec, _, _)| spec.port == port)
            .map(|(_, handler, _)| Arc::clone(handler))
    }

    // see: cans/11-protocol.md#Kind-registration
    pub fn stream_sink(&self, kind: u16) -> Option<Arc<dyn StreamFrameSink>> {
        self.stream_sinks
            .iter()
            .find(|(bound, _, _)| *bound == kind)
            .map(|(_, sink, _)| Arc::clone(sink))
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub fn register_lock_broker(&mut self, coordination: Arc<dyn LockBrokerCoordination>) {
        self.lock_broker = Some(coordination);
    }

    pub fn lock_broker(&self) -> Option<Arc<dyn LockBrokerCoordination>> {
        self.lock_broker.as_ref().map(Arc::clone)
    }

    // see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
    pub fn parked_connection_reset(&self) -> Option<Arc<dyn ParkedConnectionReset>> {
        self.parked_reset.as_ref().map(Arc::clone)
    }

    // see: cans/08-lifecycle.md#Rollback
    pub fn token_revoker(&self) -> Option<Arc<dyn TokenRevocation>> {
        self.token_revoker.as_ref().map(Arc::clone)
    }

    pub fn is_sealed(&self) -> bool {
        self.sealed
    }

    pub fn bind_trap_scoped(&mut self, stage: Arc<dyn TrapInterposer>, scope: MountScope) {
        <Self as PipelineBinder>::bind_trap_scoped(self, stage, scope);
    }

    pub(super) fn scope_at(scopes: &[MountScope], index: usize) -> MountScope {
        scopes.get(index).cloned().unwrap_or_default()
    }

    fn effective_scope(&self, scope: MountScope) -> MountScope {
        match &self.active_scope {
            Some(active) => scope.intersect(active),
            None => scope,
        }
    }

    pub(super) fn fs_scope_allows(
        scope: &MountScope,
        path: &std::path::Path,
        ctx: &HandlerCtx,
    ) -> bool {
        if !scope.allows_device(VirtioDevice::Fs) {
            return false;
        }
        if !ctx.trap.binary_path.is_empty() && !scope.allows_binary(&ctx.trap.binary_path) {
            return false;
        }
        if !path.as_os_str().is_empty() && !scope.allows_path(path) {
            return false;
        }
        true
    }

    // see: cans/02-plugin-model.md#Hook-ordering
    pub fn set_ring_priority(&mut self, ring: TrampolineRing, index: usize, priority: i32) {
        let regs = self.ring_regs_mut(ring);
        if let Some(entry) = regs.get_mut(index) {
            entry.priority = priority;
        }
    }

    // see: cans/02-plugin-model.md#Per-ring-timeouts
    pub fn set_ring_timeout(
        &mut self,
        ring: TrampolineRing,
        index: usize,
        timeout: Option<Duration>,
    ) {
        let regs = self.ring_regs_mut(ring);
        if let Some(entry) = regs.get_mut(index) {
            entry.timeout = timeout;
        }
    }

    fn ring_regs_mut(&mut self, ring: TrampolineRing) -> &mut Vec<RingRegistration> {
        match ring {
            TrampolineRing::Trap | TrampolineRing::Signal | TrampolineRing::Fork => {
                &mut self.trap_regs
            }
            TrampolineRing::FsLookup
            | TrampolineRing::FsRead
            | TrampolineRing::FsWrite
            | TrampolineRing::FsLock
            | TrampolineRing::FsUnlink
            | TrampolineRing::FsRename
            | TrampolineRing::FsSetattr
            | TrampolineRing::FsSymlink
            | TrampolineRing::FsMkdir
            | TrampolineRing::FsReaddir
            | TrampolineRing::FsReadlink => &mut self.fs_regs,
            TrampolineRing::NetOutbound
            | TrampolineRing::NetInbound
            | TrampolineRing::SocketConnect
            | TrampolineRing::DnsQuery
            | TrampolineRing::SocketBind => &mut self.net_regs,
            TrampolineRing::ConsoleInput
            | TrampolineRing::ConsoleOutput
            | TrampolineRing::ExecStdout
            | TrampolineRing::ExecStderr
            | TrampolineRing::ExecStdin => &mut self.console_regs,
            TrampolineRing::HostLsmExec
            | TrampolineRing::HostPressure
            | TrampolineRing::HostKeyringAccess
            | TrampolineRing::HostPreempt
            | TrampolineRing::HostContainmentBreach => &mut self.host_regs,
            TrampolineRing::Lifecycle => &mut self.lifecycle_regs,
            TrampolineRing::Heartbeat
            | TrampolineRing::MemoryPressure
            | TrampolineRing::Watchdog => &mut self.telemetry_regs,
            _ => &mut self.trap_regs,
        }
    }

    fn ring_regs(&self, ring: TrampolineRing) -> &[RingRegistration] {
        match ring {
            TrampolineRing::Trap | TrampolineRing::Signal | TrampolineRing::Fork => &self.trap_regs,
            TrampolineRing::FsLookup
            | TrampolineRing::FsRead
            | TrampolineRing::FsWrite
            | TrampolineRing::FsLock
            | TrampolineRing::FsUnlink
            | TrampolineRing::FsRename
            | TrampolineRing::FsSetattr
            | TrampolineRing::FsSymlink
            | TrampolineRing::FsMkdir
            | TrampolineRing::FsReaddir
            | TrampolineRing::FsReadlink => &self.fs_regs,
            TrampolineRing::NetOutbound
            | TrampolineRing::NetInbound
            | TrampolineRing::SocketConnect
            | TrampolineRing::DnsQuery
            | TrampolineRing::SocketBind => &self.net_regs,
            TrampolineRing::ConsoleInput
            | TrampolineRing::ConsoleOutput
            | TrampolineRing::ExecStdout
            | TrampolineRing::ExecStderr
            | TrampolineRing::ExecStdin => &self.console_regs,
            TrampolineRing::HostLsmExec
            | TrampolineRing::HostPressure
            | TrampolineRing::HostKeyringAccess
            | TrampolineRing::HostPreempt
            | TrampolineRing::HostContainmentBreach => &self.host_regs,
            TrampolineRing::Lifecycle => &self.lifecycle_regs,
            TrampolineRing::Heartbeat
            | TrampolineRing::MemoryPressure
            | TrampolineRing::Watchdog => &self.telemetry_regs,
            _ => &self.trap_regs,
        }
    }

    // see: cans/02-plugin-model.md#Hook-ordering
    pub fn sort_rings_by_priority(&mut self) {
        macro_rules! resort {
            ($inters:expr, $scopes:expr, $regs:expr) => {{
                let mut order: Vec<usize> = (0..$inters.len()).collect();
                order.sort_by_key(|&i| {
                    let priority = $regs.get(i).map(|r| r.priority).unwrap_or(0);
                    (std::cmp::Reverse(priority), i)
                });
                $inters = order.iter().map(|&i| Arc::clone(&$inters[i])).collect();
                $scopes = order.iter().map(|&i| $scopes[i].clone()).collect();
                $regs = order
                    .iter()
                    .map(|&i| $regs.get(i).copied().unwrap_or_default())
                    .collect();
            }};
        }
        resort!(self.trap_interceptors, self.trap_scopes, self.trap_regs);
        resort!(self.fs_interceptors, self.fs_scopes, self.fs_regs);
        resort!(self.net_interceptors, self.net_scopes, self.net_regs);
        resort!(
            self.console_interceptors,
            self.console_scopes,
            self.console_regs
        );
        resort!(self.host_interceptors, self.host_scopes, self.host_regs);
        resort!(
            self.lifecycle_interceptors,
            self.lifecycle_scopes,
            self.lifecycle_regs
        );
        resort!(
            self.telemetry_interceptors,
            self.telemetry_scopes,
            self.telemetry_regs
        );
    }

    pub fn seal(&mut self) -> Result<(), PluginError> {
        if self.sealed {
            return Err(PluginError::Halt("pipeline already sealed".into()));
        }
        // TODO (L3) [protocol_mux]: surface the structured EKindConflict/EPortConflict envelope from the artifacts; slop: collision aborts with a Halt prefix, not the structured envelope
        for (index, (spec, _, _)) in self.vsock_services.iter().enumerate() {
            if self.vsock_services[..index]
                .iter()
                .any(|(bound, _, _)| bound.port == spec.port)
            {
                return Err(PluginError::Halt(format!(
                    "vsock port already bound: {}",
                    spec.port
                )));
            }
        }
        // see: cans/11-protocol.md#Kind-registration
        for (index, (kind, _, _)) in self.stream_sinks.iter().enumerate() {
            if *kind <= 0x000F || *kind >= 0x8000 {
                return Err(PluginError::Halt(format!(
                    "EKindReserved: core stream kind {kind:#06x} is frozen and cannot be re-registered",
                )));
            }
            if self.stream_sinks[..index]
                .iter()
                .any(|(bound, _, _)| bound == kind)
            {
                return Err(PluginError::Halt(format!(
                    "EKindConflict: stream kind {kind:#06x} already registered",
                )));
            }
        }
        self.sort_rings_by_priority();
        self.sealed = true;
        Ok(())
    }

    // see: cans/02-plugin-model.md#Static-dispatch
    pub async fn dispatch_trap_typed<Plugins, Params>(
        &self,
        event: &TrapEvent,
        ctx: &HandlerCtx<Plugins, Params>,
    ) -> Result<Verdict, PluginError> {
        self.dispatch_trap(event, &ctx.erased()).await
    }

    // see: cans/02-plugin-model.md#Static-dispatch
    pub async fn dispatch_trap_typed_with_session<Plugins, Params>(
        &self,
        event: &TrapEvent,
        ctx: &HandlerCtx<Plugins, Params>,
        session: &Session,
    ) -> Result<Verdict, PluginError> {
        self.dispatch_trap_inner(event, &ctx.erased(), Some(session))
            .await
    }

    // see: cans/02-plugin-model.md#Static-dispatch
    pub async fn dispatch_process_exit_typed<Plugins, Params>(
        &self,
        event: &ProcessExit,
        ctx: &HandlerCtx<Plugins, Params>,
    ) -> Result<(), PluginError> {
        self.dispatch_process_exit(event, &ctx.erased()).await
    }

    // see: cans/02-plugin-model.md#Static-dispatch
    pub fn dispatch_fs_write_typed<Plugins, Params>(
        &self,
        path: &std::path::Path,
        data: &mut [u8],
        pid: u32,
        ctx: &HandlerCtx<Plugins, Params>,
    ) -> Result<FilterAction, PluginError> {
        self.dispatch_fs_write(path, data, pid, &ctx.erased())
    }

    // see: cans/artifacts/code/plugin-trait.rs#TrapInterposer
    pub async fn dispatch_trap(
        &self,
        event: &TrapEvent,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        self.dispatch_trap_inner(event, ctx, None).await
    }

    // see: cans/artifacts/code/plugin-trait.rs#TrapInterposer
    pub async fn dispatch_trap_with_session(
        &self,
        event: &TrapEvent,
        ctx: &HandlerCtx,
        session: &Session,
    ) -> Result<Verdict, PluginError> {
        self.dispatch_trap_inner(event, ctx, Some(session)).await
    }

    // see: cans/artifacts/code/plugin-trait.rs#TrapInterposer
    async fn dispatch_trap_inner(
        &self,
        event: &TrapEvent,
        ctx: &HandlerCtx,
        session: Option<&Session>,
    ) -> Result<Verdict, PluginError> {
        // see: cans/03-sdk-architecture.md#Params-typed-views
        let params_hydrated_ctx = params_hydration_flow::hydrate_params_ctx(ctx, session);
        let ctx = params_hydrated_ctx.as_ref().unwrap_or(ctx);
        // see: cans/10-attestation.md#Execution-receipts
        let hydrated;
        let ctx = match self.receipt_store.lookup(ctx.pid) {
            Some(receipt) if ctx.receipt.is_none() => {
                hydrated = ctx.erased().with_receipt(receipt);
                &hydrated
            }
            _ => ctx,
        };
        if let Some(syscall) = &event.syscall {
            self.metrics.record_syscall();
            // see: cans/14-observability.md#Metrics-pipeline
            if syscall.nr == libc::SYS_execve || syscall.nr == libc::SYS_execveat {
                self.metrics.exec_started();
            }
        }
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::Trap)?;
        if let Some(verdict) = fused
            .as_ref()
            .and_then(|crossing| crossing.non_pass_verdict())
        {
            return Ok(verdict);
        }
        // see: cans/10-attestation.md#Hook-trace
        let mut spans: Vec<HookSpan> = Vec::new();
        for (index, interceptor) in self.trap_interceptors.iter().enumerate() {
            let fallback = MountScope::default();
            let scope = self.trap_scopes.get(index).unwrap_or(&fallback);
            match self.gate.evaluate(event, scope) {
                GateDecision::Skip => continue,
                GateDecision::Deny(verdict) => {
                    if let (Some(callback), Some(session)) = (self.on_trap_deny.as_deref(), session)
                    {
                        callback(event, &verdict, session).await?;
                    }
                    return Ok(verdict);
                }
                GateDecision::FallThrough => {}
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            // see: cans/02-plugin-model.md#Per-ring-timeouts
            // TODO (L3) [plugin_engine]: true preemption; the deadline is checked after the hook future resolves.; slop: overrunning hook holds the trap past its deadline.
            let deadline = self.trap_regs.get(index).and_then(|reg| reg.timeout);
            let started = Instant::now();
            let verdict = {
                let outcome = quarantine_verdict(interceptor.on_trap(event, ctx)).await;
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                outcome?
            };
            let mut verdict = verdict;
            if deadline.is_some_and(|limit| started.elapsed() > limit) {
                verdict = Verdict::Drop {
                    errno: Errno::Etimeout,
                    reason: "hook timeout".into(),
                    guidance: None,
                };
            }
            spans.push(HookSpan {
                plugin: "host".to_string(),
                ring: TrampolineRing::Trap,
                verdict_kind: verdict_kind_of(&verdict),
                duration_ns: started.elapsed().as_nanos() as u64,
                ordinal: index as u32,
            });
            if !matches!(verdict, Verdict::Pass) {
                self.attach_hook_trace(ctx.pid, spans);
                return Ok(verdict);
            }
        }
        drop(fused);
        self.attach_hook_trace(ctx.pid, spans);
        Ok(Verdict::Pass)
    }

    // see: cans/10-attestation.md#Hook-trace
    fn attach_hook_trace(&self, pid: u32, spans: Vec<HookSpan>) {
        if spans.is_empty() {
            return;
        }
        if let Some(mut receipt) = self.receipt_store.lookup(pid) {
            receipt.hook_policy_descriptors =
                spans.iter().map(HookPolicyDescriptor::from_span).collect();
            receipt.hook_trace = spans;
            self.receipt_store.insert(receipt);
        }
    }

    pub async fn dispatch_process_exit(
        &self,
        event: &ProcessExit,
        ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        // see: cans/10-attestation.md#Execution-receipts
        let hydrated;
        let ctx = match self.receipt_store.lookup(ctx.pid) {
            Some(receipt) if ctx.receipt.is_none() => {
                hydrated = ctx.erased().with_receipt(receipt);
                &hydrated
            }
            _ => ctx,
        };
        self.metrics.record_exec();
        // TODO (L3) [plugin_engine]: in_flight_execs stays exact only once the L3 exec path guarantees a ProcessExit per exec_started; slop: in-flight count drifts, ProcessExit never guaranteed
        self.metrics.exec_finished();
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::ProcessExit)?;
        for (index, interceptor) in self.trap_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.trap_scopes, index);
            if !matches!(
                self.gate.evaluate(&ctx.trap, &scope),
                GateDecision::FallThrough
            ) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let outcome = quarantine_unit(interceptor.on_process_exit(event, ctx)).await;
            if fused.is_none() {
                self.foreign_submit_complete();
            }
            outcome?;
        }
        drop(fused);
        Ok(())
    }

    // L3-GAP: [plugin_engine]: host scope.devices has no VirtioDevice mapping; only the path filter is enforced here; slop: device scope unchecked, path filter only
    pub fn dispatch_host_lsm_exec(
        &self,
        path: &std::path::Path,
        pid: u32,
    ) -> Result<Verdict, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::HostLsmExec)?;
        if let Some(verdict) = fused
            .as_ref()
            .and_then(|crossing| crossing.non_pass_verdict())
        {
            return Ok(verdict);
        }
        for (index, interceptor) in self.host_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.host_scopes, index);
            if !scope.allows_path(path) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            // see: cans/02-plugin-model.md#Per-ring-timeouts
            // TODO (L3) [plugin_engine]: true preemption; the deadline is checked after the hook returns.; slop: overrunning hook holds the ring past its deadline.
            let deadline = self.host_regs.get(index).and_then(|reg| reg.timeout);
            let started = Instant::now();
            let verdict = {
                let outcome = quarantine_sync(
                    || interceptor.on_host_lsm_exec(path, pid),
                    || {
                        Ok(Verdict::Drop {
                            errno: Errno::Efault,
                            reason: "Plugin panicked".into(),
                            guidance: None,
                        })
                    },
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                outcome?
            };
            let mut verdict = verdict;
            if deadline.is_some_and(|limit| started.elapsed() > limit) {
                verdict = Verdict::Drop {
                    errno: Errno::Etimeout,
                    reason: "hook timeout".into(),
                    guidance: None,
                };
            }
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        drop(fused);
        Ok(Verdict::Pass)
    }

    // see: cans/11-protocol.md#Signal-and-fork-interception
    // Signal rules classify the target relation (self, children, session,
    // shim_pid1, peer) and the sender binary against the trap MountScope
    // before the hook runs; peers and shim_pid1 default to deny scope-wise
    // because no registered scope vouches for a cross-ExecSlot signal.
    fn signal_scope_allows(scope: &MountScope, ctx: &SignalContext) -> bool {
        if !scope.allows_device(VirtioDevice::Fs) {
            return false;
        }
        match ctx.target_relation {
            Some(SignalTargetRelation::Self_) | Some(SignalTargetRelation::Children) => {}
            // see: cans/11-protocol.md#Signal-and-fork-interception
            // TODO (L3) [plugin_engine]: session and shim_pid1 relations resolve against the live session process tree at the shim signal boundary; until then only self/children pass scope filtering.; slop: session/shim targets skip scope filtering entirely, one ExecSlot signals another
            Some(SignalTargetRelation::Session) | Some(SignalTargetRelation::ShimPid1) => {}
            Some(SignalTargetRelation::Peer) | None => return false,
        }
        true
    }

    // see: cans/02-plugin-model.md#Unified-verdict
    // Signal verdicts support Pass, Drop, Absorb, and Redirect
    // (SIGKILL->SIGTERM via Patch(SignalMutation)).
    fn surface_signal_verdict(verdict: Verdict) -> Verdict {
        match verdict {
            Verdict::Suspend { .. } => Verdict::Drop {
                errno: Errno::Eperm,
                reason: "signal verdict Suspend not surfaced; pending approval hold"
                    .to_string(),
                guidance: None,
            },
            other => other,
        }
    }

    // see: cans/11-protocol.md#Signal-and-fork-interception
    pub async fn dispatch_signal(
        &self,
        sender: u32,
        target: u32,
        sig: i32,
    ) -> Result<Verdict, PluginError> {
        self.dispatch_signal_ctx(SignalContext {
            sender,
            target,
            sig,
            syscall_nr: 0,
            target_relation: None,
        })
        .await
    }

    // see: cans/11-protocol.md#Signal-and-fork-interception
    pub async fn dispatch_signal_ctx(
        &self,
        ctx: SignalContext,
    ) -> Result<Verdict, PluginError> {
        let SignalContext {
            sender,
            target,
            sig,
            ..
        } = ctx.clone();
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::Signal)?;
        if let Some(verdict) = fused
            .as_ref()
            .and_then(|crossing| crossing.non_pass_verdict())
        {
            return Ok(verdict);
        }
        for (index, interceptor) in self.trap_interceptors.iter().enumerate() {
            // see: cans/02-plugin-model.md#Mount-scoping
            // SignalContext target relation now gates which interceptors run;
            // scopes without signal coverage (peer/shim targets) skip the ring.
            let scope = Self::scope_at(&self.trap_scopes, index);
            if !Self::signal_scope_allows(&scope, &ctx) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            // see: cans/02-plugin-model.md#Per-ring-timeouts
            // TODO (L3) [plugin_engine]: true preemption; the deadline is checked after the hook future resolves.; slop: overrunning hook holds the signal past its deadline.
            let deadline = self.trap_regs.get(index).and_then(|reg| reg.timeout);
            let started = Instant::now();
            let verdict = {
                let outcome =
                    quarantine_unit(interceptor.on_signal(sender, target, sig)).await;
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                outcome?
            };
            let mut verdict = verdict;
            if deadline.is_some_and(|limit| started.elapsed() > limit) {
                verdict = Verdict::Drop {
                    errno: Errno::Etimeout,
                    reason: "hook timeout".into(),
                    guidance: None,
                };
            }
            if !matches!(verdict, Verdict::Pass) {
                drop(fused);
                return Ok(Self::surface_signal_verdict(verdict));
            }
        }
        drop(fused);
        Ok(Verdict::Pass)
    }

    // see: cans/11-protocol.md#Signal-and-fork-interception
    pub async fn dispatch_fork(&self, parent: u32, child: u32) -> Result<Verdict, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::Fork)?;
        if let Some(verdict) = fused
            .as_ref()
            .and_then(|crossing| crossing.non_pass_verdict())
        {
            return Ok(verdict);
        }
        // TODO (L3) [plugin_engine]: fork carries no path or device payload, so trap-scope filtering lands with the shim fork boundary; slop: fork ring skips scope filter entirely
        for (index, interceptor) in self.trap_interceptors.iter().enumerate() {
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            // see: cans/02-plugin-model.md#Per-ring-timeouts
            // TODO (L3) [plugin_engine]: true preemption; the deadline is checked after the hook future resolves.; slop: overrunning hook holds the fork past its deadline.
            let deadline = self.trap_regs.get(index).and_then(|reg| reg.timeout);
            let started = Instant::now();
            let verdict = {
                let outcome = quarantine_unit(interceptor.on_fork(parent, child)).await;
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                outcome?
            };
            let mut verdict = verdict;
            if deadline.is_some_and(|limit| started.elapsed() > limit) {
                verdict = Verdict::Drop {
                    errno: Errno::Etimeout,
                    reason: "hook timeout".into(),
                    guidance: None,
                };
            }
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        drop(fused);
        Ok(Verdict::Pass)
    }
}
