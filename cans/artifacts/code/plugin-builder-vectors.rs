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
// Chain cases (chain fix): one case per new chain verb, so the parity
// run builds each fixed chain in Rust, TS, and Python as one expression.
// plugin-builder-seal pins seal() then boot(); plugin-builder-trap-filter
// pins the host trap(filter, handler) form; plugin-builder-trap-scope
// pins the trap_scope closure form; plugin-builder-lock-broker pins the
// lock_broker() verb (Rust) and the storage lock-broker flag (TS/Py).
// Each case still pins rings, ordinals, events, and outcomes; the case
// name pins which chain verb the parity run must exercise.
pub fn plugin_builder_seal_case() -> PluginBuilderCase {
    let trap = TrapEvent::new(7, 1300, 100, "/bin/sh");
    PluginBuilderCase {
        name: "plugin-builder-seal".to_string(),
        requires: vec![],
        rings: vec![(TrampolineRing::Trap, 0)],
        events: vec![
            match serde_json::to_value(GuestToHost::TrapEvent(trap)) {
                Ok(value) => value,
                Err(_) => serde_json::Value::Null,
            },
        ],
        expected_outcomes: vec![PluginBuilderOutcome::Verdict(Verdict::Pass)],
    }
}

pub fn plugin_builder_trap_filter_case() -> PluginBuilderCase {
    let trap = TrapEvent::new(7, 1300, 100, "/bin/sh");
    PluginBuilderCase {
        name: "plugin-builder-trap-filter".to_string(),
        requires: vec![],
        rings: vec![(TrampolineRing::Trap, 0)],
        events: vec![
            match serde_json::to_value(GuestToHost::TrapEvent(trap)) {
                Ok(value) => value,
                Err(_) => serde_json::Value::Null,
            },
        ],
        expected_outcomes: vec![PluginBuilderOutcome::Verdict(Verdict::Pass)],
    }
}

pub fn plugin_builder_trap_scope_case() -> PluginBuilderCase {
    let trap = TrapEvent::new(7, 1300, 100, "/bin/sh");
    PluginBuilderCase {
        name: "plugin-builder-trap-scope".to_string(),
        requires: vec![],
        rings: vec![(TrampolineRing::Trap, 0)],
        events: vec![
            match serde_json::to_value(GuestToHost::TrapEvent(trap)) {
                Ok(value) => value,
                Err(_) => serde_json::Value::Null,
            },
        ],
        expected_outcomes: vec![PluginBuilderOutcome::Verdict(Verdict::Pass)],
    }
}

pub fn plugin_builder_lock_broker_case() -> PluginBuilderCase {
    let trap = TrapEvent::new(7, 1300, 100, "/bin/sh");
    PluginBuilderCase {
        name: "plugin-builder-lock-broker".to_string(),
        requires: vec![],
        rings: vec![(TrampolineRing::Trap, 0)],
        events: vec![
            match serde_json::to_value(GuestToHost::TrapEvent(trap)) {
                Ok(value) => value,
                Err(_) => serde_json::Value::Null,
            },
        ],
        expected_outcomes: vec![PluginBuilderOutcome::Verdict(Verdict::Pass)],
    }
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
