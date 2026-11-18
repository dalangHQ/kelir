// see: cans/plugins/redaction.md#Plugin-redaction
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};

use kelir_interface::plugin_model::{
    FsInterposer, HandlerCtx, KelirPlugin, PipelineBinder, PluginDeps, PluginError,
};
use kelir_interface::protocol_wire::FilterAction;

// see: cans/plugins/redaction.md#Read-masking-(FsInterposer::on_fs_read)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReadMaskEngine {
    #[default]
    FastRegex,
    StreamingAutomata,
}

// see: cans/plugins/redaction.md#Read-masking-(FsInterposer::on_fs_read)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MaskReadRule {
    pub glob: String,
    pub patterns: Vec<(String, String)>,
}

// see: cans/plugins/redaction.md#Read-masking-(FsInterposer::on_fs_read)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MaskReadConfig {
    pub engine: ReadMaskEngine,
    pub carryover_bytes: usize,
    pub rules: Vec<MaskReadRule>,
}

// see: cans/plugins/redaction.md#Plugin-redaction
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RedactionConfig {
    pub mask_read: MaskReadConfig,
}

// see: cans/plugins/redaction.md#Write-snoops-(FsInterposer::on_fs_write)
pub type SnoopOnWrite = Arc<
    dyn Fn(
            &Path,
            &[u8],
            &HandlerCtx,
        ) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send>>
        + Send
        + Sync,
>;

// see: cans/plugins/redaction.md#Write-snoops-(FsInterposer::on_fs_write)
#[derive(Clone)]
pub struct SnoopRule {
    pub glob: String,
    pub sync: bool,
    pub on_write: SnoopOnWrite,
}

impl SnoopRule {
    pub fn new(glob: impl Into<String>, sync: bool, on_write: SnoopOnWrite) -> Self {
        Self {
            glob: glob.into(),
            sync,
            on_write,
        }
    }

    pub fn glob(&self) -> &str {
        &self.glob
    }
}

impl std::fmt::Debug for SnoopRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnoopRule")
            .field("glob", &self.glob)
            .field("sync", &self.sync)
            .field("on_write", &"callback")
            .finish()
    }
}

// see: cans/plugins/redaction.md#Service-exports-(redaction)
pub trait RedactionService: Send + Sync {
    fn mask_read(&self, path: &Path, buffer: &[u8]) -> Result<Vec<u8>, PluginError>;
    fn check_write(&self, path: &Path, data: &mut [u8]) -> Result<FilterAction, PluginError>;
}

// see: cans/plugins/redaction.md#Invariants-and-policy-(redaction)
pub struct Redactor {
    config: MaskReadConfig,
    snoops: Vec<SnoopRule>,
    carryover: Mutex<Vec<u8>>,
}

impl Redactor {
    pub fn new(config: MaskReadConfig, snoops: Vec<SnoopRule>) -> Self {
        Self {
            config,
            snoops,
            carryover: Mutex::new(Vec::new()),
        }
    }

    pub fn config(&self) -> &MaskReadConfig {
        &self.config
    }

    pub fn snoops(&self) -> &[SnoopRule] {
        &self.snoops
    }

    fn lock_carryover(&self) -> Result<MutexGuard<'_, Vec<u8>>, PluginError> {
        self.carryover
            .lock()
            .map_err(|_| PluginError::Halt("redaction carryover lock poisoned".into()))
    }
}

impl RedactionService for Redactor {
    // see: cans/plugins/redaction.md#Read-masking-(FsInterposer::on_fs_read)
    fn mask_read(&self, path: &Path, buffer: &[u8]) -> Result<Vec<u8>, PluginError> {
        let _ = path;
        // TODO (L3) [sp_14_in_flight_read_masking_pattern_replace]: apply the glob-matched ordered pattern list over the carryover-joined window and return the masked buffer; slop: guest read returns the raw secret because the mask runs after the VFS return.
        let mut carryover = self.lock_carryover()?;
        carryover.extend_from_slice(buffer);
        let keep = self.config.carryover_bytes.min(carryover.len());
        let drop = carryover.len() - keep;
        carryover.drain(..drop);
        Ok(buffer.to_vec())
    }

    // see: cans/plugins/redaction.md#Write-snoops-(FsInterposer::on_fs_write)
    fn check_write(&self, path: &Path, data: &mut [u8]) -> Result<FilterAction, PluginError> {
        // TODO (L3) [storage_plane]: match the path against the glob-keyed snoop set and emit the matching rule verdict or mutation through the unified lattice; slop: write snoops never fire; exfiltrating writes pass without a verdict.
        let _ = (path, data);
        Ok(FilterAction::Forward)
    }
}

// see: cans/plugins/redaction.md#Service-exports-(redaction)
#[derive(Clone)]
pub struct RedactionCtx {
    service: Arc<Redactor>,
}

impl RedactionCtx {
    pub fn mask_read(&self, path: &Path, buffer: &[u8]) -> Result<Vec<u8>, PluginError> {
        self.service.mask_read(path, buffer)
    }

    pub fn check_write(&self, path: &Path, data: &mut [u8]) -> Result<FilterAction, PluginError> {
        self.service.check_write(path, data)
    }
}

// see: cans/plugins/redaction.md#Plugin-redaction
#[derive(Clone, PluginDeps)]
pub struct RedactionPlugin {
    service: Arc<Redactor>,
}

impl RedactionPlugin {
    pub fn new(config: MaskReadConfig, snoops: Vec<SnoopRule>) -> Self {
        Self {
            service: Arc::new(Redactor::new(config, snoops)),
        }
    }

    pub fn with_config(config: MaskReadConfig) -> Self {
        Self::new(config, Vec::new())
    }

    pub fn service(&self) -> &Arc<Redactor> {
        &self.service
    }
}

impl Default for RedactionPlugin {
    fn default() -> Self {
        Self::new(MaskReadConfig::default(), Vec::new())
    }
}

impl KelirPlugin for RedactionPlugin {
    type Context = RedactionCtx;
    fn context(&self) -> Self::Context {
        RedactionCtx {
            service: Arc::clone(&self.service),
        }
    }
    fn name(&self) -> &'static str {
        "redaction"
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        // see: cans/plugins/redaction.md#Service-exports-(redaction)
        let fs_stage: Arc<dyn FsInterposer> = Arc::new(self.clone());
        pipeline.bind_fs_scoped(fs_stage, self.default_scope());
    }
}

impl FsInterposer for RedactionPlugin {
    fn on_fs_read(&self, path: &Path, buffer: &[u8]) -> Result<Vec<u8>, PluginError> {
        self.service.mask_read(path, buffer)
    }

    fn on_fs_write(
        &self,
        path: &Path,
        data: &mut [u8],
        pid: u32,
        ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        let _ = (pid, ctx);
        self.service.check_write(path, data)
    }
}
