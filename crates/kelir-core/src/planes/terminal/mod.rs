// see: cans/04-terminal-plane.md#Terminal-pipeline
// see: cans/04-terminal-plane.md#Stream-replay
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};

pub struct TerminalPlane {
    current_cols: AtomicU16,
    current_rows: AtomicU16,
    log_cursor: AtomicU64,
}

impl Default for TerminalPlane {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalPlane {
    pub fn new() -> Self {
        // NOTE: 0x0 default grid; no size is specified in cans/04-terminal-plane.md.
        Self {
            current_cols: AtomicU16::new(0),
            current_rows: AtomicU16::new(0),
            log_cursor: AtomicU64::new(0),
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), kelir_interface::errors::PluginError> {
        self.current_cols.store(cols, Ordering::SeqCst);
        self.current_rows.store(rows, Ordering::SeqCst);
        Ok(())
    }

    pub fn export_screen_matrix(&self) -> Result<Vec<u8>, kelir_interface::errors::PluginError> {
        // TODO: render the PTY grid to bytes; pipeline shape only until the grid store lands.
        Ok(apply_token_shield(
            &filter_ansi_shadow(&[]),
            usize::MAX,
            usize::MAX,
        ))
    }

    pub fn read_from(
        &self,
        port: u32,
        cursor: u64,
    ) -> Result<(Vec<u8>, u64), kelir_interface::errors::PluginError> {
        // TODO: replay log bytes for port from cursor.
        let _ = port;
        Ok((Vec::new(), cursor))
    }

    pub fn append_log_chunk(
        &self,
        port: u32,
        chunk: &[u8],
    ) -> Result<u64, kelir_interface::errors::PluginError> {
        // TODO: persist chunk to the port log; cursor advance only until the log store lands.
        let _ = port;
        let len = u64::try_from(chunk.len()).map_err(|_| {
            kelir_interface::errors::PluginError::Halt("log chunk too large".into())
        })?;
        let current = self.log_cursor.load(Ordering::SeqCst);
        let next = current.checked_add(len).ok_or_else(|| {
            kelir_interface::errors::PluginError::Halt("log cursor overflow".into())
        })?;
        self.log_cursor.store(next, Ordering::SeqCst);
        Ok(next)
    }
}

fn filter_ansi_shadow(buf: &[u8]) -> Vec<u8> {
    // TODO: strip ANSI shadow sequences from buf.
    let _ = buf;
    Vec::new()
}

fn apply_token_shield(grid: &[u8], max_lines: usize, max_bytes: usize) -> Vec<u8> {
    // TODO: truncate grid to max_lines/max_bytes with saturating math and redact token spans.
    let _ = (grid, max_lines, max_bytes);
    Vec::new()
}
