// crates/kelir-interface/src/telemetry/hook_trace.flow.rs
//
// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the ExecutionReceipt hook-trace shape: the HookSpan struct,
// the hook_trace vector on ExecutionReceipt, the VerdictKind
// classification, and the typed receipt-extension accessors. The
// normative contract is cans/10-attestation.md#Execution-receipts;
// the diagnostic-only posture (excluded from Merkle roots, emitted to
// OTel as spans) mirrors the phase spans contract in the same file.
//
// Owned by: cans/10-attestation.md#Execution-receipts
// see: cans/10-attestation.md#Hook-trace
// see: cans/10-attestation.md#Typed-extensions
// see: cans/artifacts/code/execution-receipt.rs

use serde::{Deserialize, Serialize};

use crate::telemetry::receipt_attest::ExecutionReceipt;

/// Ring a hook fired on. The receipt trace names the taxonomy ring
/// (the ring traits of cans/02-plugin-model.md#Segregated-ring-traits):
/// Trap, Fs, Net, Console, Exec, Host, Lifecycle, Telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookRing {
    Trap,
    Fs,
    Net,
    Console,
    Exec,
    Host,
    Lifecycle,
    Telemetry,
}

/// Verdict classification for one HookSpan: the Action variant the
/// hook returned, spelled as its kind tag (see
/// cans/artifacts/code/action-algebra.rs for the canonical enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerdictKind {
    Pass,
    Forward,
    Drop,
    Suspend,
    Abort,
    Patch,
    /// Engine denial on hook timeout (per-ring timeout):
    /// cans/02-plugin-model.md#Per-ring-timeouts
    Timeout,
}

/// One hook that fired for the trapped process. Populated for every
/// hook that fired; diagnostic only: excluded from Merkle roots
/// (consistent with phase spans) and emitted to OTel as spans.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSpan {
    /// Name of the plugin (or "host") that registered the hook.
    pub plugin: String,
    /// Ring the hook fired on.
    pub ring: HookRing,
    /// Action variant the hook returned.
    pub verdict_kind: VerdictKind,
    /// Duration of the hook invocation, nanoseconds.
    pub duration_ns: u64,
    /// Registration ordinal within the ring (see
    /// cans/02-plugin-model.md#Hook-ordering).
    pub ordinal: u32,
}

/// Deterministic policy projection of one HookSpan: everything the
/// Merkle leaf commits about a fired hook, without the timing. The
/// ring is the kebab-case tag ("trap", "fs-read"); duration_ns is
/// deliberately absent, so identical policy executions hash
/// identically. Derived from HookSpan at attach time; the full span
/// stays diagnostic and out of the leaf.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPolicyDescriptor {
    pub plugin: String,
    pub ring: String,
    pub verdict_kind: VerdictKind,
    pub ordinal: u32,
}

/// One in-place masked record: the raw payload digest committed
/// before the mask rewrote the bytes. pre_redaction_blake3 is None
/// only when the digest was never committed, which the acceptance
/// examples fail on; a complete receipt carries Some for every entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskedRecord {
    pub record: String,
    pub pre_redaction_blake3: Option<String>,
    pub masked_bytes: u64,
}

/// One step-DAG lineage entry: a checkpoint node on the receipt's
/// chain from its step head back to the DAG root. reviewer_id and
/// reviewer_signature carry the Ed25519-signed reviewer identity when
/// step attribution landed; both None until the step-DAG lane records
/// it on the node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepLineageEntry {
    pub node_id: u64,
    pub parent_node_id: Option<u64>,
    pub reviewer_id: Option<String>,
    pub reviewer_signature: Option<String>,
}

impl ExecutionReceipt {
    /// hook_trace accessor: every hook that fired for the trapped
    /// process, in firing order. Diagnostic only; never part of
    /// receipt identity (Merkle roots exclude it, like phase spans).
    pub fn hooks(&self) -> &[HookSpan] {
        &self.hook_trace
    }

    /// Typed extension accessor: the fs:vfs extension payload, when a
    /// producer emitted one. None when the namespace is absent.
    /// The typed block (see TypedExtensions below) is decoded from the
    /// namespace's opaque bytes at mint time; the accessor is a read.
    pub fn fs_vfs(&self) -> Option<&FsVfsExt> {
        self.typed_extensions.fs_vfs.as_ref()
    }

    /// Typed extension accessor: the net:l7:sql extension payload,
    /// when a producer emitted one. None when the namespace is absent.
    pub fn net_l7_sql(&self) -> Option<&NetL7SqlExt> {
        self.typed_extensions.net_l7_sql.as_ref()
    }

    /// Third-party fallback: raw extension entries for namespaces
    /// without a typed accessor. Namespace-carried pairs; the payload
    /// stays opaque bytes (the raw pairs vector).
    pub fn ext_raw(&self) -> &[(String, Vec<u8>)] {
        &self.ext_raw_entries
    }
}

/// Typed extension storage on ExecutionReceipt: the typed decode of the
/// namespaces that carry first-party payloads. fs_vfs decodes the
/// "fs:vfs" namespace; net_l7_sql decodes "net:l7:sql"; both None when
/// no producer emitted the namespace. Third-party namespaces stay in
/// the opaque raw vector (ext_raw()).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypedExtensions {
    pub fs_vfs: Option<FsVfsExt>,
    pub net_l7_sql: Option<NetL7SqlExt>,
}

/// Typed fs:vfs extension payload (namespace "fs:vfs"): the synthetic
/// FUSE view the fs plane served for the process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsVfsExt {
    /// Synthetic inodes served at the trap/exit boundary.
    pub served_inodes: u32,
    /// Distinct workspace roots under interception.
    pub roots: Vec<String>,
}

/// Typed net:l7:sql extension payload (namespace "net:l7:sql"): the
/// L7 SQL statement accounting plugin-db-proxy emits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetL7SqlExt {
    /// Statements observed in the receipt window.
    pub statements: u32,
    /// Distinct prepared-statement shapes.
    pub distinct_shapes: u32,
}
