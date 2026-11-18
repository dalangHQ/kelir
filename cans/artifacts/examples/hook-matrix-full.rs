// see: cans/02-plugin-model.md#Segregated-ring-traits
// see: cans/02-plugin-model.md#TrapFilter-predicates
// see: cans/04-terminal-plane.md#Terminal-pipeline
// see: cans/05-storage-plane.md#In-flight-read-masking

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kelir::async_trait;
use kelir_interface::plugin_model::HandlerCtx;

use kelir::{
    driver_interface::{DriverPolicy, WxPolicy},
    plugin_model::{
        AnyPlugin, ConsoleInterposer, FilterAction, FsInterposer, HostInterposer, KelirPlugin,
        LifecycleCtx, LifecycleParticipant, NetInterposer, NoDeps, PluginDeps, PluginError,
        SystemTelemetryParticipant, TrapInterposer, Verdict, WatchdogEvent, WatchdogVerdict,
    },
    protocol_wire::{Errno, Heartbeat, PatchPayload, ProcessExit, SuspendUntil, TrapEvent},
    KelirBuilder, SnapshotDescriptor, TrapFilter, VmConfig,
};

const IDENTITY_PATH: &str = "/workspace/.policy/identity";
const IDENTITY_CONTENT: &[u8] = b"policy-matrix-identity\n";
const DENY_MARKER: &[u8] = b"198.51.100.";
const TOKEN_PREFIX: &[u8] = b"sk_live_";
const TOKEN_SUFFIX_LEN: usize = 24;
const TOKEN_LEN: usize = TOKEN_PREFIX.len() + TOKEN_SUFFIX_LEN;

const CARRYOVER: usize = TOKEN_LEN - 1;

#[derive(Default)]
struct HookState {
    tail: Vec<u8>,
    console_tail: Vec<u8>,
    heartbeats: usize,
    last_uptime_ms: Option<u64>,
    stalls: usize,
    exits: usize,
    failures: usize,
    traps: usize,
    fs_locks: usize,
    fs_unlinks: usize,
    fs_renames: usize,
    net_inbound: usize,
    exec_stdout: usize,
    exec_stderr: usize,
    host_preempts: usize,
    vm_inits: usize,
    vm_readys: usize,
    vm_shutdowns: usize,
    pre_commits: usize,
    post_commits: usize,
    memory_pressures: usize,
    masked_records: usize,
}

#[derive(Clone)]
pub struct HookHandle {
    state: Arc<Mutex<HookState>>,
}

impl HookHandle {
    pub fn heartbeats(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .heartbeats
    }
    pub fn stalls(&self) -> usize {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).stalls
    }
    pub fn exits(&self) -> usize {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).exits
    }
    pub fn failures(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .failures
    }
    pub fn traps(&self) -> usize {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).traps
    }
    pub fn fs_locks(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .fs_locks
    }
    pub fn fs_unlinks(&self) -> usize {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).fs_unlinks
    }
    pub fn fs_renames(&self) -> usize {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).fs_renames
    }
    pub fn net_inbound(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .net_inbound
    }
    pub fn exec_stdout(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .exec_stdout
    }
    pub fn exec_stderr(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .exec_stderr
    }
    pub fn host_preempts(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .host_preempts
    }
    pub fn vm_shutdowns(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .vm_shutdowns
    }
    pub fn memory_pressures(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .memory_pressures
    }
    pub fn masked_records(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .masked_records
    }
}

fn mask_tokens(window: &mut [u8]) {
    if window.len() < TOKEN_LEN {
        return;
    }
    let mut i = 0;
    while i + TOKEN_LEN <= window.len() {
        if &window[i..i + TOKEN_PREFIX.len()] == TOKEN_PREFIX
            && window[i + TOKEN_PREFIX.len()..i + TOKEN_LEN]
                .iter()
                .all(|b| b.is_ascii_alphanumeric())
        {
            for b in &mut window[i..i + TOKEN_LEN] {
                *b = b'*';
            }
            i += TOKEN_LEN;
        } else {
            i += 1;
        }
    }
}

