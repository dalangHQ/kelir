// ILLUSTRATIVE EXCERPT — not standalone-compilable.
//
// Demonstrates Rust SDK's typestate builder pattern:
// Builder chains stay fluid; capabilities project directly onto HandlerCtx.
// SyntheticFile: FUSE-presented paths with no host backing file.
// Verification Reject: Decision::deny returns a POSIX errno.
//
// Full runnable example: artifacts/examples/autonomous-swe-agent.rs
// Canonical spec home:    cans/03-sdk-architecture.md#Rust-SDK

use kelir::plugin::{MountScope, VirtioDevice};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
enum Action {
    FixWebhook { repo: String, issue_id: String },
}

// 1. Composite tree definition unpacked at assembly time
struct WorkspaceBundle {
    policy: RepoPolicyPlugin,
}

impl CompositePlugin for WorkspaceBundle {
    fn unpack(self: Box<Self>, parent: &MountScope) -> Vec<(Box<dyn KelirPlugin>, MountScope)> {
        vec![(
            Box::new(self.policy),
            parent.intersect(&MountScope::default().with_binaries(["git"])),
        )]
    }
}

// 2. Fluid builder assembly: .with_composite() returns Builder directly
let scope = MountScope::default().with_devices([VirtioDevice::Vsock, VirtioDevice::Fs]);
let builder = builder.with_composite(
    WorkspaceBundle { policy: RepoPolicyPlugin::new(options) },
    scope,
);

// 2. Downstream handler receives typed plugin reference directly from context:
let builder = builder.exec(|exec| {
    exec
        // Serde deserializes `Action` before handler execution:
        .before("npm test", handler(|c: HandlerCtx| async move {
            let params = c.params::<Action>()?;
            Ok(Decision::Resume)
        }))
        .before("git commit *", handler(|c: HandlerCtx| {
            async move {
                if c.policy().violations().await > 0 {
                    return Ok(Decision::deny(1, "Policy violations recorded"));
                }
                Ok(Decision::Resume)
            }
        }))
});
