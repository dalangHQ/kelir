// see: cans/02-plugin-model.md#Plugin-testkit
// see: cans/15-operations.md#Multi-language-parity-gate
pub fn canonical_vector_names() -> Vec<&'static str> {
    // TODO (L3): serialize the synthetic fixtures themselves (TrapEvent JSON, raw frames, Decision expectations) instead of names only; the vectors.json wire format beyond {"vectors":[{"name"}]} is not pinned in cans/artifacts.
    vec![
        "trap-git-commit-pass",
        "trap-git-push-deny-epolicy",
        "trap-pipe-install-rewrite-execve-mutation",
        "trap-mutating-tool-suspend-approval",
        "trap-clock-skew-deadline-suspend",
        "trap-unverified-caller-abort-ewxviolation",
        "heartbeat-telemetry-observer-pass",
        "watchdog-reboot-suppress",
        "watchdog-terminate-mitigate",
        "veto-fork",
        "veto-suspend",
        "veto-clock-skew",
    ]
}
