//! examples/hook-matrix.rs
//!
//! Canonical depth example: the custom hook surface, not the config
//! surface. Where the swe-agent family configures wire/vfs/pty behavior
//! (SinkholeConfig, MaskRule, TokenShield), this file implements the
//! hooks themselves in one plugin, identically in hook-matrix.ts and
//! hook-matrix.py.
//!
//! SAME HOOK SET in all three SDKs, in dispatch order:
//!
//!   1. NetInterposer::on_net_outbound -> SyncVerdict. Frames containing
//!      the documentation-range marker 198.51.100. yield to the host for
//!      inspection (YieldToHost); all other frames Allow.
//!   2. FsInterposer::on_fs_lookup -> Ok(Some(FsInodeAttr)) for
//!      exactly one path, /workspace/.policy/identity, with mode 0o400
//!      (no permission fallback, per 02); Ok(None) — fall through —
//!      everywhere else.
//!   3. FsInterposer::on_fs_read -> sliding-window masking. A
//!      31-byte tail is carried between reads, so a secret token split
//!      across two reads is still masked from the split point onward and
//!      no single read ever emits a complete unmasked token (the token
//!      is 32 bytes; see the masking note on the function).
//!   4. ConsoleInterposer::on_console_input -> strips OSC52 clipboard
//!      sequences (ESC ] 52 ; … terminator) from guest-bound input.
//!   5. ConsoleInterposer::on_console_output -> masks sk_live_ tokens in
//!      the chunk before it reaches the host terminal.
//!   6. TrapInterposer::on_heartbeat -> counts heartbeats; a receipt
//!      whose uptime_ms has not advanced since the previous receipt
//!      increments the stall counter (observation only, per 07).
//!   7. TrapInterposer::on_process_exit -> records (pid, exit_code);
//!      non-zero exits increment the failure counter.
//!
//! Masking is same-length (token bytes overwritten with b'*'), because
//! on_fs_read / on_console_output mutate fixed-size buffers in place and
//! cannot change a chunk's length.
//!
//! GUARDS (not new dispatch rows; hook order 1-7 above is unchanged):
//!   - on_fs_lookup rejects a path whose `..` segments resolve outside
//!     the /workspace root with E_PATH_TRAVERSAL (05), before the
//!     identity check runs. Paths without `..` keep the old behavior:
//!     the identity inode, else fall through.
//!   - on_fs_write rejects any payload over the 64KB (65536-byte) FFI
//!     inspection ceiling with PluginError::BufferCeilingExceeded
//!     (plugin-trait.rs). That is the runtime inspection ceiling, NOT
//!     the swe-agent policy diff ceiling, which rejects with ENOSPC.
//!
//! GAPS (flagged, not silently invented):
//!   - SyncVerdict::YieldToHost(u64): the u64's meaning (queue id?
//!     reason code?) is specified nowhere. The outbound hook below
//!     passes 0 as an explicit placeholder.
//!   - FsInodeAttr.occ_metadata_hash: 02 lists mode, uid, gid, size,
//!     content for the struct; the hash field appears only in the
//!     struct definition with no stated semantics. The lookup hook
//!     below sets it to 0 as an explicit placeholder.
//!   - on_console_input / on_console_output take &mut Vec<u8> in plugin-trait.rs
//!     (whole-chunk mutation); there is no per-byte streaming hook, so
//!     the OSC52 stripper below buffers one chunk at a time.
//!
//! Cargo.toml:
//!   [dependencies]
//!   kelir = { version = "3.0", features = ["pty", "vfs", "wire"] }
//!   tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync"] }
//!   async-trait = "0.1"
//!   serde = { version = "1", features = ["derive"] }

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use kelir::{
    DriverChoice, HandlerCtx, KelirBuilder, Session, VmConfig,
    plugin::{
        AnyPlugin, ExecutionTier, KelirPlugin, PluginError, PluginTaxonomy,
        FsInterposer, SyncVerdict, TelemetryRole, ConsoleInterposer,
        TrapInterposer, FsInodeAttr, VirtioDevice, WatchdogEvent, WatchdogVerdict,
        NetInterposer,
    },
    protocol::{Heartbeat, ProcessExit},
};

