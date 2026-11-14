// cans/artifacts/code/session-accessors.rs
//
// Unified session and hook-context accessors — content home.
// Three accessors, one meaning: session.plane(), session.cap(), c.cap().
// Signatures and sugar spellings live here; the normative invariant
// (one accessor per concept, deprecated-form table) lives in
// cans/03-sdk-architecture.md#Service-lookup-across-languages.
//
// Owned by: cans/03-sdk-architecture.md#Service-lookup-across-languages

use crate::handlers::HandlerCtx;

/// Marker for engine plane handle types reachable via session.plane().
/// The plane set is closed: VmHandle, TerminalPlane, StoragePlane,
/// WirePlane, CheckpointPlane, HostOps, VsockHandle.
pub trait PlaneHandle: sealed::Sealed {}

/// Marker for plugin capability types reachable via session.cap() / c.cap().
/// A Capability is the plugin's Context type (KelirPlugin::Context).
pub trait Capability: sealed::Sealed {}

mod sealed {
    pub trait Sealed {}
}

/// session.plane::<P>() — the one plane accessor (Rust resolves by type).
///
/// Documented sugar, all resolving through this accessor:
///   session.vm()        == session.plane::<VmHandle>()
///   session.terminal    == session.plane::<TerminalPlane>()
///   session.storage     == session.plane::<StoragePlane>()
///   session.wire        == session.plane::<WirePlane>()
///   session.checkpoint  == session.plane::<CheckpointPlane>()
///   session.host        == session.plane::<HostOps>()
///   session.vsock       == session.plane::<VsockHandle>()
pub trait SessionPlanes {
    fn plane<P: PlaneHandle>(&self) -> &P;
}

/// session.cap::<T>() — the one capability accessor on Session.
/// Infallible by typestate: `Session<Plugins, Params>` where
/// `Plugins: Has<T>`; a plugin that is not mounted is a compile error,
/// never a runtime Result or Option.
///
/// session.ctx::<T, _>() and session.ctx::<T>("name") are deleted forms.
pub trait SessionCaps {
    fn cap<T: Capability>(&self) -> &T;
}

/// c.cap::<T>() — the same capability resolution inside a hook.
/// HandlerCtx resolves T from the accumulated Plugins HList via Has<T>;
/// no string keys, no attribute bags, no c["<key>"] indexing.
///
/// c.ctx::<T>() is a deleted form.
pub trait HandlerCtxCaps {
    fn cap<T: Capability>(&self) -> &T;
}

// ── Plugin deps: single source of truth ───────────────────────────
//
// `type Deps` is DERIVED from the plugin's require::<T>() calls by
// #[derive(KelirDeps)] — it is never hand-maintained. The derive walks
// every require::<T>() site in the plugin and emits the Deps HList;
// it is the only writer of `type Deps`, so a hand-written list that
// disagrees with the require sites cannot exist.
//
// #[derive(KelirDeps)]
// struct RepoPolicyPlugin;
//
// // expands to (one HList slot per require::<LedgerCtx>() site):
// impl PluginDeps for RepoPolicyPlugin {
//     type Deps = HCons<LedgerCtx, HNil>;
// }
//
// TypeScript and Python populate the same registry from their require
// spellings: requires("ledger") (TS) and requires: tuple[str, ...] (Py);
// see session-accessors.ts / session-accessors.py.
