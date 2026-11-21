// see: cans/plugins/ledger.md#Plugin-ledger
use kelir_interface::plugin_model::LifecycleParticipant;

use crate::LedgerPlugin;

// see: cans/02-plugin-model.md#KelirPlugin-trait
impl LifecycleParticipant for LedgerPlugin {}

// TODO (L3) [telemetry_audit]: bind ledger strict-mode drop accounting — when LedgerConfig.strict is set, halt FUSE/ring ops closed on sink stall and surface AuditDropCounters (overflow, crc_corruption, mapper, auth_loss) instead of dropping records silently; slop: strict mode indistinguishable from best-effort, drops uncounted, unaudited guest execution
// TODO (L3) [telemetry_audit]: bind OCSF mapping — when LedgerConfig.ocsf_mapping is set, route every EventBus security event through OcsfClassId::for_event so unmapped events fail closed; slop: security events emitted as custom records, SIEM mapping absent
// TODO (L3) [telemetry_audit]: key_fingerprint rotation check — verify MerkleWalConfig.verify_key_fingerprint against the rotation sidecar and refuse startup on mismatch; slop: rotated key forks the audit chain silently
// TODO (L3) [telemetry_audit]: KMS-backed signing key fetch — resolve MerkleWalConfig.kms_provider (AWS, Azure, GCP, Vault) and reject keys shorter than MerkleWalConfig.min_signing_key_bytes; slop: short keys accepted, signatures forgeable
