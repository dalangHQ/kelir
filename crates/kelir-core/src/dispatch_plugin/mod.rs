// see: cans/02-plugin-model.md#Static-dispatch
// see: cans/02-plugin-model.md#Unified-interposition-pipeline
mod console_dispatch;
mod fs_dispatch;
mod host_dispatch;
mod lifecycle_dispatch;
mod params_hydration;
mod registration;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::{SystemTime, UNIX_EPOCH};

use kelir_interface::driver_interface::{WatchdogEvent, WatchdogVerdict};
use kelir_interface::errno_types::Errno;
use kelir_interface::fusion_plugin::{
    FusedTrampolineCallback, FusedTrampolineDescriptor, TrampolineRing,
};
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    ConsoleInterposer, FfiDispatchQueue, FsInterposer, HandlerCtx, HostInterposer, LifecycleCtx,
    LifecycleEvent, LifecycleParticipant, LockBrokerCoordination, MountScope, NetInterposer,
    ParkedConnectionReset, PipelineBinder, PostExecGate, StreamFrameSink,
    SystemTelemetryParticipant, TokenRevocation, TrapInterposer, VirtioDevice,
};
use kelir_interface::protocol_wire::{
    EXEC_STREAM_STDERR, EXEC_STREAM_STDOUT, FilterAction, Heartbeat, PostExecAction, ProcessExit,
    StepVerdict, StreamKind, TrapEvent, Verdict,
};
use kelir_interface::telemetry::{ExecutionReceipt, ReceiptStore};
use kelir_interface::vsock_mesh::{VsockServiceHandler, VsockServiceSpec};

use crate::planes::checkpoint::{CheckpointPlane, PidLineage};
use crate::planes::terminal::LogStream;
use crate::session_lifecycle::Session;
use crate::trap_gate::{GateDecision, TrapGateEvaluator};

// see: cans/artifacts/code/plugin-trait.rs#PANIC_QUARANTINE_ENABLED
struct Quarantined<F> {
    inner: Option<Pin<Box<F>>>,
}

impl<F: Future> Quarantined<F> {
    fn new(future: F) -> Self {
        Self {
            inner: Some(Box::pin(future)),
        }
    }
}

impl<F: Future> Future for Quarantined<F> {
    type Output = Result<F::Output, PluginError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let Some(inner) = this.inner.as_mut() else {
            return Poll::Ready(Err(PluginError::Halt(
                "quarantined future polled after completion".into(),
            )));
        };
        // TODO (L3) [lc_03_step_evaluation_taint_rollback]: log the panic payload and backtrace to the host ledger and roll back the active StepSlot; slop: panic logs without StepSlot rollback, tainted state kept
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Future::poll(inner.as_mut(), cx)
        })) {
            Ok(Poll::Ready(output)) => {
                this.inner = None;
                Poll::Ready(Ok(output))
            }
            Ok(Poll::Pending) => Poll::Pending,
            Err(_) => {
                this.inner = None;
                Poll::Ready(Err(PluginError::Halt("plugin panicked".into())))
            }
        }
    }
}

async fn quarantine_verdict<F>(future: F) -> Result<Verdict, PluginError>
where
    F: Future<Output = Result<Verdict, PluginError>>,
{
    match Quarantined::new(future).await {
        Ok(output) => output,
        Err(_) => Ok(Verdict::Drop {
            errno: Some(Errno::Efault),
            reason: "Plugin panicked".into(),
        }),
    }
}

async fn quarantine_unit<F, T>(future: F) -> Result<T, PluginError>
where
    F: Future<Output = Result<T, PluginError>>,
{
    Quarantined::new(future).await?
}

fn quarantine_sync<T>(run: impl FnOnce() -> T, on_panic: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(output) => output,
        Err(_) => on_panic(),
    }
}

// see: cans/02-plugin-model.md#Unified-verdict
pub(crate) fn map_reject_to_verdict(
    result: Result<Verdict, PluginError>,
) -> Result<Verdict, PluginError> {
    match result {
        Err(PluginError::Reject { errno, reason }) => Ok(Verdict::Drop {
            errno: Some(errno),
            reason,
        }),
        other => other,
    }
}

// see: cans/02-plugin-model.md#Unified-verdict
pub(crate) fn map_reject_to_filter_action(
    result: Result<FilterAction, PluginError>,
) -> Result<FilterAction, PluginError> {
    match result {
        Err(PluginError::Reject { errno, reason }) => Ok(FilterAction::Drop {
            errno: Some(errno),
            reason,
        }),
        other => other,
    }
}

