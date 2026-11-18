// see: cans/plugins/redaction.md#Plugin-redaction
#![forbid(unsafe_code)]

#[path = "service_redaction.flow.rs"]
pub mod service_redaction;

pub use service_redaction::{
    MaskReadConfig, MaskReadRule, ReadMaskEngine, RedactionConfig, RedactionCtx, RedactionPlugin,
    RedactionService, Redactor, SnoopOnWrite, SnoopRule,
};
