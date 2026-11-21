// see: cans/plugins/redaction.md#Plugin-redaction
#![forbid(unsafe_code)]

#[path = "service_redaction.flow.rs"]
pub mod service_redaction;
#[path = "service_redaction.run.rs"]
pub mod service_redaction_run;

pub use service_redaction::{
    MaskReadConfig, MaskReadRule, QuarantineManifest, ReadMaskEngine, RedactionConfig,
    RedactionCtx, RedactionPlugin, RedactionService, Redactor, SnoopOnWrite, SnoopRule,
    SoftDeleteConfig,
};
