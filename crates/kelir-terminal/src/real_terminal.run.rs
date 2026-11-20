// see: cans/04-terminal-plane.md#Terminal-pipeline
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use kelir_interface::plugin_model::PluginError;
use kelir_interface::telemetry::MetricRegistry;
use kelir_interface::terminal::{LogSegment, LogStream, TerminalOps, TerminalWiring};

use crate::vt100::Vt100StateMachine;

// see: cans/04-terminal-plane.md#Terminal-pipeline
#[derive(Debug, Clone, Copy, Default)]
// TODO (L3) [terminal_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow masks the unused item, so the stubbed caller can stay unwired while the build stays green.
#[allow(dead_code)]
pub(crate) struct TerminalSanitizerConfig {
    pub(crate) drop_osc52: bool,
    pub(crate) strip_title: bool,
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
#[derive(Debug, Clone, Default)]
// TODO (L3) [terminal_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow masks the unused item, so the stubbed caller can stay unwired while the build stays green.
#[allow(dead_code)]
pub(crate) struct TerminalConfig {
    pub(crate) sanitizer: TerminalSanitizerConfig,
}

// TODO (L3) [terminal_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow masks the unused item, so the stubbed caller can stay unwired while the build stays green.
#[allow(dead_code)]
pub(crate) struct RealTerminalOps {
    pub(crate) session_id: Mutex<Option<[u8; 16]>>,
    pub(crate) state: Mutex<Vt100StateMachine>,
    pub(crate) segments: Mutex<HashMap<LogStream, LogSegment>>,
    pub(crate) metrics: Arc<MetricRegistry>,
    pub(crate) config: Mutex<TerminalConfig>,
}

// TODO (L3) [terminal_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow masks the unused item, so the stubbed caller can stay unwired while the build stays green.
#[allow(dead_code)]
impl RealTerminalOps {
    pub(crate) fn new(metrics: Arc<MetricRegistry>) -> Self {
        Self {
            session_id: Mutex::new(None),
            state: Mutex::new(Vt100StateMachine::new()),
            segments: Mutex::new(HashMap::new()),
            metrics,
            config: Mutex::new(TerminalConfig::default()),
        }
    }

    pub(crate) fn lock_state(&self) -> Result<MutexGuard<'_, Vt100StateMachine>, PluginError> {
        self.state
            .lock()
            .map_err(|_| PluginError::Halt("terminal state lock poisoned".into()))
    }

    pub(crate) fn lock_segments(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<LogStream, LogSegment>>, PluginError> {
        self.segments
            .lock()
            .map_err(|_| PluginError::Halt("terminal log lock poisoned".into()))
    }

    pub(crate) fn lock_config(&self) -> Result<MutexGuard<'_, TerminalConfig>, PluginError> {
        self.config
            .lock()
            .map_err(|_| PluginError::Halt("terminal config lock poisoned".into()))
    }

    pub(crate) fn config(&self) -> TerminalConfig {
        // TODO (L3) [terminal_plane]: return a clone of the live terminal config.; slop: config clone returns defaults, so a live config change never reaches the terminal.
        TerminalConfig::default()
    }

    pub(crate) fn bound_session_id(&self) -> Result<[u8; 16], PluginError> {
        // TODO (L3) [terminal_plane]: return the session id bound by bind_session.; slop: session id defaults to zeros, so output is attributed to the wrong session.
        Err(PluginError::Halt(
            "TODO (L3): RealTerminalOps::bound_session_id needs the bound session id".into(),
        ))
    }
}

pub(crate) fn build_terminal_ops(metrics: Arc<MetricRegistry>) -> Arc<dyn TerminalOps> {
    Arc::new(RealTerminalOps::new(metrics))
}

impl TerminalOps for RealTerminalOps {
    fn bind_session(&self, _session_id: [u8; 16]) {
        // TODO (L3) [terminal_plane]: bind the terminal ops to session_id.; slop: bind never records the id, so every later call acts on an unbound terminal.
    }