// see: cans/10-attestation.md#Hook-trace
// Ground truth for the pre-redaction acceptance check: how many token
// records this plugin masked, counted on the same window mask_tokens
// rewrites, before the rewrite. The receipt must carry one
// pre-redaction digest per masked record; fewer means the raw digest
// was never committed.
fn count_tokens(window: &[u8]) -> usize {
    if window.len() < TOKEN_LEN {
        return 0;
    }
    let mut count = 0;
    let mut i = 0;
    while i + TOKEN_LEN <= window.len() {
        if &window[i..i + TOKEN_PREFIX.len()] == TOKEN_PREFIX
            && window[i + TOKEN_PREFIX.len()..i + TOKEN_LEN]
                .iter()
                .all(|b| b.is_ascii_alphanumeric())
        {
            count += 1;
            i += TOKEN_LEN;
        } else {
            i += 1;
        }
    }
    count
}

#[derive(Clone, Default)]
pub struct HookMatrixFullPlugin {
    state: Arc<Mutex<HookState>>,
}

impl HookMatrixFullPlugin {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HookState::default())),
        }
    }
    pub fn handle(&self) -> HookHandle {
        HookHandle {
            state: Arc::clone(&self.state),
        }
    }
}

impl KelirPlugin for HookMatrixFullPlugin {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn name(&self) -> &'static str {
        "hook-matrix-full"
    }

    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        Box::new(Self {
            state: Arc::new(Mutex::new(HookState::default())),
        })
    }
}

impl PluginDeps for HookMatrixFullPlugin {
    type Deps = NoDeps;
}

#[async_trait]
impl LifecycleParticipant for HookMatrixFullPlugin {
    async fn on_vm_init(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .vm_inits += 1;
        Ok(())
    }
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .vm_readys += 1;
        Ok(())
    }
    async fn on_vm_shutdown(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .vm_shutdowns += 1;
        Ok(())
    }
    async fn on_pre_commit(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pre_commits += 1;
        Ok(())
    }
    async fn on_post_commit(
        &self,
        _ctx: &mut LifecycleCtx<'_>,
        _committed: bool,
    ) -> Result<(), PluginError> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .post_commits += 1;
        Ok(())
    }
    async fn on_host_preempt(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .host_preempts += 1;
        Ok(())
    }
fn on_host_containment_breach(&self, _pid: u32, _path: &Path, _kind: &str) -> Result<(), PluginError> {
    Ok(())
}
}

impl NetInterposer for HookMatrixFullPlugin {
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        if frame.windows(DENY_MARKER.len()).any(|w| w == DENY_MARKER) {
            return Ok(FilterAction::Suspend {
                until: SuspendUntil::Deadline(0),
            });
        }
        Ok(FilterAction::Forward)
    }

    fn on_net_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        let _ = frame;
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .net_inbound += 1;
        Ok(FilterAction::Forward)
    }
fn on_socket_connect(&self, _pid: u32, _proto: u8, _ip: [u8; 4], _port: u16) -> Result<Verdict, PluginError> {
    Ok(Verdict::Pass)
}
fn on_dns_query(&self, _pid: u32, _domain: &str, _qtype: u16) -> Result<DnsVerdict, PluginError> {
    Ok(DnsVerdict::Forward)
}
fn on_socket_bind(&self, _pid: u32, _proto: u8, _ip: [u8; 4], _port: u16) -> Result<Verdict, PluginError> {
    Ok(Verdict::Pass)
}
fn on_fs_setattr(&self, _path: &Path, _mode: u32, _size: u64) -> Result<Verdict, PluginError> {
    Ok(Verdict::Pass)
}
fn on_fs_symlink(&self, _target: &Path, _link: &Path) -> Result<Verdict, PluginError> {
    Ok(Verdict::Pass)
}
fn on_fs_mkdir(&self, _path: &Path, _mode: u32) -> Result<Verdict, PluginError> {
    Ok(Verdict::Pass)
}
fn on_fs_readdir(&self, _path: &Path, _entries: &mut Vec<DirEntry>) -> Result<FilterAction, PluginError> {
    Ok(FilterAction::Forward)
}
fn on_fs_readlink(&self, _path: &Path) -> Result<Verdict, PluginError> {
    Ok(Verdict::Pass)
}
}

fn escapes_workspace(path: &Path) -> bool {
    use std::path::Component;
    if !path.components().any(|c| matches!(c, Component::ParentDir)) {
        return false;
    }
    let mut stack: Vec<&str> = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => {
                if let Some(s) = segment.to_str() {
                    stack.push(s);
                }
            }
            Component::ParentDir => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.first() != Some(&"workspace")
}

