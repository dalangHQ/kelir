// see: cans/04-terminal-plane.md#Terminal-pipeline
#[derive(Debug, Clone)]
pub struct PtyConfig {
    pub cols: u16,
    pub rows: u16,
    pub drop_osc52: bool,
    pub strip_title: bool,
    pub stream_engine: String,
}