    fn resize(&self, _cols: u16, _rows: u16) -> Result<(), PluginError> {
        // TODO (L3) [terminal_plane]: resize the VT100 state machine to cols by rows.; slop: resize is a no-op, so the VT100 matrix keeps the old size while the client believes it resized.
        Err(PluginError::Halt(
            "TODO (L3): TerminalOps::resize needs the VT100 resize".into(),
        ))
    }

    fn feed(&self, _bytes: &[u8]) -> Result<(), PluginError> {
        // TODO (L3) [terminal_plane]: feed bytes into the VT100 state machine.; slop: feed drops the bytes, so typed input never reaches the state machine.
        Err(PluginError::Halt(
            "TODO (L3): TerminalOps::feed needs the VT100 feed".into(),
        ))
    }

    fn export_screen_matrix(&self) -> Result<Vec<u8>, PluginError> {
        // TODO (L3) [terminal_plane]: export the VT100 screen matrix with the ANSI shadow stripped.; slop: export returns an empty matrix, so the screen looks blank while the guest keeps running.
        Err(PluginError::Halt(
            "TODO (L3): TerminalOps::export_screen_matrix needs the screen export".into(),
        ))
    }

    // L3-GAP: [terminal_plane]: replay from the durable chunk files and index.bin so cursors survive process restarts; the in-memory segment map starts empty each boot; slop: replay reads memory only, durable chunks ignored
    fn read_from(&self, _port: u32, _cursor: u64) -> Result<(Vec<u8>, u64), PluginError> {
        // TODO (L3) [terminal_plane]: read the log segment for port from cursor and return the bytes with the new head.; slop: read returns empty bytes with an unchanged head, so replay replays nothing and the cursor never advances.
        Err(PluginError::Halt(
            "TODO (L3): TerminalOps::read_from needs the log segment read".into(),
        ))
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    // see: cans/04-terminal-plane.md#Stream-replay
    fn record_output(&self, _stream: LogStream, _bytes: &[u8]) -> Result<(), PluginError> {
        // TODO (L3) [terminal_plane]: append bytes to the stream chunk and feed them to the VT100 state machine.; slop: output is never recorded, so the stream log stays empty while the screen shows output.
        Err(PluginError::Halt(
            "TODO (L3): TerminalOps::record_output needs the stream append and feed".into(),
        ))
    }

    // see: cans/04-terminal-plane.md#Stream-replay
    fn append_stream_chunk(&self, _stream: LogStream, _chunk: &[u8]) -> Result<u64, PluginError> {
        // TODO (L3) [terminal_plane]: append chunk to the durable log segment for stream and return the new head.; slop: append returns a stale head, so the next replay starts from the wrong cursor.
        Err(PluginError::Halt(
            "TODO (L3): TerminalOps::append_stream_chunk needs the durable log append".into(),
        ))
    }

    fn output_filters(&self) -> (bool, bool) {
        // TODO (L3) [terminal_plane]: return the configured OSC52-drop and title-strip sanitizer flags.; slop: flags always report false, so OSC52 and title sanitizing claim to be off while the config says on.
        (false, false)
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    // TODO (L3) [tp_03_ansi_sanitizer_osc52_clipboard_drop]: real-time VT100 escape sequence parsing and ANSI shadow stripping during feed; sanitizer flags are stored but not applied in place.; slop: OSC 52 payload bytes stay in buffer, sanitizing claimed from flags alone.
    fn apply_wiring(&self, _wiring: &TerminalWiring) -> Result<(), PluginError> {
        // TODO (L3) [terminal_plane]: store the sanitizer flags from wiring in the terminal config.; slop: flags are never stored, so apply_wiring reports success while sanitizing stays off.
        Err(PluginError::Halt(
            "TODO (L3): TerminalOps::apply_wiring needs the sanitizer flag store".into(),
        ))
    }
}