impl FsInterposer for HookMatrixFullPlugin {
    fn on_fs_lookup(&self, path: &Path) -> Result<Verdict, PluginError> {
        if escapes_workspace(path) {
            // PathTraversal maps to EPERM in the driver; the closed Errno
            // enum has no stringly form (02-plugin-model.md#Unified-verdict).
            return Ok(Verdict::Drop {
                errno: Errno::PathTraversal,
                reason: "lookup escapes the /workspace root".into(),
            });
        }

        if path.as_os_str() != IDENTITY_PATH {
            return Ok(Verdict::Pass);
        }
        Ok(Verdict::Patch(PatchPayload::FsInode {
            mode: 0o400,
            uid: 0,
            gid: 0,
            size: IDENTITY_CONTENT.len() as u64,
            content: IDENTITY_CONTENT.to_vec(),
        }))
    }

    fn on_fs_read(&self, _path: &Path, buffer: &[u8]) -> Result<Vec<u8>, PluginError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());

        let mut window = state.tail.clone();
        window.extend_from_slice(buffer);
        state.masked_records += count_tokens(&window);
        mask_tokens(&mut window);
        let chunk_start = window.len() - buffer.len();
        let out = window[chunk_start..].to_vec();
        state.tail = window[window.len().saturating_sub(CARRYOVER)..].to_vec();
        Ok(out)
    }

    fn on_fs_write(
        &self,
        _path: &Path,
        _data: &mut [u8],
        _pid: u32,
        _ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        Ok(FilterAction::Forward)
    }

    fn on_fs_lock(
        &self,
        _path: &Path,
        _flock_cmd: i32,
        _flock_type: i16,
        _pid: u32,
    ) -> Result<Verdict, PluginError> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .fs_locks += 1;
        Ok(Verdict::Pass)
    }

    fn on_fs_unlink(&self, path: &Path, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.fs_unlinks += 1;

        // Destructive delete interception: redirect to quarantine or trigger task-scoped approval
        if path.ends_with(".protected_source") {
            // Non-deadlocking task freeze: calling process suspended, VirtIO ring yields
            return Ok(Verdict::Suspend {
                until: SuspendUntil::Approval,
            });
        }

        if path.extension().is_some_and(|ext| ext == "trashable") {
            if let Some(name) = path.file_name() {
                let quarantined = Path::new("/workspace/.agentsh_trash").join(name);
                return Ok(Verdict::Patch(PatchPayload::FsPathRedirect {
                    target_path: quarantined.to_string_lossy().to_string(),
                }));
            }
        }

        Ok(Verdict::Pass)
    }

    fn on_fs_rename(&self, _src: &Path, dst: &Path, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).fs_renames += 1;
        if escapes_workspace(dst) {
            return Ok(Verdict::Drop {
                errno: Errno::PathTraversal,
                reason: "rename destination escapes /workspace".into(),
            });
        }
        Ok(Verdict::Pass)
    }
}

impl HostInterposer for HookMatrixFullPlugin {
    fn on_host_lsm_exec(&self, _path: &Path, _pid: u32) -> Result<Verdict, PluginError> {
        Ok(Verdict::Pass)
    }
    fn on_host_pressure(&self, _level: u32) -> Result<Verdict, PluginError> {
        Ok(Verdict::Pass)
    }
    fn on_host_keyring_access(&self, _key: &str) -> Result<Verdict, PluginError> {
        Ok(Verdict::Pass)
    }
}

#[async_trait]
impl ConsoleInterposer for HookMatrixFullPlugin {
    async fn on_console_input(&self, _port: u32, bytes: &[u8]) -> Result<Vec<u8>, PluginError> {
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i..].starts_with(b"\x1b]52;") {
                i += 5;
                while i < bytes.len()
                    && bytes[i] != 0x07
                    && !(bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'\\'))
                {
                    i += 1;
                }
                if bytes.get(i) == Some(&0x1b) {
                    i += 2;
                } else if i < bytes.len() {
                    i += 1;
                }
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        Ok(out)
    }

    async fn on_console_output(&self, _port: u32, chunk: &[u8]) -> Result<Vec<u8>, PluginError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let mut window = state.console_tail.clone();
        window.extend_from_slice(chunk);
        state.masked_records += count_tokens(&window);
        mask_tokens(&mut window);
        let chunk_start = window.len() - chunk.len();
        let out = window[chunk_start..].to_vec();
        state.console_tail = window[window.len().saturating_sub(CARRYOVER)..].to_vec();
        Ok(out)
    }

    async fn on_exec_stdout(
        &self,
        _pid: u32,
        _exec_id: u64,
        chunk: &[u8],
    ) -> Result<Vec<u8>, PluginError> {
        let masked = count_tokens(chunk);
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.exec_stdout += 1;
            state.masked_records += masked;
        }
        let mut out = chunk.to_vec();
        mask_tokens(&mut out);
        Ok(out)
    }

    async fn on_exec_stderr(
        &self,
        _pid: u32,
        _exec_id: u64,
        chunk: &[u8],
    ) -> Result<Vec<u8>, PluginError> {
        let masked = count_tokens(chunk);
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.exec_stderr += 1;
            state.masked_records += masked;
        }
        let mut out = chunk.to_vec();
        mask_tokens(&mut out);
        Ok(out)
    }