/// Guest path served as a synthetic inode, and the secret-token shape the
/// masking hooks redact. TEST-NET-2 (198.51.100.0/24) is documentation
/// space, used here as a denylist marker, not a real peer address.
const IDENTITY_PATH: &str = "/workspace/.policy/identity";
const IDENTITY_CONTENT: &[u8] = b"policy-matrix-identity\n";
const DENY_MARKER: &[u8] = b"198.51.100.";
const TOKEN_PREFIX: &[u8] = b"sk_live_";
const TOKEN_SUFFIX_LEN: usize = 24;
const TOKEN_LEN: usize = TOKEN_PREFIX.len() + TOKEN_SUFFIX_LEN;
/// Bytes carried between reads: one less than the token length, so any
/// token split across a read boundary is reassembled in the window.
const CARRYOVER: usize = TOKEN_LEN - 1;

#[derive(Default)]
struct HookState {
    tail: Vec<u8>,
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
    pub fn heartbeats(&self) -> usize { self.state.lock().unwrap().heartbeats }
    pub fn stalls(&self) -> usize { self.state.lock().unwrap().stalls }
    pub fn exits(&self) -> usize { self.state.lock().unwrap().exits }
    pub fn failures(&self) -> usize { self.state.lock().unwrap().failures }
}

/// Overwrite every byte of a complete sk_live_ token (prefix + 24 alnum)
/// with b'*', in place. A token is only masked when all 32 bytes are
/// visible in the window; partial prefixes at the window's end are left
/// for the next read's tail, never emitted as a complete token.
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

/// Taxonomy: urn:kelir:sys:hook-matrix; rings: virtio-net, virtio-fs,
/// virtio-console, virtio-vsock (heartbeat/exit events arrive over vsock).
pub struct HookMatrixPlugin {
    state: Arc<Mutex<HookState>>,
}

impl HookMatrixPlugin {
    pub fn new() -> Self {
        Self { state: Arc::new(Mutex::new(HookState::default())) }
    }
    pub fn handle(&self) -> HookHandle {
        HookHandle { state: Arc::clone(&self.state) }
    }
}

impl KelirPlugin for HookMatrixPlugin {
    fn name(&self) -> &'static str { "hook-matrix" }

    fn taxonomy(&self) -> PluginTaxonomy {
        PluginTaxonomy {
            boundary: "urn:kelir:sys:hook-matrix",
            rings: &[
                VirtioDevice::Net,
                VirtioDevice::Fs,
                VirtioDevice::Console,
                VirtioDevice::Vsock,
            ],
            role: TelemetryRole::Gate,
            tier: ExecutionTier::Native,
            mutation_targets: &["net-frame", "vfs-read", "pty-stream"],
        }
    }

    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        // Fresh hook state per forked child; the tail must not leak
        // across fork boundaries (a carried prefix belongs to one VM).
        Box::new(Self { state: Arc::new(Mutex::new(HookState::default())) })
    }
}

impl NetInterposer for HookMatrixPlugin {
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<SyncVerdict, PluginError> {
        // Sliding search, no allocation: any frame carrying the denylist
        // marker yields to host inspection instead of leaving the TAP.
        if frame.windows(DENY_MARKER.len()).any(|w| w == DENY_MARKER) {
            // GAP: the u64 payload of YieldToHost is unspecified; 0 is
            // an explicit placeholder (see file header).
            return Ok(SyncVerdict::YieldToHost(0));
        }
        Ok(SyncVerdict::Allow)
    }
}

/// Lexical workspace escape check, `..` paths only: resolve `..`
/// segments without touching the filesystem (no symlink resolution
/// here; the reconciler's RESOLVE_BENEATH guard owns that) and report
/// whether the resolved path left the /workspace root. Paths with no
/// `..` segment are untouched by this guard — they keep the old
/// identity-or-fall-through behavior.
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
    fn on_fs_lookup(&self, path: &Path) -> Result<Option<FsInodeAttr>, PluginError> {
        // GUARD: traversal first — an escaping path is rejected, never
        // served and never silently fallen through to the host fs.
        if escapes_workspace(path) {
            return Err(PluginError::reject(
                "E_PATH_TRAVERSAL",
                "lookup escapes the /workspace root",
            ));
        }
        if path.as_os_str() != IDENTITY_PATH {
            return Ok(None);
        }
        Ok(Some(FsInodeAttr {
            // 0o400 exactly, no permission fallback (02).
            mode: 0o400,
            uid: 0,
            gid: 0,
            size: IDENTITY_CONTENT.len() as u64,
            // GAP: hash semantics unspecified; 0 is an explicit
            // placeholder (see file header).
            occ_metadata_hash: 0,
            content: IDENTITY_CONTENT.to_vec(),
        }))
    }

    fn on_fs_read(&self, _path: &Path, buffer: &mut [u8]) -> Result<(), PluginError> {
        let mut state = self.state.lock().unwrap();
        // Window = carried tail + this chunk. Mask over the window, then
        // write the chunk's portion back and carry the last CARRYOVER
        // bytes forward. Invariant: a token split across reads is
        // masked from the split point onward in the later chunk, and no
        // single chunk ever contains a complete unmasked token, because
        // the earlier chunk held at most a 31-byte fragment of it.
        let mut window = state.tail.clone();
        window.extend_from_slice(buffer);
        mask_tokens(&mut window);
        let chunk_start = window.len() - buffer.len();
        buffer.copy_from_slice(&window[chunk_start..]);
        state.tail = window[window.len().saturating_sub(CARRYOVER)..].to_vec();
        Ok(())
    }

    // GUARD (not a new dispatch row): the 64KB FFI inspection ceiling.
    // A payload over it is rejected with BufferCeilingExceeded — the
    // runtime inspection error, distinct from the swe-agent policy
    // diff ceiling, which rejects oversize diffs with ENOSPC.
    fn on_fs_write(
        &self, _path: &Path, data: &[u8], _pid: u32, _ctx: &HandlerCtx,
    ) -> Result<SyncVerdict, PluginError> {
        const FFI_INSPECTION_CEILING: usize = 64 * 1024;
        if data.len() > FFI_INSPECTION_CEILING {
            return Err(PluginError::BufferCeilingExceeded {
                size: data.len(),
                max_bytes: FFI_INSPECTION_CEILING,
            });
        }
        Ok(SyncVerdict::Allow)
    }
}

