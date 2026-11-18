use serde::{Deserialize, Serialize};

// see: cans/04-terminal-plane.md#Headless-VT100-wireframe-grid
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct TerminalWireframeGridConfig {
    pub track_screen_matrix: bool,
    pub export_on_trap: bool,
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct TerminalSanitizerConfig {
    pub drop_osc52: bool,
    pub strip_title: bool,
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
// see: cans/artifacts/examples/fixtures/kelir.yaml#terminal
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PtyConfig {
    pub cols: u16,
    pub rows: u16,
    pub wireframe_grid: TerminalWireframeGridConfig,
    // see: cans/artifacts/examples/fixtures/kelir.yaml#terminal.stream_engine
    pub stream_engine: String,
    pub sanitizer: TerminalSanitizerConfig,
}
