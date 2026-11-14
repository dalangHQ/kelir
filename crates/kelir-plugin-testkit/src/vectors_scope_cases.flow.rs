// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
// see: cans/artifacts/examples/policy-matrix.rs#PolicyMatrixPlugin
// see: cans/03-sdk-architecture.md#Unified-builder-surface
use kelir_interface::fusion_plugin::TrampolineRing;
use kelir_interface::protocol_wire::{ExecveMutation, FilterAction, PatchPayload, Verdict};

use crate::vectors_testkit::{
    UnifiedBuilderOutcome, UnifiedBuilderScope, VectorCase, VectorError, plugin_builder_case,
    plugin_builder_trap_json, plugin_builder_verb_event, trap_filter_five_predicate_json,
};

pub(crate) fn plugin_builder_scope_cases(
    prefix: &str,
    seq_start: u64,
    merge: bool,
) -> Result<Vec<VectorCase>, VectorError> {
    type ScopeVerb = (&'static str, TrampolineRing, Option<UnifiedBuilderOutcome>);
    let scopes: &[(&str, &[ScopeVerb])] = &[
        (
            "trap",
            &[
                (
                    "trap",
                    TrampolineRing::Trap,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
                ("process-exit", TrampolineRing::ProcessExit, None),
            ],
        ),
        (
            "fs",
            &[
                (
                    "fs-lookup",
                    TrampolineRing::FsLookup,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
                ("fs-read", TrampolineRing::FsRead, None),
                (
                    "fs-write",
                    TrampolineRing::FsWrite,
                    Some(UnifiedBuilderOutcome::Filter(FilterAction::Forward)),
                ),
                (
                    "fs-lock",
                    TrampolineRing::FsLock,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
            ],
        ),
        (
            "net",
            &[
                (
                    "net-outbound",
                    TrampolineRing::NetOutbound,
                    Some(UnifiedBuilderOutcome::Filter(FilterAction::Forward)),
                ),
                (
                    "net-inbound",
                    TrampolineRing::NetInbound,
                    Some(UnifiedBuilderOutcome::Filter(FilterAction::Forward)),
                ),
            ],
        ),
        (
            "console",
            &[
                ("console-input", TrampolineRing::ConsoleInput, None),
                ("console-output", TrampolineRing::ConsoleOutput, None),
            ],
        ),
        (
            "exec",
            &[
                ("exec-stdout", TrampolineRing::ExecStdout, None),
                ("exec-stderr", TrampolineRing::ExecStderr, None),
            ],
        ),
        (
            "host",
            &[
                (
                    "host-lsm-exec",
                    TrampolineRing::HostLsmExec,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
                (
                    "host-pressure",
                    TrampolineRing::HostPressure,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
                (
                    "host-keyring-access",
                    TrampolineRing::HostKeyringAccess,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
                ("host-preempt", TrampolineRing::HostPreempt, None),
            ],
        ),
        (
            "lifecycle",
            &[
                ("vm-init", TrampolineRing::Lifecycle, None),
                ("vm-ready", TrampolineRing::Lifecycle, None),
                ("vm-shutdown", TrampolineRing::Lifecycle, None),
                ("pre-commit", TrampolineRing::Lifecycle, None),
                ("post-commit", TrampolineRing::Lifecycle, None),
            ],
        ),
        (
            "telemetry",
            &[
                ("memory-pressure", TrampolineRing::MemoryPressure, None),
                ("heartbeat", TrampolineRing::Heartbeat, None),
                ("watchdog", TrampolineRing::Watchdog, None),
            ],
        ),
    ];
    let mut cases = Vec::with_capacity(scopes.len());
    for (seq, (scope, verbs)) in (seq_start..).zip(scopes.iter()) {
        let mut rings = Vec::with_capacity(verbs.len());
        let mut events = Vec::new();
        let mut outcomes = Vec::new();
        let host_ring = verbs.first().map(|(_, ring, _)| *ring);
        for (ordinal, (verb, ring, outcome)) in verbs.iter().enumerate() {
            let ordinal = if merge {
                ordinal as u32 + 1
            } else {
                ordinal as u32
            };
            rings.push((*ring, ordinal));
            let event = if *verb == "trap" {
                if merge {
                    Some(plugin_builder_trap_json(seq, "policy-rewrite")?)
                } else {
                    Some(plugin_builder_trap_json(seq, "policy-allow")?)
                }
            } else {
                plugin_builder_verb_event(verb, 1300)?
            };
            if let Some(event) = event {
                events.push(event);
            }
            let outcome = if merge && *verb == "trap" {
                Some(UnifiedBuilderOutcome::Verdict(Verdict::Patch(
                    PatchPayload::ExecveMutation(ExecveMutation {
                        new_binary: Some("/bin/policy-allow".to_string()),
                        new_argv: Some(vec!["policy-allow".to_string(), "--rewritten".to_string()]),
                        set_env: vec![("KELIR_POLICY_REWRITE".to_string(), "1".to_string())],
                        unset_env: Vec::new(),
                    }),
                )))
            } else {
                outcome.clone()
            };
            if let Some(outcome) = outcome {
                outcomes.push(outcome);
            }
        }
        let host_rings = match (merge, host_ring) {
            (true, Some(ring)) => vec![(ring, 0)],
            _ => Vec::new(),
        };
        let scope_pin = if merge {
            Some(UnifiedBuilderScope {
                binaries: None,
                paths: Some(vec!["/workspace".to_string()]),
            })
        } else {
            None
        };
        let filter = if *scope == "trap" {
            Some(trap_filter_five_predicate_json())
        } else {
            None
        };
        cases.push(plugin_builder_case(
            &format!("{prefix}-{scope}"),
            seq,
            Vec::new(),
            rings,
            host_rings,
            scope_pin,
            events,
            outcomes,
            filter,
        )?);
    }
    Ok(cases)
}
