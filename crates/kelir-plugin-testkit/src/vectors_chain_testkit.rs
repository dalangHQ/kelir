// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
use kelir_interface::fusion_plugin::TrampolineRing;
use kelir_interface::protocol_wire::Verdict;

use crate::vectors_testkit::{
    UnifiedBuilderOutcome, VectorCase, VectorError, plugin_builder_case, plugin_builder_trap_json,
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
    let mut cases = Vec::with_capacity(names.len());
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
    Ok(cases)
}
