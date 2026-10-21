// ILLUSTRATIVE EXCERPT — not standalone-compilable.
//
// Demonstrates Rust SDK's compile-time plugin verification invariant:
// removing `.plugin_with_handle(RepoPolicyPlugin)` from the chain makes
// the typed handle unavailable, and every closure capturing it fails
// to compile (rustc E0425).
//
// Full runnable example: artifacts/examples/autonomous-swe-agent.rs
// Canonical spec home:    cans/03-sdk-architecture.md#Rust-SDK

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
enum Action {
    FixWebhook { repo: String, issue_id: String },
}

// 1. Atomic handle extraction:
let (builder, policy) = builder.plugin_with_handle(RepoPolicyPlugin::new(options));

// 2. Downstream handler captures typed handle:
let builder = builder.exec(move |exec| {
    let policy = policy.clone();
    exec
        // Serde deserializes `Action` before handler execution:
        .before("npm test", handler(|c: HandlerCtx| async move {
            let params = c.params::<Action>()?;
            Ok(Decision::Resume)
        }))
        // Compiler guarantees `policy` exists in scope:
        .before("git commit *", handler(move |_c: HandlerCtx| {
            let policy = policy.clone();
            async move {
                if policy.violations().await > 0 {
                    return Ok(Decision::deny(1, "Policy violations recorded"));
                }
                Ok(Decision::Resume)
            }
        }))
});
