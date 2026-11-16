// see: cans/01-architecture.md#Policy-plane
// see: cans/02-plugin-model.md#Policy-plane
use serde::{Deserialize, Serialize};

// see: cans/01-architecture.md#Policy-plane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PolicyPlaneMutability {
    #[default]
    Static,
    DynamicSigned,
}

// see: cans/01-architecture.md#Policy-plane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HardwareInvariant {
    MmuWx,
    CapabilityLatticeMonotonicity,
}

// see: cans/01-architecture.md#Policy-plane
pub const HARDWARE_INVARIANTS: &[HardwareInvariant] = &[
    HardwareInvariant::MmuWx,
    HardwareInvariant::CapabilityLatticeMonotonicity,
];

// see: cans/01-architecture.md#Policy-plane
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicRule {
    pub id: String,
    pub fragment: String,
}

// see: cans/01-architecture.md#Policy-plane
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyPlaneError {
    StaticPlane(String),
    UnsignedLease(String),
    SignatureVerificationPending(String),
    InvariantWidening(String),
}

impl std::fmt::Display for PolicyPlaneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaticPlane(detail) => write!(f, "policy plane is static: {detail}"),
            Self::UnsignedLease(detail) => write!(f, "unsigned operator lease rejected: {detail}"),
            Self::SignatureVerificationPending(detail) => {
                write!(f, "operator lease signature unverified: {detail}")
            }
            Self::InvariantWidening(detail) => {
                write!(f, "operator lease widens a hardware invariant: {detail}")
            }
        }
    }
}

impl std::error::Error for PolicyPlaneError {}

// see: cans/01-architecture.md#Policy-plane
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorLease {
    pub lease_id: String,
    pub rule: DynamicRule,
    pub signature: Vec<u8>,
    pub signer_public_key: Vec<u8>,
    pub expires_at_ns: u64,
}

impl OperatorLease {
    // see: cans/01-architecture.md#Policy-plane
    pub fn verify_signature(&self) -> Result<(), PolicyPlaneError> {
        if self.signature.is_empty() || self.signer_public_key.is_empty() {
            return Err(PolicyPlaneError::UnsignedLease(format!(
                "lease {} carries no signature",
                self.lease_id
            )));
        }
        // TODO (L3) [plugin_engine]: verify the Ed25519 lease signature against signer_public_key over the rule fragment.; slop: unsigned lease accepted, dynamic rules unauthenticated
        Err(PolicyPlaneError::SignatureVerificationPending(format!(
            "lease {} Ed25519 verification is TODO (L3)",
            self.lease_id
        )))
    }
}

// see: cans/01-architecture.md#Policy-plane
pub struct PolicyPlane {
    pub mutability: PolicyPlaneMutability,
    rules: Vec<DynamicRule>,
}

impl PolicyPlane {
    // see: cans/01-architecture.md#Policy-plane
    pub fn new(mutability: PolicyPlaneMutability) -> Self {
        Self {
            mutability,
            rules: Vec::new(),
        }
    }

    // see: cans/01-architecture.md#Policy-plane
    pub fn rules(&self) -> &[DynamicRule] {
        &self.rules
    }

    // see: cans/01-architecture.md#Policy-plane
    pub fn rule(&self, id: &str) -> Option<&DynamicRule> {
        self.rules.iter().find(|rule| rule.id == id)
    }

    // see: cans/01-architecture.md#Policy-plane
    pub fn apply_lease(&mut self, lease: &OperatorLease) -> Result<(), PolicyPlaneError> {
        if self.mutability != PolicyPlaneMutability::DynamicSigned {
            return Err(PolicyPlaneError::StaticPlane(format!(
                "lease {} needs dynamic_signed mutability",
                lease.lease_id
            )));
        }
        if lease.rule.id.starts_with("hardware.") {
            return Err(PolicyPlaneError::InvariantWidening(format!(
                "lease {} targets hardware rule {}",
                lease.lease_id, lease.rule.id
            )));
        }
        lease.verify_signature()?;
        match self.rules.iter_mut().find(|rule| rule.id == lease.rule.id) {
            Some(rule) => *rule = lease.rule.clone(),
            None => self.rules.push(lease.rule.clone()),
        }
        Ok(())
    }
}

// see: cans/11-protocol.md#Control-channel
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSyncFragment {
    pub lease_id: String,
    pub rule: DynamicRule,
    pub signature: Vec<u8>,
    pub signer_public_key: Vec<u8>,
    pub expires_at_ns: u64,
}

impl ProfileSyncFragment {
    // see: cans/11-protocol.md#Control-channel
    pub fn into_lease(self) -> OperatorLease {
        OperatorLease {
            lease_id: self.lease_id,
            rule: self.rule,
            signature: self.signature,
            signer_public_key: self.signer_public_key,
            expires_at_ns: self.expires_at_ns,
        }
    }
}

// see: cans/11-protocol.md#Control-channel
pub fn hot_reload_fragment(
    plane: &mut PolicyPlane,
    fragment: ProfileSyncFragment,
) -> Result<(), PolicyPlaneError> {
    plane.apply_lease(&fragment.into_lease())
}
