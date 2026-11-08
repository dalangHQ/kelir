// see: cans/04-terminal-plane.md#Terminal-pipeline
// see: cans/04-terminal-plane.md#Stream-replay
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

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

    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): parse VT100 escape sequences in bytes and update matrix + cursor.
        let _ = bytes;
        Err(kelir_interface::plugin::PluginError::Halt(
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

    pub fn advance(&self, len: u64) -> Result<Self, kelir_interface::plugin::PluginError> {
        self.0
            .checked_add(len)
            .map(Self)
            .ok_or_else(|| kelir_interface::plugin::PluginError::Halt("log cursor overflow".into()))
    }
}

pub struct LogSegment {
    pub port: u32,
    pub head: LogCursor,
    pub bytes: Vec<u8>,
    pub index_path: PathBuf,
}

impl LogSegment {
    pub fn new(
        session_id: [u8; 16],
        port: u32,
    ) -> Result<Self, kelir_interface::plugin::PluginError> {
        Ok(Self {
            port,
            head: LogCursor::start(),
            bytes: Vec::new(),
            index_path: session_log_path(&session_id)?,
        })
    }
}

// see: cans/01-architecture.md#Host-state-root
fn session_log_path(
    session_id: &[u8; 16],
) -> Result<PathBuf, kelir_interface::plugin::PluginError> {
    let home = std::env::var("HOME").map_err(|_| {
        kelir_interface::plugin::PluginError::Halt(
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
        .join("index.bin"))
}

pub struct TerminalPlane {
    session_id: [u8; 16],
    state: Mutex<Vt100StateMachine>,
    segments: Mutex<HashMap<u32, LogSegment>>,
}

impl TerminalPlane {
    pub fn new(session_id: [u8; 16]) -> Self {
        // see: cans/04-terminal-plane.md#Terminal-pipeline
        Self {
            session_id,
            state: Mutex::new(Vt100StateMachine::new()),
            segments: Mutex::new(HashMap::new()),
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), kelir_interface::plugin::PluginError> {
        let mut state = self.state.lock().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("terminal state lock poisoned".into())
        })?;
        state.resize(cols, rows);
        Ok(())
    }

    pub fn feed(&self, bytes: &[u8]) -> Result<(), kelir_interface::plugin::PluginError> {
        let mut state = self.state.lock().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("terminal state lock poisoned".into())
        })?;
        state.feed(bytes)
    }

    pub fn export_screen_matrix(&self) -> Result<Vec<u8>, kelir_interface::plugin::PluginError> {
        let state = self.state.lock().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("terminal state lock poisoned".into())
        })?;
        Ok(apply_token_shield(
            &filter_ansi_shadow(&state.export()),
            usize::MAX,
            usize::MAX,
        ))
    }

    pub fn read_from(
        &self,
        port: u32,
        cursor: u64,
    ) -> Result<(Vec<u8>, u64), kelir_interface::plugin::PluginError> {
        let segments = self.segments.lock().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("terminal log lock poisoned".into())
        })?;
        let segment = segments.get(&port).ok_or_else(|| {
            kelir_interface::plugin::PluginError::Halt("no log segment for port".into())
        })?;
        if cursor > segment.head.0 {
            return Err(kelir_interface::plugin::PluginError::Halt(
                "cursor beyond log head".into(),
            ));
        }
        let offset = usize::try_from(cursor).map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("log cursor too large".into())
        })?;
        Ok((segment.bytes[offset..].to_vec(), segment.head.0))
    }

    pub fn append_log_chunk(
        &self,
        port: u32,
        chunk: &[u8],
    ) -> Result<u64, kelir_interface::plugin::PluginError> {
        let mut segments = self.segments.lock().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("terminal log lock poisoned".into())
        })?;
        if let std::collections::hash_map::Entry::Vacant(e) = segments.entry(port) {
            e.insert(LogSegment::new(self.session_id, port)?);
        }
        let segment = segments.get_mut(&port).ok_or_else(|| {
            kelir_interface::plugin::PluginError::Halt("log segment missing after insert".into())
        })?;
        let len = u64::try_from(chunk.len()).map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("log chunk too large".into())
        })?;
        let new_head = segment.head.advance(len)?;
        if let Some(parent) = segment.index_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                kelir_interface::plugin::PluginError::Halt(format!("create session log dir: {e}"))
            })?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&segment.index_path)
            .map_err(|e| {
                kelir_interface::plugin::PluginError::Halt(format!("open log index: {e}"))
            })?;
        file.write_all(chunk).map_err(|e| {
            kelir_interface::plugin::PluginError::Halt(format!("append log index: {e}"))
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
