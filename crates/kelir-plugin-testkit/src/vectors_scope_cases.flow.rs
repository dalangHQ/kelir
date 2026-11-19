// see: cans/02-plugin-model.md#Unified-builder
// see: cans/15-operations.md#Unified-builder-parity-vectors
// see: cans/artifacts/examples/policy-matrix.rs#PolicyMatrixPlugin
// see: cans/03-sdk-architecture.md#Unified-builder-surface
use kelir_interface::driver_interface::{WatchdogAction, WatchdogEvent, WatchdogVerdict};
use kelir_interface::fusion_plugin::TrampolineRing;
use kelir_interface::plugin_model::VetoEvent;
use kelir_interface::protocol_wire::{ExecveMutation, FilterAction, PatchPayload, Verdict};

use crate::vectors_testkit::{
    UnifiedBuilderOutcome, UnifiedBuilderScope, VectorCase, VectorError, VetoCase, WatchdogCase,
    plugin_builder_case, plugin_builder_trap_json, plugin_builder_verb_event,
    trap_filter_five_predicate_json,
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
                (
                    "signal",
                    TrampolineRing::Signal,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
                (
                    "fork",
                    TrampolineRing::Fork,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
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
                ("fs-setattr", TrampolineRing::FsSetattr, None),
                ("fs-symlink", TrampolineRing::FsSymlink, None),
                ("fs-mkdir", TrampolineRing::FsMkdir, None),
                ("fs-readdir", TrampolineRing::FsReaddir, None),
                ("fs-readlink", TrampolineRing::FsReadlink, None),
                ("fs-unlink", TrampolineRing::FsUnlink, None),
                ("fs-rename", TrampolineRing::FsRename, None),
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
                (
                    "socket-connect",
                    TrampolineRing::SocketConnect,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
                ),
                (
                    "dns-query",
                    TrampolineRing::DnsQuery,
                    Some(UnifiedBuilderOutcome::Filter(FilterAction::Forward)),
                ),
                (
                    "socket-bind",
                    TrampolineRing::SocketBind,
                    Some(UnifiedBuilderOutcome::Verdict(Verdict::Pass)),
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
                ("exec-stdin", TrampolineRing::ExecStdin, None),
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
                (
                    "host-containment-breach",
                    TrampolineRing::HostContainmentBreach,
                    None,
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
    let mut cases = Vec::with_capacity(scopes.len() * 3);
    for (seq, (scope, verbs)) in (seq_start..).zip(scopes.iter()) {
        let host_ring = verbs.first().map(|(_, ring, _)| *ring);
        let mut kind1 = scope_kind_case(prefix, scope, seq, verbs, host_ring, merge, "core")?;
        if *scope == "trap"
            && let Some(builder) = kind1.plugin_builder.as_mut()
        {
            builder.filter = Some(trap_filter_five_predicate_json());
        }
        if *scope == "lifecycle" && !merge {
            kind1.veto = Some(VetoCase {
                event: VetoEvent::Fork,
                callback_allows: false,
            });
        }
        if *scope == "lifecycle" && merge {
            kind1.veto = Some(VetoCase {
                event: VetoEvent::Suspend,
                callback_allows: false,
            });
        }
        if *scope == "telemetry" && !merge {
            kind1.watchdog = Some(WatchdogCase {
                event: WatchdogEvent {
                    timeout_ms: 5_000,
                    action: WatchdogAction::Reboot,
                    fired_at_ns: 60_000_000_000,
                    last_heartbeat_ms: 55_000,
                    last_active_pid: None,
                },
                expected_verdict: WatchdogVerdict::Suppress,
            });
        }
        if *scope == "telemetry" && merge {
            kind1.watchdog = Some(WatchdogCase {
                event: WatchdogEvent {
                    timeout_ms: 5_000,
                    action: WatchdogAction::Terminate {
                        emit_diagnostic: true,
                    },
                    fired_at_ns: 60_000_000_000,
                    last_heartbeat_ms: 55_000,
                    last_active_pid: Some(1234),
                },
                expected_verdict: WatchdogVerdict::Mitigate,
            });
        }
        cases.push(kind1);
        let mut kind2 = scope_kind_case(
            prefix,
            scope,
            seq,
            &verbs[..1],
            host_ring,
            merge,
            "requires",
        )?;
        if let Some(builder) = kind2.plugin_builder.as_mut() {
            builder.requires = vec!["ledger".to_string()];
        }
        if *scope == "lifecycle" && !merge {
            kind2.veto = Some(VetoCase {
                event: VetoEvent::ClockSkew,
                callback_allows: false,
            });
        }
        cases.push(kind2);
        let kind3_name = if merge { "rules" } else { "events" };
        let eventful: Vec<ScopeVerb> = verbs
            .iter()
            .filter(|(verb, _, _)| {
                matches!(
                    *verb,
                    "trap" | "process-exit" | "signal" | "fork" | "memory-pressure" | "heartbeat"
                )
            })
            .cloned()
            .collect();
        let kind3_verbs: &[ScopeVerb] = if eventful.is_empty() {
            &verbs[..1]
        } else {
            &eventful
        };
        cases.push(scope_kind_case(
            prefix,
            scope,
            seq,
            kind3_verbs,
            host_ring,
            merge,
            kind3_name,
        )?);
    }
    Ok(cases)
}

// see: cans/02-plugin-model.md#Nested-scopes
fn scope_kind_case(
    prefix: &str,
    scope: &str,
    seq: u64,
    verbs: &[(&'static str, TrampolineRing, Option<UnifiedBuilderOutcome>)],
    host_ring: Option<TrampolineRing>,
    merge: bool,
    kind: &str,
) -> Result<VectorCase, VectorError> {
    let name = match kind {
        "core" => format!("{prefix}-{scope}"),
        other => format!("{prefix}-{scope}-{other}"),
    };
    let mut rings = Vec::with_capacity(verbs.len());
    let mut events = Vec::new();
    let mut outcomes = Vec::new();
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
    plugin_builder_case(
        &name,
        seq,
        Vec::new(),
        rings,
        host_rings,
        scope_pin,
        events,
        outcomes,
        None,
    )
}