async fn on_exec_stdin(&self, _pid: u32, _id: u64, chunk: &[u8]) -> Result<Vec<u8>, PluginError> {
    Ok(chunk.to_vec())
}
}

#[async_trait]
impl TrapInterposer for HookMatrixFullPlugin {
    async fn on_trap(&self, event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).traps += 1;

        // Process lineage validation without host ptrace
        if event.tree_depth > 16 {
            return Ok(Verdict::Abort { reason: "fork bomb mitigation: max process depth exceeded".into() });
        }
        Ok(Verdict::Pass)
    }

    async fn on_process_exit(
        &self,
        event: &ProcessExit,
        ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.exits += 1;
            if event.exit_code != 0 {
                state.failures += 1;
            }
        }
        // see: cans/10-attestation.md#Hook-trace
        // hook_trace read: every hook that fired for the trapped process
        // carries a HookSpan (plugin, ring, verdict_kind, duration_ns,
        // ordinal); diagnostic only, excluded from Merkle roots. The
        // comment-pinned HookSpan case (the hook-trace-coverage vector
        // this example stands in for) asserts the ring + ordinal pair
        // below: a span for this plugin on the Trap ring at ordinal 0.
        //
        // HookSpan case (vectors.json cannot express a hook-trace
        // expectation today; pinned here as Agent 3's negative vectors
        // are): { plugin: "hook-matrix-full", ring: Trap, verdict_kind:
        // Pass, duration_ns: 0, ordinal: 0 }
        if let Ok(receipt) = ctx.receipt().await {
            let _ = receipt.hooks(); // &[HookSpan]; populated per fired hook
            // see: cans/10-attestation.md#Hook-trace
            // WS-E acceptance: the receipt must commit the raw
            // pre-redaction digest for every record this plugin masked.
            // A listed masked record without its digest, or masked
            // records with no digests at all on the receipt, fails the
            // run instead of passing silently.
            if !receipt.masked_records_complete() {
                return Err(PluginError::Halt(
                    "receipt lists a masked record without its pre-redaction digest".into(),
                ));
            }
            let masked = self
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .masked_records;
            if masked > 0 && receipt.masked_records().is_empty() {
                return Err(PluginError::Halt(
                    "plugin masked records but the receipt carries no pre-redaction digests"
                        .into(),
                ));
            }
        }
        Ok(())
    }
async fn on_signal(&self, _sender: u32, _target: u32, _sig: i32) -> Result<Verdict, PluginError> {
    Ok(Verdict::Pass)
}
async fn on_fork(&self, _parent: u32, _child: u32) -> Result<Verdict, PluginError> {
    Ok(Verdict::Pass)
}
}

#[async_trait]
impl SystemTelemetryParticipant for HookMatrixFullPlugin {
    async fn on_memory_pressure(
        &self,
        _available_mb: u32,
        _total_mb: u32,
    ) -> Result<(), PluginError> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .memory_pressures += 1;
        Ok(())
    }

    async fn on_heartbeat(&self, event: &Heartbeat) -> Result<(), PluginError> {
        let uptime_ms = event.uptime_ms;
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());

        if state.last_uptime_ms == Some(uptime_ms) && state.heartbeats > 0 {
            state.stalls += 1;
        }
        state.last_uptime_ms = Some(uptime_ms);
        state.heartbeats += 1;
        Ok(())
    }

    async fn on_watchdog(&self, _event: &WatchdogEvent) -> Result<WatchdogVerdict, PluginError> {
        Ok(WatchdogVerdict::ExecuteDefault)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    kelir::block_on(async_main())?
}

