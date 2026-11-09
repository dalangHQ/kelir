// see: cans/04-terminal-plane.md#Terminal-pipeline
// see: cans/04-terminal-plane.md#Stream-replay
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct Cell {
    pub glyph: char,
}

#[derive(Debug, Clone, Default)]
pub struct Row {
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, Default)]
pub struct ScreenMatrix {
    pub rows: Vec<Row>,
    pub cols: u16,
}

impl ScreenMatrix {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            rows: (0..rows)
                .map(|_| Row {
                    cells: (0..cols).map(|_| Cell { glyph: ' ' }).collect(),
                })
                .collect(),
            cols,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for row in &self.rows {
            for cell in &row.cells {
                let mut buf = [0u8; 4];
                out.extend_from_slice(cell.glyph.encode_utf8(&mut buf).as_bytes());
            }
            out.push(b'\n');
        }
        out
    }
}

#[derive(Debug, Clone, Default)]
pub struct Vt100StateMachine {
    pub matrix: ScreenMatrix,
    pub cursor_row: u16,
    pub cursor_col: u16,
}

impl Vt100StateMachine {
    pub fn new() -> Self {
        Self {
            matrix: ScreenMatrix::new(0, 0),
            cursor_row: 0,
            cursor_col: 0,
        }
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.matrix = ScreenMatrix::new(cols, rows);
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): parse VT100 escape sequences in bytes and update matrix + cursor.
        let _ = bytes;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Vt100StateMachine::feed".into(),
        ))
    }

    pub fn export(&self) -> Vec<u8> {
        self.matrix.to_bytes()
    }
}

pub struct LogCursor(pub u64);

impl LogCursor {
    pub fn start() -> Self {
        Self(0)
    }

    pub fn advance(&self, len: u64) -> Result<Self, kelir_interface::plugin_model::PluginError> {
        self.0.checked_add(len).map(Self).ok_or_else(|| {
            kelir_interface::plugin_model::PluginError::Halt("log cursor overflow".into())
        })
    }
}

// see: cans/04-terminal-plane.md#Stream-replay
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogStream {
    Port(u32),
    Exec { id: u64, lane: u8 },
}

pub struct LogSegment {
    pub stream: LogStream,
    pub head: LogCursor,
    pub bytes: Vec<u8>,
    pub log_path: PathBuf,
}

impl LogSegment {
    pub fn new(
        session_id: [u8; 16],
        stream: LogStream,
    ) -> Result<Self, kelir_interface::plugin_model::PluginError> {
        Ok(Self {
            stream,
            head: LogCursor::start(),
            bytes: Vec::new(),
            log_path: session_log_path(&session_id, stream)?,
        })
    }
}

// see: cans/01-architecture.md#Host-state-root
fn log_file_name(stream: LogStream) -> String {
    match stream {
        LogStream::Port(port) => format!("port-{port}.log"),
        LogStream::Exec { id, lane } => format!("exec-{id}-{lane}.log"),
    }
}

