// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
use kelir_interface::fusion_plugin::TrampolineRing;
use kelir_interface::protocol_wire::{FilterAction, Verdict};

use crate::vectors_testkit::{
    UnifiedBuilderOutcome, UnifiedBuilderScope, VectorCase, VectorError, plugin_builder_case,
    plugin_builder_trap_json,
};

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
pub(crate) fn plugin_builder_chain_cases() -> Result<Vec<VectorCase>, VectorError> {
    let names = [
        "plugin-builder-seal",
        "plugin-builder-trap-filter",
        "plugin-builder-trap-scope",
        "plugin-builder-lock-broker",
    ];
    let mut cases = Vec::with_capacity(names.len() + 1);
    for (offset, name) in names.iter().enumerate() {
        let seq = 55 + offset as u64;
        cases.push(plugin_builder_case(
            name,
            seq,
            Vec::new(),
            vec![(TrampolineRing::Trap, 0)],
            Vec::new(),
            None,
            vec![plugin_builder_trap_json(seq, "/bin/sh")?],
            vec![UnifiedBuilderOutcome::Verdict(Verdict::Pass)],
        )?);
    }
    cases.push(chaos_plugin_builder_case()?);
    Ok(cases)
}

// see: cans/plugins/chaos.md#Network-chaos
// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
// see: cans/artifacts/examples/fluent-builder-chaos.rs#build_chaos_host
// TODO (L3) [pe_16_chaos_rst_injection_drops_matching_frame]: add the matching-frame vector (FilterAction::Drop, errno ECONNREFUSED) once frame_matches lands; slop: vector asserts Forward for a frame that should Drop once matching lands.
fn chaos_plugin_builder_case() -> Result<VectorCase, VectorError> {
    plugin_builder_case(
        "plugin-builder-chaos-only",
        59,
        Vec::new(),
        vec![
            (TrampolineRing::NetOutbound, 1),
            (TrampolineRing::NetInbound, 2),
            (TrampolineRing::FsWrite, 3),
        ],
        vec![(TrampolineRing::NetOutbound, 0)],
        Some(UnifiedBuilderScope {
            binaries: None,
            paths: Some(vec!["/workspace".to_string()]),
        }),
        Vec::new(),
        vec![
            UnifiedBuilderOutcome::Filter(FilterAction::Forward),
            UnifiedBuilderOutcome::Filter(FilterAction::Forward),
            UnifiedBuilderOutcome::Filter(FilterAction::Forward),
        ],
    )
}
