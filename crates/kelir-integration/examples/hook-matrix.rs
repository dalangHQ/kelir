// see: cans/02-plugin-model.md#Segregated-ring-traits

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use kelir_interface::plugin_model::HandlerCtx;

use kelir::{
    KelirBuilder, SnapshotDescriptor, VmConfig,
    driver_interface::{DriverPolicy, WxPolicy},
    plugin_model::{
        AnyPlugin, ConsoleInterposer, FilterAction, FsInterposer, HostInterposer, KelirPlugin,
        NetInterposer, PluginError, SystemTelemetryParticipant, TrapInterposer, Verdict,
        WatchdogEvent, WatchdogVerdict,
    },
    protocol_wire::{PatchPayload, ProcessExit, SuspendUntil},
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

// see: cans/04-terminal-plane.md#Terminal-pipeline
fn safe_console_emit_len(window: &[u8]) -> usize {
    let emit = window.len().saturating_sub(CARRYOVER);
    back_off_incomplete_ansi(window, back_off_incomplete_utf8(window, emit))
}

fn back_off_incomplete_utf8(window: &[u8], emit: usize) -> usize {
    if emit == 0 || emit >= window.len() {
        return emit;
    }
    let mut start = emit;
    let mut continuations = 0;
    while start > 0 && continuations < 3 && (0x80..0xC0).contains(&window[start - 1]) {
        start -= 1;
        continuations += 1;
    }
    if start == 0 {
        return emit;
    }
    let lead = window[start - 1];
    let needed = if (0xC2..0xE0).contains(&lead) {
        2
    } else if (0xE0..0xF0).contains(&lead) {
        3
    } else if (0xF0..0xF5).contains(&lead) {
        4
    } else {
        return emit;
    };
    if start - 1 + needed > emit {
        start - 1
    } else {
        emit
    }
}

fn back_off_incomplete_ansi(window: &[u8], emit: usize) -> usize {
    if emit == 0 {
        return emit;
    }
    let mut escape = None;
    for (index, byte) in window.iter().enumerate().take(emit) {
        if *byte == 0x1b {
            escape = Some(index);
        }
    }
    let Some(escape) = escape else {
        return emit;
    };
    if escape + 1 >= emit {
        return escape;
    }
    match window[escape + 1] {
        b'[' => {
            for byte in &window[escape + 2..emit] {
                if (0x40..0x7F).contains(byte) {
                    return emit;
                }
            }
            escape
        }
        b']' => {
            let mut index = escape + 2;
            while index < emit {
                if window[index] == 0x07 {
                    return emit;
                }
                if window[index] == 0x1b && window.get(index + 1) == Some(&b'\\') {
                    return emit;
                }
                index += 1;
            }
            escape
        }
        _ => emit,
    }
}

#[derive(Clone, Default)]
pub struct HookMatrixPlugin {
    state: Arc<Mutex<HookState>>,
}

impl HookMatrixPlugin {
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

impl KelirPlugin for HookMatrixPlugin {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn name(&self) -> &'static str {
        "hook-matrix"
    }

    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        Box::new(Self {
            state: Arc::new(Mutex::new(HookState::default())),
        })
    }
}

impl NetInterposer for HookMatrixPlugin {
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        if frame.windows(DENY_MARKER.len()).any(|w| w == DENY_MARKER) {
            return Ok(FilterAction::Suspend {
                until: SuspendUntil::Deadline(0),
            });
        }
        Ok(FilterAction::Forward)
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

impl FsInterposer for HookMatrixPlugin {
    fn on_fs_lookup(&self, path: &Path) -> Result<Verdict, PluginError> {
        if escapes_workspace(path) {
            return Ok(Verdict::Drop {
                errno: Some("E_PATH_TRAVERSAL".into()),
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

    fn on_fs_read(&self, _path: &Path, buffer: &mut [u8]) -> Result<(), PluginError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());

        let mut window = state.tail.clone();
        window.extend_from_slice(buffer);
        mask_tokens(&mut window);
        let chunk_start = window.len() - buffer.len();
        buffer.copy_from_slice(&window[chunk_start..]);
        state.tail = window[window.len().saturating_sub(CARRYOVER)..].to_vec();
        Ok(())
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
}

impl HostInterposer for HookMatrixPlugin {
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
impl ConsoleInterposer for HookMatrixPlugin {
    async fn on_console_input(&self, _port: u32, bytes: &mut Vec<u8>) -> Result<(), PluginError> {
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
        *bytes = out;
        Ok(())
    }

    async fn on_console_output(&self, _port: u32, chunk: &mut Vec<u8>) -> Result<(), PluginError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let mut window = state.console_tail.clone();
        window.extend_from_slice(chunk);
        mask_tokens(&mut window);
        let emit = safe_console_emit_len(&window);
        *chunk = window[..emit].to_vec();
        state.console_tail = window[emit..].to_vec();
        Ok(())
    }
}

#[async_trait]
impl TrapInterposer for HookMatrixPlugin {
    async fn on_process_exit(
        &self,
        event: &ProcessExit,
        _ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.exits += 1;
        if event.exit_code != 0 {
            state.failures += 1;
        }
        Ok(())
    }
}

#[async_trait]
impl SystemTelemetryParticipant for HookMatrixPlugin {
    async fn on_heartbeat(&self, uptime_ms: u64) -> Result<(), PluginError> {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let plugin = HookMatrixPlugin::new();
    let handle = plugin.handle();

    let mut session = KelirBuilder::new(VmConfig {
        vcpus: 2,
        memory_mb: 2048,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-hooks.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver_policy: DriverPolicy {
            driver: None,
            require_hardware_vmm: false,
            allow_emulated_fallback: true,
            verify_hw_lockfile: false,
            wx_enforcement: WxPolicy::PermissiveAudit,
            ffi_worker_threads: 2,
            ffi_max_ops_per_sec: 1000,
        },
        page_size_kb: 4,
        deterministic_clock: false,
        network_off: false,
        pinned_clocksource: None,
        ns_per_instruction: None,
        preemption_grace_ms: 0,
        entropy_seed: None,
    })
    .plugin(plugin)
    .boot()
    .await?;
    session.terminal.resize(120, 40)?;
    eprintln!("[kelir] host pressure={}", session.host.pressure_level());
    let slot = session.checkpoint.create_step(
        session.lineage(1),
        SnapshotDescriptor {
            id: "hook-matrix-probe".into(),
            dirty_bytes: 0,
            created_at_ns: 0,
            parent_digest: None,
            guest_clock: None,
        },
    )?;
    let node = session.checkpoint.commit_step(session.lineage(1))?;
    eprintln!("[kelir] checkpoint slot={slot} node={node}");
    // TODO (L3): spawn cat IDENTITY_PATH in the guest and drive the hook matrix over it.

    tokio::time::sleep(Duration::from_millis(100)).await;
    eprintln!(
        "[kelir] heartbeats={} stalls={} exits={} failures={}",
        handle.heartbeats(),
        handle.stalls(),
        handle.exits(),
        handle.failures(),
    );

    session.stop()?;
    Ok(())
}