async fn async_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let plugin = HookMatrixFullPlugin::new();
    let handle = plugin.handle();

    let mut session = KelirBuilder::new(VmConfig {
        tenant_id: None,
        vcpus: 2,
        memory_mb: 2048,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-hooks.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver_policy: DriverPolicy {
            emulated_tier_classification: None,
            driver: None,
            require_hardware_vmm: false,
            allow_emulated_fallback: true,
            verify_hw_lockfile: false,
            wx_enforcement: WxPolicy::PermissiveAudit,
            ffi_worker_threads: 2,
            ffi_max_ops_per_sec: 1000,
            ffi_queue_capacity: 16384,
            ffi_max_buffer_bytes: 16777216,
            ffi_zero_copy_shm_threshold_bytes: 65536,
            ffi_allow_async_worker_yield: true,
        },
        page_size_kb: 4,
        deterministic_clock: false,
        network_off: false,
        pinned_clocksource: None,
        ns_per_instruction: None,
        preemption_grace_ms: 0,
        entropy_seed: None,
    })
    .plugin_instance(plugin)
    // see: cans/02-plugin-model.md#TrapFilter-predicates
    // The predicate chain: cwd/env/uid/argv_regex constraints live in the
    // filter, not in the handler body (AND-ed predicates; a handler-body
    // guard duplicating them is redundant and should not be written).
    .trap(
        TrapFilter::binary("git")
            .with_arg("push")
            .with_cwd("/workspace/**")
            .with_env("CI", "1")
            .with_uid(0)
            .argv_matches(r"^--force$"),
        |_, _| {
            // Handler body carries no cwd/argv guard: the filter already
            // pinned those predicates before the handler was admitted.
            Ok(Verdict::Drop {
                errno: Errno::Epolicy,
                reason: "forced push denied by policy".into(),
            })
        },
    )
    // see: cans/02-plugin-model.md#Hook-context-restriction
    // on_trap_deny(event, verdict, session): the verdict parameter is the
    // DENYING Action (the unified enum), so break-glass can say "denied by
    // X for Y, quarantine" — see: 03-sdk-architecture.md#Session-surfaces.
    .deny(|d| {
        d.on_trap_deny(|event, verdict, session| {
            // Signature: (event, verdict, session); verdict is the DENYING
            // Action (the unified enum), not the old Verdict split.
            let _ = (event, verdict, session);
            async move { Ok::<(), PluginError>(()) }
        })
        .end()
    })
    .seal()?
    .boot()
    .await?;
    session.terminal.resize(120, 40)?;
    eprintln!("[kelir] host pressure={}", session.host.pressure_level());
    let slot = session.step_evaluator().begin_step(
        session.lineage(1),
        SnapshotDescriptor {
            id: "hook-matrix-full-probe".into(),
            dirty_bytes: 0,
            created_at_ns: 0,
            parent_digest: None,
            guest_clock: None,
        },
    )?;
    let node = session.step_evaluator().commit_step(session.lineage(1))?;
    eprintln!("[kelir] checkpoint slot={slot} node={node}");

    // Drive the 24-ring hook matrix by accessing the synthetic FUSE inode
    let probe = session.spawn("cat", &[IDENTITY_PATH], None).await?;
    eprintln!("[kelir] cat identity -> exit={}", probe.exit_code);
    assert_eq!(probe.exit_code, 0);
    assert_eq!(probe.stdout.as_slice(), IDENTITY_CONTENT);

    // Drive VFS mutation interposer: unlink redirection test
    let unlink_test = session.spawn("rm", &["/workspace/file.trashable"], None).await?;
    assert_eq!(unlink_test.exit_code, 0);
    assert!(handle.fs_unlinks() > 0, "on_fs_unlink failed to intercept deletion");

    kelir::time::sleep(Duration::from_millis(100)).await;
    eprintln!(
        "[kelir] heartbeats={} stalls={} exits={} failures={} traps={} fs_locks={} net_inbound={} exec_stdout={} exec_stderr={} host_preempts={} vm_shutdowns={} memory_pressures={}",
        handle.heartbeats(),
        handle.stalls(),
        handle.exits(),
        handle.failures(),
        handle.traps(),
        handle.fs_locks(),
        handle.net_inbound(),
        handle.exec_stdout(),
        handle.exec_stderr(),
        handle.host_preempts(),
        handle.vm_shutdowns(),
        handle.memory_pressures(),
    );

    assert!(handle.traps() > 0, "no traps recorded across execution");
    assert!(
        handle.fs_locks() > 0 || handle.exec_stdout() > 0,
        "fs/exec interposers failed to record telemetry"
    );
    assert_eq!(handle.failures(), 0, "unexpected step failures occurred");

    session.stop().await?;
    Ok(())
}
