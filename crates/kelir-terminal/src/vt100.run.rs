// see: cans/04-terminal-plane.md#Terminal-pipeline
// see: cans/04-terminal-plane.md#Stream-replay
use kelir_interface::plugin_model::PluginError;

#[derive(Debug, Clone, Default)]
pub(crate) struct Cell {
    pub(crate) glyph: char,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Row {
    pub(crate) cells: Vec<Cell>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ScreenMatrix {
    pub(crate) rows: Vec<Row>,
}

impl ScreenMatrix {
    pub(crate) fn new(cols: u16, rows: u16) -> Self {
        Self {
            rows: (0..rows)
                .map(|_| Row {
                    cells: (0..cols).map(|_| Cell { glyph: ' ' }).collect(),
                })
                .collect(),
        }
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
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
pub(crate) struct Vt100StateMachine {
    pub(crate) matrix: ScreenMatrix,
    pub(crate) cursor_row: u16,
    pub(crate) cursor_col: u16,
}

impl Vt100StateMachine {
    pub(crate) fn new() -> Self {
        Self {
            matrix: ScreenMatrix::new(0, 0),
            cursor_row: 0,
            cursor_col: 0,
        }
    }

    pub(crate) fn resize(&mut self, cols: u16, rows: u16) {
        self.matrix = ScreenMatrix::new(cols, rows);
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    pub(crate) fn feed(&mut self, _bytes: &[u8]) -> Result<(), PluginError> {
        // TODO (L3) [terminal_plane]: parse VT100 escape sequences in bytes and update matrix + cursor; slop: escape sequences unparsed, matrix and cursor wrong
        Ok(())
    }

    pub(crate) fn export(&self) -> Vec<u8> {
        self.matrix.to_bytes()
    }
}

pub(crate) fn filter_ansi_shadow(buf: &[u8]) -> Vec<u8> {
    // TODO (L3) [tp_03_ansi_sanitizer_osc52_clipboard_drop]: strip ANSI shadow sequences from buf; slop: OSC52 payload reaches host terminal, bytes not purged
    buf.to_vec()
}