#[async_trait]
impl ConsoleInterposer for HookMatrixPlugin {
    async fn on_console_input(&self, bytes: &mut Vec<u8>) -> Result<(), PluginError> {
        // Strip OSC52 clipboard exfiltration sequences: ESC ] 52 ; …
        // terminated by BEL (0x07) or ST (ESC \). Everything else passes.
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

    async fn on_console_output(&self, chunk: &mut Vec<u8>) -> Result<(), PluginError> {
        // Single-chunk masking; the cross-chunk case is the vfs read
        // path's sliding window above, not the terminal path.
        mask_tokens(chunk);
        Ok(())
    }
}

#[async_trait]
impl TrapInterposer for HookMatrixPlugin {
    async fn on_heartbeat(&self, event: &Heartbeat) -> Result<(), PluginError> {
        let mut state = self.state.lock().unwrap();
        // Observation only (07): count, and note stalls, but never gate.
        if state.last_uptime_ms == Some(event.uptime_ms) && state.heartbeats > 0 {
            state.stalls += 1;
        }
        state.last_uptime_ms = Some(event.uptime_ms);
        state.heartbeats += 1;
        Ok(())
    }

    async fn on_process_exit(&self, event: &ProcessExit, _ctx: &HandlerCtx) -> Result<(), PluginError> {
        let mut state = self.state.lock().unwrap();
        state.exits += 1;
        if event.exit_code != 0 {
            state.failures += 1;
        }
        Ok(())
    }

    // This plugin does not touch the watchdog; the default verdict keeps
    // the swe-agent watchdog behavior (Reboot after the armed timeout).
    async fn on_watchdog(
        &self, _event: &WatchdogEvent,
    ) -> Result<WatchdogVerdict, PluginError> {
        Ok(WatchdogVerdict::ExecuteDefault)
    }
}

// ============================================================================
// BUILDER + MAIN — boot, exercise the hooks, report counters
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let plugin = HookMatrixPlugin::new();
    let handle = plugin.handle();

    let session: Session = KelirBuilder::new(VmConfig {
        vcpus: 2,
        memory_mb: 2048,
        kernel: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs: "./fixtures/rootfs-hooks.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver: DriverChoice::Auto,
        entropy_seed: None,
    })
    .plugin(plugin)
    .boot()
    .await?;
    eprintln!("[kelir] driver={} boot={}ms", session.driver_name(), session.boot_ms());

    // Reads the synthetic inode (lookup hook) through the masking read
    // path, and produces process-exit + heartbeat traffic for the trap
    // hooks. PTY input/output hooks fire on the session's terminal.
    let probe = session
        .spawn("cat")
        .args([IDENTITY_PATH])
        .cwd("/workspace")
        .await?;
    eprintln!("[kelir] cat identity -> exit={}", probe.exit_code);

    // Let at least one heartbeat receipt arrive before reporting.
    tokio::time::sleep(Duration::from_millis(100)).await;
    eprintln!(
        "[kelir] heartbeats={} stalls={} exits={} failures={}",
        handle.heartbeats(),
        handle.stalls(),
        handle.exits(),
        handle.failures(),
    );

    session.terminate().await?;
    Ok(())
}