// see: cans/01-architecture.md#Host-state-root
// TODO (L3): write the append-only index.bin chunk index beside the per-stream logs so cursors survive restarts.
fn session_log_path(
    session_id: &[u8; 16],
    stream: LogStream,
) -> Result<PathBuf, kelir_interface::plugin_model::PluginError> {
    let home = std::env::var("HOME").map_err(|_| {
        kelir_interface::plugin_model::PluginError::Halt(
            "HOME is not set; cannot locate ~/.kelir session logs".into(),
        )
    })?;
    let mut hex = String::with_capacity(32);
    for byte in session_id {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok(PathBuf::from(home)
        .join(".kelir")
        .join("sessions")
        .join(hex)
        .join("logs")
        .join(log_file_name(stream)))
}

pub struct TerminalPlane {
    session_id: [u8; 16],
    state: Mutex<Vt100StateMachine>,
    segments: Mutex<HashMap<LogStream, LogSegment>>,
    metrics: Arc<kelir_otlp::registry_otlp::MetricRegistry>,
    filters: (bool, bool),
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalWiring {
    pub cols: u16,
    pub rows: u16,
    pub drop_osc52: bool,
    pub strip_title: bool,
}

impl TerminalPlane {
    pub fn new(session_id: [u8; 16]) -> Self {
        // see: cans/04-terminal-plane.md#Terminal-pipeline
        Self {
            session_id,
            state: Mutex::new(Vt100StateMachine::new()),
            segments: Mutex::new(HashMap::new()),
            metrics: Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new()),
            filters: (false, false),
        }
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    // TODO (L3): the host filter pipeline applies drop_osc52/strip_title to console output before VT100 feed.
    pub fn with_wiring(self, wiring: &TerminalWiring) -> Self {
        let _ = self.resize(wiring.cols, wiring.rows);
        Self {
            filters: (wiring.drop_osc52, wiring.strip_title),
            ..self
        }
    }

    pub fn with_metrics(mut self, metrics: Arc<kelir_otlp::registry_otlp::MetricRegistry>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> &Arc<kelir_otlp::registry_otlp::MetricRegistry> {
        &self.metrics
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    pub fn output_filters(&self) -> (bool, bool) {
        self.filters
    }

    pub fn resize(
        &self,
        cols: u16,
        rows: u16,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let mut state = self.state.lock().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("terminal state lock poisoned".into())
        })?;
        state.resize(cols, rows);
        Ok(())
    }

    pub fn feed(&self, bytes: &[u8]) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let mut state = self.state.lock().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("terminal state lock poisoned".into())
        })?;
        state.feed(bytes).inspect_err(|_| {
            self.metrics.record_drop();
        })
    }

    pub fn export_screen_matrix(
        &self,
    ) -> Result<Vec<u8>, kelir_interface::plugin_model::PluginError> {
        let state = self.state.lock().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("terminal state lock poisoned".into())
        })?;
        Ok(apply_token_shield(
            &filter_ansi_shadow(&state.export()),
            usize::MAX,
            usize::MAX,
        ))
    }

    // TODO (L3): replay from the durable chunk files and index.bin so cursors survive process restarts; the in-memory segment map starts empty each boot.
    pub fn read_from(
        &self,
        port: u32,
        cursor: u64,
    ) -> Result<(Vec<u8>, u64), kelir_interface::plugin_model::PluginError> {
        let segments = self.segments.lock().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("terminal log lock poisoned".into())
        })?;
        let segment = segments.get(&LogStream::Port(port)).ok_or_else(|| {
            kelir_interface::plugin_model::PluginError::Halt("no log segment for port".into())
        })?;
        if cursor > segment.head.0 {
            return Err(kelir_interface::plugin_model::PluginError::Halt(
                "cursor beyond log head".into(),
            ));
        }
        let offset = usize::try_from(cursor).map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("log cursor too large".into())
        })?;
        Ok((segment.bytes[offset..].to_vec(), segment.head.0))
    }

    pub fn append_log_chunk(
        &self,
        port: u32,
        chunk: &[u8],
    ) -> Result<u64, kelir_interface::plugin_model::PluginError> {
        self.append_stream_chunk(LogStream::Port(port), chunk)
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    // see: cans/04-terminal-plane.md#Stream-replay
    pub fn record_output(
        &self,
        stream: LogStream,
        bytes: &[u8],
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        self.append_stream_chunk(stream, bytes)?;
        self.feed(bytes)
    }

    // see: cans/04-terminal-plane.md#Stream-replay
    pub fn append_stream_chunk(
        &self,
        stream: LogStream,
        chunk: &[u8],
    ) -> Result<u64, kelir_interface::plugin_model::PluginError> {
        let mut segments = self.segments.lock().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("terminal log lock poisoned".into())
        })?;
        if let std::collections::hash_map::Entry::Vacant(entry) = segments.entry(stream) {
            entry.insert(LogSegment::new(self.session_id, stream)?);
        }
        let segment = segments.get_mut(&stream).ok_or_else(|| {
            kelir_interface::plugin_model::PluginError::Halt(
                "log segment missing after insert".into(),
            )
        })?;
        let len = u64::try_from(chunk.len()).map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("log chunk too large".into())
        })?;
        let new_head = segment.head.advance(len)?;
        if let Some(parent) = segment.log_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                kelir_interface::plugin_model::PluginError::Halt(format!(
                    "create session log dir: {e}"
                ))
            })?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&segment.log_path)
            .map_err(|e| {
                kelir_interface::plugin_model::PluginError::Halt(format!("open stream log: {e}"))
            })?;
        file.write_all(chunk).map_err(|e| {
            kelir_interface::plugin_model::PluginError::Halt(format!("append stream log: {e}"))
        })?;
        segment.head = new_head;
        segment.bytes.extend_from_slice(chunk);
        Ok(segment.head.0)
    }
}

fn filter_ansi_shadow(buf: &[u8]) -> Vec<u8> {
    // TODO (L3): strip ANSI shadow sequences from buf.
    buf.to_vec()
}

fn apply_token_shield(grid: &[u8], max_lines: usize, max_bytes: usize) -> Vec<u8> {
    // TODO (L3): truncate grid to max_lines/max_bytes with saturating math and redact token spans.
    let _ = (max_lines, max_bytes);
    grid.to_vec()
}
