// crates/kelir-interface/src/action_algebra.flow.rs
//
// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the unified Action algebra: the canonical enum, the closed
// Errno set, and the Verdict / FilterAction type-alias collapse. The
// normative contract is cans/02-plugin-model.md#Unified-verdict; the
// method-mapping table is cans/11-protocol.md#Verdict-method-mapping.
//
// One Action enum for every interposer hook. Verdict and FilterAction
// are type aliases of Action, not separate enums; ring acceptance is
// restricted by each ring trait's return type (From<Action> impls per
// ring). Errno is the closed errno enum in all three SDKs; stringly
// errnos are banned at every hook boundary. A plugin that wants to deny
// returns Action::Drop; Err(PluginError::*) is reserved for "I broke"
// (panic, I/O, invariant violation) per the PluginError-scope pin.
//
// Owned by: cans/02-plugin-model.md#Unified-verdict
// see: cans/02-plugin-model.md#Unified-verdict
// see: cans/11-protocol.md#Verdict-method-mapping

use serde::{Deserialize, Serialize};

use crate::protocol::messages::{PatchPayload, SuspendUntil};

/// Closed errno set (SSOT for all three SDKs). Variant spelling is E +
/// the POSIX name in lower CamelCase (Eperm, Eacces); the wire form is
/// the SCREAMING name, except EConfig, ESecurityViolation, EUnknownKind,
/// EClockRegress, and PathTraversal, which keep their mixed-case wire
/// spelling. There is no silent fallback: an unrecognized name is not an
/// errno, and no hook boundary accepts a bare string errno.
///
/// PathTraversal is the one traversal-specific member: a lookup that
/// escapes the declared root denies with Action::Drop carrying
/// Errno::PathTraversal, and the driver maps it to EPERM before it
/// surfaces to the guest (the guest never sees a non-POSIX traversal
/// errno).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Errno {
    Eperm,
    Eacces,
    Enoent,
    Einval,
    Enospc,
    Erofs,
    Econnrefused,
    Enxio,
    Ewouldblock,
    Efault,
    Ebusy,
    Exdev,
    Epipe,
    Enodata,
    Eproto,
    Epolicy,
    Etoolquota,
    Ewxviolation,
    #[serde(rename = "EConfig")]
    Econfig,
    #[serde(rename = "ESecurityViolation")]
    Esecurityviolation,
    #[serde(rename = "EUnknownKind")]
    Eunknownkind,
    #[serde(rename = "EClockRegress")]
    Eclockregress,
    #[serde(rename = "PathTraversal")]
    PathTraversal,
}

impl Errno {
    /// Wire form: the SCREAMING POSIX name (e.g. "EPERM").
    pub fn as_str(&self) -> &'static str {
        match self {
            Errno::Eperm => "EPERM",
            Errno::Eacces => "EACCES",
            Errno::Enoent => "ENOENT",
            Errno::Einval => "EINVAL",
            Errno::Enospc => "ENOSPC",
            Errno::Erofs => "EROFS",
            Errno::Econnrefused => "ECONNREFUSED",
            Errno::Enxio => "ENXIO",
            Errno::Ewouldblock => "EWOULDBLOCK",
            Errno::Efault => "EFAULT",
            Errno::Ebusy => "EBUSY",
            Errno::Exdev => "EXDEV",
            Errno::Epipe => "EPIPE",
            Errno::Enodata => "ENODATA",
            Errno::Eproto => "EPROTO",
            Errno::Epolicy => "EPOLICY",
            Errno::Etoolquota => "ETOOLQUOTA",
            Errno::Ewxviolation => "EWXVIOLATION",
            Errno::Econfig => "EConfig",
            Errno::Esecurityviolation => "ESecurityViolation",
            Errno::Eunknownkind => "EUnknownKind",
            Errno::Eclockregress => "EClockRegress",
            Errno::PathTraversal => "PathTraversal",
        }
    }
}

/// The unified action algebra: one enum for every interposer hook
/// return (02-plugin-model #Unified-verdict). Denial is Action::Drop;
/// fatal termination is Action::Abort; both are actions, never errors.
///
/// Ring restriction is enforced by the trait signature, not by the
/// enum: FsInterposer::on_fs_write accepts only Forward | Drop |
/// Suspend | Abort; FsInterposer::on_fs_lookup accepts only Pass |
/// Drop | Suspend | Abort | Patch(FsInode). Ring-specific acceptance is
/// enforced by From<Action> on each ring's return type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Action {
    /// Proceed untouched (decision rings). Registers and memory are
    /// never modified.
    Pass,
    /// Forward the (possibly in-place-mutated) buffer (filter rings).
    Forward,
    /// Deny; errno surfaces to the guest. Policy denial is fast-fail,
    /// never silent on the wire planes (L4 gets TCP RST or ICMP Port
    /// Unreachable, L7 gets a synthetic 403 JSON receipt); the one
    /// host-response suppression is chaos loss injection, where the
    /// driver suppresses the response (plugins/chaos.md).
    Drop { errno: Errno, reason: String },
    /// Yield or hold to the host until `until`.
    Suspend { until: SuspendUntil },
    /// Fatal; pipeline freezes.
    Abort { reason: String },
    /// In-place mutation; payload schema per ring (ExecveMutation,
    /// FsInode, Custom). Not accepted on filter rings: the transformed
    /// buffer IS the payload there.
    Patch(PatchPayload),
}

/// Type aliases, not separate enums: the Verdict / FilterAction split
/// collapses onto Action here. Old call sites keep resolving; new code
/// writes Action directly. No third alias exists, and PluginError
/// never carries a policy denial (PluginError-scope pin).
pub type Verdict = Action;
pub type FilterAction = Action;
