// see: cans/04-terminal-plane.md#Terminal-pipeline
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex, MutexGuard};

use kelir_interface::plugin_model::PluginError;
use kelir_interface::terminal::{
    LogSegment, LogStream, PromptRuleConfig, TerminalOps, TerminalWiring, TokenShieldConfig,
};
use kelir_otlp::registry_otlp::MetricRegistry;

use crate::vt100::Vt100StateMachine;

#[derive(Debug, Clone, Default)]
pub(crate) struct TerminalConfig {
    pub(crate) filters: (bool, bool),
    pub(crate) token_shield: Option<TokenShieldConfig>,
    pub(crate) prompts: Vec<PromptRuleConfig>,
}

pub(crate) struct RealTerminalOps {
    pub(crate) session_id: Mutex<Option<[u8; 16]>>,
    pub(crate) state: Mutex<Vt100StateMachine>,
    pub(crate) segments: Mutex<HashMap<LogStream, LogSegment>>,
    pub(crate) metrics: Arc<MetricRegistry>,
    pub(crate) config: Mutex<TerminalConfig>,
}

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
        self.config
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    pub(crate) fn bound_session_id(&self) -> Result<[u8; 16], PluginError> {
        self.session_id
            .lock()
            .map_err(|_| PluginError::Halt("terminal session lock poisoned".into()))?
            .ok_or_else(|| PluginError::Halt("terminal ops not bound to a session".into()))
    }
}

pub(crate) fn build_terminal_ops(metrics: Arc<MetricRegistry>) -> Arc<dyn TerminalOps> {
    Arc::new(RealTerminalOps::new(metrics))
}

impl TerminalOps for RealTerminalOps {
    fn bind_session(&self, session_id: [u8; 16]) {
        if let Ok(mut bound) = self.session_id.lock() {
            *bound = Some(session_id);
        }
    }

    fn resize(&self, cols: u16, rows: u16) -> Result<(), PluginError> {
        let mut state = self.lock_state()?;
        state.resize(cols, rows);
        Ok(())
    }

    fn feed(&self, bytes: &[u8]) -> Result<(), PluginError> {
        let mut state = self.lock_state()?;
        state.feed(bytes).inspect_err(|_| {
            self.metrics.record_drop();
        })
    }

    fn export_screen_matrix(&self) -> Result<Vec<u8>, PluginError> {
        let state = self.lock_state()?;
        let (max_lines, max_bytes) = match self.config().token_shield.as_ref() {
            Some(shield) => (shield.max_lines_total, shield.max_bytes_total),
            None => (usize::MAX, usize::MAX),
        };
        Ok(crate::token_shield::apply_token_shield(
            &crate::vt100::filter_ansi_shadow(&state.export()),
            max_lines,
            max_bytes,
        ))
    }

    // TODO (L3) [terminal_plane]: replay from the durable chunk files and index.bin so cursors survive process restarts; the in-memory segment map starts empty each boot; slop: replay reads memory only, durable chunks ignored
    fn read_from(&self, port: u32, cursor: u64) -> Result<(Vec<u8>, u64), PluginError> {
        let segments = self.lock_segments()?;
        let segment = segments
            .get(&LogStream::Port(port))
            .ok_or_else(|| PluginError::Halt("no log segment for port".into()))?;
        if cursor > segment.head.0 {
            return Err(PluginError::Halt("cursor beyond log head".into()));
        }
        let offset = usize::try_from(cursor)
            .map_err(|_| PluginError::Halt("log cursor too large".into()))?;
        Ok((segment.bytes[offset..].to_vec(), segment.head.0))
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    // see: cans/04-terminal-plane.md#Stream-replay
    fn record_output(&self, stream: LogStream, bytes: &[u8]) -> Result<(), PluginError> {
        self.append_stream_chunk(stream, bytes)?;
        self.feed(bytes)
    }

    // see: cans/04-terminal-plane.md#Stream-replay
    fn append_stream_chunk(&self, stream: LogStream, chunk: &[u8]) -> Result<u64, PluginError> {
        let session_id = self.bound_session_id()?;
        let mut segments = self.lock_segments()?;
        if let std::collections::hash_map::Entry::Vacant(entry) = segments.entry(stream) {
            entry.insert(LogSegment::new(
                stream,
                crate::log_segment::session_log_path(&session_id, stream)?,
            ));
        }
        let segment = segments
            .get_mut(&stream)
            .ok_or_else(|| PluginError::Halt("log segment missing after insert".into()))?;
        let len = u64::try_from(chunk.len())
            .map_err(|_| PluginError::Halt("log chunk too large".into()))?;
        let new_head = segment.head.advance(len)?;
        if let Some(parent) = segment.log_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| PluginError::Halt(format!("create session log dir: {e}")))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&segment.log_path)
            .map_err(|e| PluginError::Halt(format!("open stream log: {e}")))?;
        file.write_all(chunk)
            .map_err(|e| PluginError::Halt(format!("append stream log: {e}")))?;
        segment.head = new_head;
        segment.bytes.extend_from_slice(chunk);
        Ok(segment.head.0)
    }

    fn output_filters(&self) -> (bool, bool) {
        self.config().filters
    }

    fn token_shield(&self) -> Option<TokenShieldConfig> {
        self.config().token_shield
    }

    fn prompts(&self) -> Vec<PromptRuleConfig> {
        self.config().prompts
    }

    fn apply_wiring(&self, wiring: &TerminalWiring) -> Result<(), PluginError> {
        let mut config = self.lock_config()?;
        *config = TerminalConfig {
            filters: (wiring.drop_osc52, wiring.strip_title),
            token_shield: wiring.token_shield.clone(),
            prompts: wiring.prompts.clone(),
        };
        Ok(())
    }
}
