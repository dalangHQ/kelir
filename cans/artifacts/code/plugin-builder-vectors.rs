// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the shape of one plugin_builder parity vector case; the live shape
// is crates/kelir-plugin-testkit/src/vectors_testkit.rs (PluginBuilderCase)
// and the normative contract is cans/02-plugin-model.md#Fluent-plugin-creation-builder.
//
// Owned by: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/15-operations.md#Multi-language-parity-gate

use kelir_interface::fusion_plugin::TrampolineRing;
use kelir_interface::protocol_wire::{FilterAction, GuestToHost, TrapEvent, Verdict};

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/15-operations.md#Multi-language-parity-gate
pub struct PluginBuilderCase {
    pub name: String,
    pub requires: Vec<String>,
    pub rings: Vec<(TrampolineRing, u32)>,
    pub events: Vec<serde_json::Value>,
    pub expected_outcomes: Vec<PluginBuilderOutcome>,
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/15-operations.md#Multi-language-parity-gate
pub enum PluginBuilderOutcome {
    Verdict(Verdict),
    Filter(FilterAction),
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// One case, as it lands in vectors.json under the vector's "plugin_builder"
// key. The ring ordinal is the hook's position in the .on_* call order of
// the builder chain (chain order, not ring declaration order): the fs_write
// hook is bound first (ordinal 0), the trap hook second (ordinal 1).
// The vector's top-level frame/event/expected_verdict carry the first
// event (the trap) so kelir-plugin-cli parity run frame-validates the case
// and reports its verdict evaluation not-executable (pending engine).
pub fn plugin_builder_fs_write_trap_case() -> PluginBuilderCase {
    let trap = TrapEvent::new(7, 1300, 100, "/bin/sh");
    PluginBuilderCase {
        name: "plugin-builder-fs-write-trap".to_string(),
        requires: vec!["ledger".to_string()],
        rings: vec![
            (TrampolineRing::FsWrite, 0),
            (TrampolineRing::Trap, 1),
        ],
        events: vec![
            serde_json::json!({
                "kind": "fs-write",
                "path": "/workspace/ledger/wal",
                "pid": 1300,
            }),
            match serde_json::to_value(GuestToHost::TrapEvent(trap)) {
                Ok(value) => value,
                Err(_) => serde_json::Value::Null,
            },
        ],
        expected_outcomes: vec![
            PluginBuilderOutcome::Filter(FilterAction::Forward),
            PluginBuilderOutcome::Verdict(Verdict::Pass),
        ],
    }
}
