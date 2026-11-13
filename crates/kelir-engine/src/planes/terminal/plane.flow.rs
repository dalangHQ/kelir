// see: cans/01-architecture.md#Native-Engine-Planes-on-Session
use super::*;
impl TerminalPlane {
    pub fn new(session_id: [u8; 16], ops: Arc<dyn TerminalOps>) -> Self {
        ops.bind_session(session_id);
        Self {
            session_id,
            ops,
            metrics: Arc::new(MetricRegistry::new()),
        }
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    // TODO (L3) [tp_03_ansi_sanitizer_osc52_clipboard_drop]: the host filter pipeline applies drop_osc52/strip_title to console output before VT100 feed; slop: OSC52 payload reaches host terminal, bytes not purged
    pub fn with_wiring(self, wiring: &TerminalWiring) -> Self {
        let _ = self.ops.apply_wiring(wiring);
        let _ = self.resize(wiring.cols, wiring.rows);
        self
    }

    pub fn with_metrics(mut self, metrics: Arc<MetricRegistry>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> &Arc<MetricRegistry> {
        &self.metrics
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    pub fn output_filters(&self) -> (bool, bool) {
        self.ops.output_filters()
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    pub fn token_shield(&self) -> Option<TokenShieldConfig> {
        self.ops.token_shield()
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    pub fn prompts(&self) -> Vec<PromptRuleConfig> {
        self.ops.prompts()
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), PluginError> {
        self.ops.resize(cols, rows)
    }

    pub fn feed(&self, bytes: &[u8]) -> Result<(), PluginError> {
        self.ops.feed(bytes)
    }

    pub fn export_screen_matrix(&self) -> Result<Vec<u8>, PluginError> {
        self.ops.export_screen_matrix()
    }

    pub fn read_from(&self, port: u32, cursor: u64) -> Result<(Vec<u8>, u64), PluginError> {
        self.ops.read_from(port, cursor)
    }

    pub fn append_log_chunk(&self, port: u32, chunk: &[u8]) -> Result<u64, PluginError> {
        self.ops.append_stream_chunk(LogStream::Port(port), chunk)
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    // see: cans/04-terminal-plane.md#Stream-replay
    pub fn record_output(&self, stream: LogStream, bytes: &[u8]) -> Result<(), PluginError> {
        self.ops.record_output(stream, bytes)
    }

    // see: cans/04-terminal-plane.md#Stream-replay
    pub fn append_stream_chunk(&self, stream: LogStream, chunk: &[u8]) -> Result<u64, PluginError> {
        self.ops.append_stream_chunk(stream, chunk)
    }
}