// see: cans/03-sdk-architecture.md#Builder-consumption-model-across-languages
pub type TrapDenyCallback = Box<
    dyn Fn(&TrapEvent, &Session) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send>>
        + Send
        + Sync,
>;

// see: cans/03-sdk-architecture.md#Rust-SDK
pub type CustomActionCallback = Box<
    dyn Fn(
            ciborium::value::Value,
        ) -> Pin<
            Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send>,
        > + Send
        + Sync,
>;

// see: cans/03-sdk-architecture.md#Rust-SDK
pub struct CustomActionBinding {
    pub name: String,
    pub handler: CustomActionCallback,
}

// see: cans/08-lifecycle.md#CoW-fork
#[derive(Clone)]
pub struct HostRegistrationState {
    pub on_trap_deny: Option<Arc<TrapDenyCallback>>,
    pub custom_actions: Vec<Arc<CustomActionBinding>>,
    pub stream_kind_services: Vec<(StreamKind, Arc<dyn StreamFrameSink>)>,
    pub lock_broker: Option<Arc<dyn LockBrokerCoordination>>,
    pub token_revoker: Option<Arc<dyn TokenRevocation>>,
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
struct FusedCrossing<'a> {
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

pub struct PipelineEngine {
    trap_interceptors: Vec<Arc<dyn TrapInterposer>>,
    trap_scopes: Vec<MountScope>,
    gate: TrapGateEvaluator,
    fs_interceptors: Vec<Arc<dyn FsInterposer>>,
    fs_scopes: Vec<MountScope>,
    net_interceptors: Vec<Arc<dyn NetInterposer>>,
    net_scopes: Vec<MountScope>,
    console_interceptors: Vec<Arc<dyn ConsoleInterposer>>,
    console_scopes: Vec<MountScope>,
    terminal: Option<Arc<crate::planes::terminal::TerminalPlane>>,
    host_interceptors: Vec<Arc<dyn HostInterposer>>,
    host_scopes: Vec<MountScope>,
    vsock_services: Vec<(VsockServiceSpec, Arc<dyn VsockServiceHandler>, MountScope)>,
    lifecycle_interceptors: Vec<Arc<dyn LifecycleParticipant>>,
    lifecycle_scopes: Vec<MountScope>,
    telemetry_interceptors: Vec<Arc<dyn SystemTelemetryParticipant>>,
    telemetry_scopes: Vec<MountScope>,
    post_exec_gates: Vec<Arc<dyn PostExecGate>>,
    post_exec_scopes: Vec<MountScope>,
    stream_sinks: Vec<(StreamKind, Arc<dyn StreamFrameSink>, MountScope)>,
    lock_broker: Option<Arc<dyn LockBrokerCoordination>>,
    parked_reset: Option<Arc<dyn ParkedConnectionReset>>,
    token_revoker: Option<Arc<dyn TokenRevocation>>,
    metrics: Arc<kelir_otlp::registry_otlp::MetricRegistry>,
    trampoline: Option<kelir_interface::fusion_plugin::FusedTrampolineDescriptor>,
    checkpoint: Option<Arc<CheckpointPlane>>,
    on_trap_deny: Option<Arc<TrapDenyCallback>>,
    custom_actions: Vec<Arc<CustomActionBinding>>,
    receipt_store: ReceiptStore,
    fused_callbacks: Vec<(TrampolineRing, u32, Arc<dyn FusedTrampolineCallback>)>,
    foreign_queue: FfiDispatchQueue,
    ffi_rate_per_sec: u32,
    ffi_window_start_ms: AtomicU64,
    ffi_window_ops: AtomicU64,
    active_scope: Option<MountScope>,
    sealed: bool,
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
    }
    fn bind_net_scoped(&mut self, stage: Arc<dyn NetInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.net_interceptors.push(stage);
        self.net_scopes.push(scope);
    }
    fn bind_trap_scoped(&mut self, stage: Arc<dyn TrapInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.trap_interceptors.push(stage);
        self.trap_scopes.push(scope);
    }
    // see: cans/10-attestation.md#Step-evaluation-loop
    fn bind_post_exec_scoped(&mut self, gate: Arc<dyn PostExecGate>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.post_exec_gates.push(gate);
        self.post_exec_scopes.push(scope);
    }
    fn bind_console_scoped(&mut self, stage: Arc<dyn ConsoleInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.console_interceptors.push(stage);
        self.console_scopes.push(scope);
    }
    fn bind_host_scoped(&mut self, stage: Arc<dyn HostInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.host_interceptors.push(stage);
        self.host_scopes.push(scope);
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
    }
    fn bind_telemetry_scoped(
        &mut self,
        stage: Arc<dyn SystemTelemetryParticipant>,
        scope: MountScope,
    ) {
        let scope = self.effective_scope(scope);
        self.telemetry_interceptors.push(stage);
        self.telemetry_scopes.push(scope);
    }
    // see: cans/11-protocol.md#Kind-registration
    fn bind_stream_kind_scoped(
        &mut self,
        kind: StreamKind,
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
            gate: TrapGateEvaluator::new(
                crate::trap_gate::TRUSTED_PREFIXES
                    .iter()
                    .map(|prefix| (*prefix).to_string())
                    .collect(),
            ),
            fs_interceptors: Vec::new(),
            fs_scopes: Vec::new(),
            net_interceptors: Vec::new(),
            net_scopes: Vec::new(),
            console_interceptors: Vec::new(),
            console_scopes: Vec::new(),
            terminal: None,
            host_interceptors: Vec::new(),
            host_scopes: Vec::new(),
            vsock_services: Vec::new(),
            lifecycle_interceptors: Vec::new(),
            lifecycle_scopes: Vec::new(),
            telemetry_interceptors: Vec::new(),
            telemetry_scopes: Vec::new(),
            post_exec_gates: Vec::new(),
            post_exec_scopes: Vec::new(),
            stream_sinks: Vec::new(),
            lock_broker: None,
            parked_reset: None,
            token_revoker: None,
            metrics: Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new()),
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

