// see: cans/04-terminal-plane.md#Terminal-pipeline
// see: cans/04-terminal-plane.md#Stream-replay
use std::collections::HashMap;
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

    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO (L3): parse VT100 escape sequences in bytes and update matrix + cursor.
        let _ = bytes;
        Err(kelir_interface::errors::PluginError::Halt(
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

    pub fn advance(&self, len: u64) -> Result<Self, kelir_interface::errors::PluginError> {
        self.0
            .checked_add(len)
            .map(Self)
            .ok_or_else(|| kelir_interface::errors::PluginError::Halt("log cursor overflow".into()))
    }
}

pub struct LogSegment {
    pub port: u32,
    pub head: LogCursor,
    pub bytes: Vec<u8>,
    pub index_path: PathBuf,
}

impl LogSegment {
    pub fn new(port: u32) -> Self {
        Self {
            port,
            head: LogCursor::start(),
            bytes: Vec::new(),
            index_path: PathBuf::from(format!("sessions/log-{port}/index.bin")),
        }
    }
}

pub struct TerminalPlane {
    state: Mutex<Vt100StateMachine>,
    segments: Mutex<HashMap<u32, LogSegment>>,
}

impl Default for TerminalPlane {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalPlane {
    pub fn new() -> Self {
        // see: cans/04-terminal-plane.md#Terminal-pipeline
        Self {
            state: Mutex::new(Vt100StateMachine::new()),
            segments: Mutex::new(HashMap::new()),
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), kelir_interface::errors::PluginError> {
        let mut state = self.state.lock().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("terminal state lock poisoned".into())
        })?;
        state.resize(cols, rows);
        Ok(())
    }

    pub fn feed(&self, bytes: &[u8]) -> Result<(), kelir_interface::errors::PluginError> {
        let mut state = self.state.lock().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("terminal state lock poisoned".into())
        })?;
        state.feed(bytes)
    }

    pub fn export_screen_matrix(&self) -> Result<Vec<u8>, kelir_interface::errors::PluginError> {
        let state = self.state.lock().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("terminal state lock poisoned".into())
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
    ) -> Result<(Vec<u8>, u64), kelir_interface::errors::PluginError> {
        let segments = self.segments.lock().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("terminal log lock poisoned".into())
        })?;
        let segment = segments.get(&port).ok_or_else(|| {
            kelir_interface::errors::PluginError::Halt("no log segment for port".into())
        })?;
        if cursor > segment.head.0 {
            return Err(kelir_interface::errors::PluginError::Halt(
                "cursor beyond log head".into(),
            ));
        }
        let offset = usize::try_from(cursor).map_err(|_| {
            kelir_interface::errors::PluginError::Halt("log cursor too large".into())
        })?;
        Ok((segment.bytes[offset..].to_vec(), segment.head.0))
    }

    pub fn append_log_chunk(
        &self,
        port: u32,
        chunk: &[u8],
    ) -> Result<u64, kelir_interface::errors::PluginError> {
        // TODO (L3): flush the segment to its append-only index.bin on durable storage.
        let mut segments = self.segments.lock().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("terminal log lock poisoned".into())
        })?;
        let segment = segments
            .entry(port)
            .or_insert_with(|| LogSegment::new(port));
        let len = u64::try_from(chunk.len()).map_err(|_| {
            kelir_interface::errors::PluginError::Halt("log chunk too large".into())
        })?;
        segment.head = segment.head.advance(len)?;
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
