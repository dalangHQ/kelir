// see: cans/02-plugin-model.md#Static-dispatch
// see: cans/02-plugin-model.md#Unified-interposition-pipeline
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::{SystemTime, UNIX_EPOCH};

use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    ConsoleInterposer, FfiDispatchQueue, FsInterposer, HandlerCtx, HostInterposer,
    LockBrokerCoordination, MountScope, NetInterposer, ParkedConnectionReset, PipelineBinder,
    TokenRevocation, TrapInterposer, VirtioDevice,
};
use kelir_interface::protocol::{FilterAction, ProcessExit, TrapEvent, Verdict};
use kelir_interface::vsock::{VsockServiceHandler, VsockServiceSpec};

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
        // TODO (L3): log the panic payload and backtrace to the host ledger and roll back the active StepSlot.
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
            errno: Some("EFAULT".into()),
            reason: "Plugin panicked".into(),
        }),
    }
}

async fn quarantine_unit<F>(future: F) -> Result<(), PluginError>
where
    F: Future<Output = Result<(), PluginError>>,
{
    Quarantined::new(future).await?
}

fn quarantine_sync<T>(run: impl FnOnce() -> T, on_panic: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(output) => output,
        Err(_) => on_panic(),
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
    host_interceptors: Vec<Arc<dyn HostInterposer>>,
    host_scopes: Vec<MountScope>,
    vsock_services: Vec<(VsockServiceSpec, Arc<dyn VsockServiceHandler>, MountScope)>,
    lock_broker: Option<Arc<dyn LockBrokerCoordination>>,
    parked_reset: Option<Arc<dyn ParkedConnectionReset>>,
    token_revoker: Option<Arc<dyn TokenRevocation>>,
    metrics: Arc<kelir_otlp::registry::MetricRegistry>,
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
    fn bind_fs(&mut self, stage: Arc<dyn FsInterposer>) {
        self.bind_fs_scoped(stage, MountScope::default());
    }
    fn bind_fs_scoped(&mut self, stage: Arc<dyn FsInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.fs_interceptors.push(stage);
        self.fs_scopes.push(scope);
    }
    fn bind_net(&mut self, stage: Arc<dyn NetInterposer>) {
        self.bind_net_scoped(stage, MountScope::default());
    }
    fn bind_net_scoped(&mut self, stage: Arc<dyn NetInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.net_interceptors.push(stage);
        self.net_scopes.push(scope);
    }
    fn bind_trap(&mut self, stage: Arc<dyn TrapInterposer>) {
        self.bind_trap_scoped(stage, MountScope::default());
    }
    fn bind_trap_scoped(&mut self, stage: Arc<dyn TrapInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.trap_interceptors.push(stage);
        self.trap_scopes.push(scope);
    }
    fn bind_console(&mut self, stage: Arc<dyn ConsoleInterposer>) {
        self.bind_console_scoped(stage, MountScope::default());
    }
    fn bind_console_scoped(&mut self, stage: Arc<dyn ConsoleInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.console_interceptors.push(stage);
        self.console_scopes.push(scope);
    }
    fn bind_host(&mut self, stage: Arc<dyn HostInterposer>) {
        self.bind_host_scoped(stage, MountScope::default());
    }
    fn bind_host_scoped(&mut self, stage: Arc<dyn HostInterposer>, scope: MountScope) {
        let scope = self.effective_scope(scope);
        self.host_interceptors.push(stage);
        self.host_scopes.push(scope);
    }
    fn bind_vsock_service(
        &mut self,
        spec: VsockServiceSpec,
        service: Arc<dyn VsockServiceHandler>,
    ) {
        self.bind_vsock_service_scoped(spec, service, MountScope::default());
    }
    fn bind_vsock_service_scoped(
        &mut self,
        spec: VsockServiceSpec,
        service: Arc<dyn VsockServiceHandler>,
        scope: MountScope,
    ) {
        // TODO (L3): scope is stored; dispatch-time enforcement lands with vsock dispatch.
        let scope = self.effective_scope(scope);
        self.vsock_services.push((spec, service, scope));
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
            host_interceptors: Vec::new(),
            host_scopes: Vec::new(),
            vsock_services: Vec::new(),
            lock_broker: None,
            parked_reset: None,
            token_revoker: None,
            metrics: Arc::new(kelir_otlp::registry::MetricRegistry::new()),
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
        // TODO (L3): exact token-bucket accounting; this native 1s window is the L2 pre-insertion rate gate.
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

    pub fn with_metrics(mut self, metrics: Arc<kelir_otlp::registry::MetricRegistry>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> &Arc<kelir_otlp::registry::MetricRegistry> {
        &self.metrics
    }

    pub fn vsock_service(&self, port: u32) -> Option<Arc<dyn VsockServiceHandler>> {
        self.vsock_services
            .iter()
            .find(|(spec, _, _)| spec.port == port)
            .map(|(_, handler, _)| Arc::clone(handler))
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

    pub fn seal(&mut self) -> Result<(), PluginError> {
        if self.sealed {
            return Err(PluginError::Halt("pipeline already sealed".into()));
        }
        // TODO (L3): conflicts surface the structured EKindConflict/EPortConflict envelope from the artifacts.
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
        // TODO: freeze all five ring chains.
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
        if event.syscall.is_some() {
            self.metrics.record_syscall();
        }
        for (index, interceptor) in self.trap_interceptors.iter().enumerate() {
            let fallback = MountScope::default();
            let scope = self.trap_scopes.get(index).unwrap_or(&fallback);
            match self.gate.evaluate(event, scope) {
                GateDecision::Allow => continue,
                GateDecision::Deny(verdict) => return Ok(verdict),
                GateDecision::FallThrough => {}
            }
            // see: cans/01-architecture.md#FFI-boundary-economics
            // TODO (L3): AOT hook fusion collapses chained crossings into one trampoline per ring event.
            self.try_foreign_submit()?;
            let verdict = {
                let outcome = quarantine_verdict(interceptor.on_trap(event, ctx)).await;
                self.foreign_submit_complete();
                outcome?
            };
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        Ok(Verdict::Pass)
    }

    pub async fn dispatch_process_exit(
        &self,
        event: &ProcessExit,
        ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        self.metrics.record_exec();
        // TODO (L3): exec_started pairs with this finish once the L3 exec spawn path dispatches it.
        self.metrics.exec_finished();
        for (index, interceptor) in self.trap_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.trap_scopes, index);
            if !matches!(
                self.gate.evaluate(&ctx.trap, &scope),
                GateDecision::FallThrough
            ) {
                continue;
            }
            quarantine_unit(interceptor.on_process_exit(event, ctx)).await?;
        }
        Ok(())
    }

    pub fn dispatch_fs_write(
        &self,
        path: &std::path::Path,
        data: &mut [u8],
        pid: u32,
        ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        for (index, interceptor) in self.fs_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.fs_scopes, index);
            if !Self::fs_scope_allows(&scope, path, ctx) {
                continue;
            }
            // see: cans/01-architecture.md#FFI-boundary-economics
            self.try_foreign_submit()?;
            let action = {
                let outcome = quarantine_sync(
                    || interceptor.on_fs_write(path, data, pid, ctx),
                    || {
                        Ok(FilterAction::Abort {
                            reason: "Plugin panicked".into(),
                        })
                    },
                );
                self.foreign_submit_complete();
                outcome?
            };
            if !matches!(action, FilterAction::Forward) {
                return Ok(action);
            }
        }
        Ok(FilterAction::Forward)
    }

    // TODO (L3): CIDR/port scope rules need a frame parser; only the device mask is enforced here.
    pub fn dispatch_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        for (index, interceptor) in self.net_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.net_scopes, index);
            if !scope.allows_device(VirtioDevice::Net) {
                continue;
            }
            // see: cans/01-architecture.md#FFI-boundary-economics
            self.try_foreign_submit()?;
            let action = {
                let outcome = quarantine_sync(
                    || interceptor.on_net_outbound(frame),
                    || {
                        Ok(FilterAction::Abort {
                            reason: "Plugin panicked".into(),
                        })
                    },
                );
                self.foreign_submit_complete();
                outcome?
            };
            if !matches!(action, FilterAction::Forward) {
                return Ok(action);
            }
        }
        Ok(FilterAction::Forward)
    }

    pub fn dispatch_net_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        for (index, interceptor) in self.net_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.net_scopes, index);
            if !scope.allows_device(VirtioDevice::Net) {
                continue;
            }
            let action = quarantine_sync(
                || interceptor.on_net_inbound(frame),
                || {
                    Ok(FilterAction::Abort {
                        reason: "Plugin panicked".into(),
                    })
                },
            )?;
            if !matches!(action, FilterAction::Forward) {
                return Ok(action);
            }
        }
        Ok(FilterAction::Forward)
    }

    pub async fn dispatch_console_input(
        &self,
        port: u32,
        bytes: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        for (index, interceptor) in self.console_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.console_scopes, index);
            if !scope.allows_device(VirtioDevice::Console) {
                continue;
            }
            quarantine_unit(interceptor.on_console_input(port, bytes)).await?;
        }
        Ok(())
    }

    pub async fn dispatch_console_output(
        &self,
        port: u32,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        for (index, interceptor) in self.console_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.console_scopes, index);
            if !scope.allows_device(VirtioDevice::Console) {
                continue;
            }
            quarantine_unit(interceptor.on_console_output(port, chunk)).await?;
        }
        Ok(())
    }

    pub async fn dispatch_exec_stdout(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        for (index, interceptor) in self.console_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.console_scopes, index);
            if !scope.allows_device(VirtioDevice::Console) {
                continue;
            }
            quarantine_unit(interceptor.on_exec_stdout(pid, exec_id, chunk)).await?;
        }
        Ok(())
    }

    pub async fn dispatch_exec_stderr(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        for (index, interceptor) in self.console_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.console_scopes, index);
            if !scope.allows_device(VirtioDevice::Console) {
                continue;
            }
            quarantine_unit(interceptor.on_exec_stderr(pid, exec_id, chunk)).await?;
        }
        Ok(())
    }

    // TODO (L3): host scope.devices has no VirtioDevice mapping; only the path filter is enforced here.
    pub fn dispatch_host_lsm_exec(
        &self,
        path: &std::path::Path,
        pid: u32,
    ) -> Result<Verdict, PluginError> {
        for (index, interceptor) in self.host_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.host_scopes, index);
            if !scope.allows_path(path) {
                continue;
            }
            let verdict = quarantine_sync(
                || interceptor.on_host_lsm_exec(path, pid),
                || {
                    Ok(Verdict::Drop {
                        errno: Some("EFAULT".into()),
                        reason: "Plugin panicked".into(),
                    })
                },
            )?;
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        Ok(Verdict::Pass)
    }

    // TODO (L3): pressure/keyring dispatch carries no path or device input, so no scope filter can evaluate here yet.
    pub fn dispatch_host_pressure(&self, level: u32) -> Result<Verdict, PluginError> {
        for interceptor in &self.host_interceptors {
            let verdict = quarantine_sync(
                || interceptor.on_host_pressure(level),
                || {
                    Ok(Verdict::Drop {
                        errno: Some("EFAULT".into()),
                        reason: "Plugin panicked".into(),
                    })
                },
            )?;
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        Ok(Verdict::Pass)
    }

    pub fn dispatch_host_keyring_access(&self, key: &str) -> Result<Verdict, PluginError> {
        for interceptor in &self.host_interceptors {
            let verdict = quarantine_sync(
                || interceptor.on_host_keyring_access(key),
                || {
                    Ok(Verdict::Drop {
                        errno: Some("EFAULT".into()),
                        reason: "Plugin panicked".into(),
                    })
                },
            )?;
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        Ok(Verdict::Pass)
    }
}