    pub fn with_metrics(mut self, metrics: Arc<kelir_otlp::registry_otlp::MetricRegistry>) -> Self {
        self.metrics = metrics;
        self
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    pub fn with_terminal(mut self, terminal: Arc<crate::planes::terminal::TerminalPlane>) -> Self {
        self.terminal = Some(terminal);
        self
    }

    pub fn metrics(&self) -> &Arc<kelir_otlp::registry_otlp::MetricRegistry> {
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

    // see: cans/10-attestation.md#Step-evaluation-loop
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

    // see: cans/10-attestation.md#Step-evaluation-loop
    // TODO (L3) [plugin_engine]: resolve guest pid to its lineage root before lookup once the trap-stream ppid chain walk lands; slop: trap lookup without lineage root, wrong process blamed
    pub fn hydrate_step<Plugins, Params>(
        &self,
        ctx: &mut HandlerCtx<Plugins, Params>,
        lineage: PidLineage,
    ) -> Result<(), PluginError> {
        let Some(plane) = self.checkpoint.as_ref() else {
            return Ok(());
        };
        if let Some(descriptor) = plane.active_step(lineage)? {
            ctx.step.push(descriptor);
        }
        Ok(())
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
    // TODO (L3) [plugin_engine]: the AOT-fused trampoline closure marshals the whole hook chain across this single crossing at the native-to-foreign boundary; slop: hook chain split across crossings, single-crossing unproven
    fn fused_crossing(
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
                        errno: Some(Errno::Efault),
                        reason: "Plugin panicked".into(),
                    })
                },
            );
            verdict = map_reject_to_verdict(outcome)?;
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
    pub fn stream_sink(&self, kind: StreamKind) -> Option<Arc<dyn StreamFrameSink>> {
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

    // see: cans/10-attestation.md#Step-evaluation-loop
    pub fn bind_post_exec_scoped(&mut self, gate: Arc<dyn PostExecGate>, scope: MountScope) {
        <Self as PipelineBinder>::bind_post_exec_scoped(self, gate, scope);
    }

    fn scope_at(scopes: &[MountScope], index: usize) -> MountScope {
        scopes.get(index).cloned().unwrap_or_default()
    }

    fn effective_scope(&self, scope: MountScope) -> MountScope {
        match &self.active_scope {
            Some(active) => scope.intersect(active),
            None => scope,
        }
    }

    fn fs_scope_allows(scope: &MountScope, path: &std::path::Path, ctx: &HandlerCtx) -> bool {
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

    // see: cans/02-plugin-model.md#Mount-scoping
    fn net_scope_allows(scope: &MountScope, remote: Option<([u8; 4], u16)>) -> bool {
        match remote {
            Some((ip, port)) => scope.allows_net(ip, port),
            None => scope.net_cidrs.is_none() && scope.net_ports.is_none(),
        }
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    // see: cans/07-wire-plane.md#Userspace-network-stack
    fn net_remote_endpoint(frame: &[u8], inbound: bool) -> Option<([u8; 4], u16)> {
        use crate::planes::wire::layout_wire::{
            ETH_HEADER_BYTES, ETHERTYPE_IPV4, EthView, IP_PROTO_TCP, IP_PROTO_UDP,
            IPV4_MIN_HEADER_BYTES, Ipv4View, L4_PORT_PAIR_BYTES,
        };
        if frame.len() < ETH_HEADER_BYTES || EthView::new_unchecked(frame).etype() != ETHERTYPE_IPV4
        {
            return None;
        }
        let ip = Ipv4View::new_unchecked(&frame[ETH_HEADER_BYTES..]);
        let ihl = ip.ihl_bytes_checked()?;
        if ihl < IPV4_MIN_HEADER_BYTES
            || frame.len() < ETH_HEADER_BYTES + ihl + L4_PORT_PAIR_BYTES
            || !matches!(ip.proto(), IP_PROTO_TCP | IP_PROTO_UDP)
        {
            return None;
        }
        let l4 = &frame[ETH_HEADER_BYTES + ihl..];
        if inbound {
            Some((ip.src_checked()?, u16::from_be_bytes([l4[0], l4[1]])))
        } else {
            Some((ip.dst_checked()?, u16::from_be_bytes([l4[2], l4[3]])))
        }
    }

    pub fn seal(&mut self) -> Result<(), PluginError> {
        if self.sealed {
            return Err(PluginError::Halt("pipeline already sealed".into()));
        }
        // TODO (L3) [pm_05_stream_kind_conflict_ekindconflict]: conflicts surface the structured EKindConflict/EPortConflict envelope from the artifacts; slop: kind collision seals Ok, EKindConflict never surfaced
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
        // TODO (L3) [pm_03_core_stream_kind_reregistration_abort]: freeze all five ring chains; slop: frozen core kind re-registers, seal never aborts
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
        let params_hydrated_ctx = params_hydration::hydrate_params_ctx(ctx, session);
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
        for (index, interceptor) in self.trap_interceptors.iter().enumerate() {
            let fallback = MountScope::default();
            let scope = self.trap_scopes.get(index).unwrap_or(&fallback);
            match self.gate.evaluate(event, scope) {
                GateDecision::Skip => continue,
                GateDecision::Deny(verdict) => {
                    if let (Some(callback), Some(session)) = (self.on_trap_deny.as_deref(), session)
                    {
                        callback(event, session).await?;
                    }
                    return Ok(verdict);
                }
                GateDecision::FallThrough => {}
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let verdict = {
                let outcome = quarantine_verdict(interceptor.on_trap(event, ctx)).await;
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                map_reject_to_verdict(outcome)?
            };
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        drop(fused);
        Ok(Verdict::Pass)
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

    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn dispatch_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::NetOutbound)?;
        let remote = Self::net_remote_endpoint(frame, false);
        for (index, interceptor) in self.net_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.net_scopes, index);
            if !scope.allows_device(VirtioDevice::Net) {
                continue;
            }
            if !Self::net_scope_allows(&scope, remote) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let action = {
                let outcome = quarantine_sync(
                    || interceptor.on_net_outbound(frame),
                    || {
                        Ok(FilterAction::Abort {
                            reason: "Plugin panicked".into(),
                        })
                    },
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                map_reject_to_filter_action(outcome)?
            };
            if !matches!(action, FilterAction::Forward) {
                return Ok(action);
            }
        }
        drop(fused);
        Ok(FilterAction::Forward)
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn dispatch_net_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::NetInbound)?;
        let remote = Self::net_remote_endpoint(frame, true);
        for (index, interceptor) in self.net_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.net_scopes, index);
            if !scope.allows_device(VirtioDevice::Net) {
                continue;
            }
            if !Self::net_scope_allows(&scope, remote) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let action = {
                let outcome = quarantine_sync(
                    || interceptor.on_net_inbound(frame),
                    || {
                        Ok(FilterAction::Abort {
                            reason: "Plugin panicked".into(),
                        })
                    },
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                map_reject_to_filter_action(outcome)?
            };
            if !matches!(action, FilterAction::Forward) {
                return Ok(action);
            }
        }
        drop(fused);
        Ok(FilterAction::Forward)
    }

    // TODO (L3) [plugin_engine]: host scope.devices has no VirtioDevice mapping; only the path filter is enforced here; slop: device scope unchecked, path filter only
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
            let verdict = {
                let outcome = quarantine_sync(
                    || interceptor.on_host_lsm_exec(path, pid),
                    || {
                        Ok(Verdict::Drop {
                            errno: Some(Errno::Efault),
                            reason: "Plugin panicked".into(),
                        })
                    },
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                map_reject_to_verdict(outcome)?
            };
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        drop(fused);
        Ok(Verdict::Pass)
    }
}
