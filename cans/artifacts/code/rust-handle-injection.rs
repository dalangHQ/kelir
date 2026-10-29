// ILLUSTRATIVE EXCERPT — not standalone-compilable.
//
// Demonstrates Rust SDK's typestate builder pattern:
// Builder chains stay fluid; each `.plugin()` appends P::Context to a
// monomorphic HList, and `c.ctx::<T>()` resolves infallibly via
// `Plugins: Has<T>`. No `c.service::<dyn Trait>()?` exists.
// SyntheticFile: FUSE-presented paths with no host backing file.
// Verification Reject: Decision::deny returns a POSIX errno.
//
// Full runnable example: artifacts/examples/autonomous-swe-agent.rs
// Canonical spec home:    cans/03-sdk-architecture.md#Rust-SDK

use kelir::plugin::{AnyPlugin, MountScope, VirtioDevice};

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
    // Unpacked plugins are AnyPlugin identity objects the builder
    // sorts into the ring-specific dispatch arrays at assembly time.
    fn unpack(self: Box<Self>, parent: &MountScope) -> Vec<(Box<dyn AnyPlugin>, MountScope)> {
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
let builder = builder
    .params::<Action>()
    // Serde deserializes `Action` before handler execution:
    .trap(TrapFilter::binary("npm").with_arg("test"), handler(|c: HandlerCtx| async move {
        let params = c.params::<Action>()?;
        Ok(Decision::Resume)
    }))
    .trap(TrapFilter::binary("git").with_arg("commit"), handler(|c: HandlerCtx<_, _>| async move {
        let policy = c.ctx::<RepoPolicyCtx>();
        if policy.violations().await > 0 {
            return Ok(Decision::deny(1, "Policy violations recorded"));
        }
        Ok(Decision::Resume)
    }))
    .on(LifecycleEvent::VmReady, handler(|c: HandlerCtx| async move {
        Ok(Decision::Resume)
    }));

// 3. Native engine planes directly on Session.
let session = builder.boot().await?;
session.terminal.resize(120, 40).await?;
