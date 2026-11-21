// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use kelir_interface::hex_codec::encode_hex;

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepDagStorage {
    ContentAddressed,
}

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepDagConfig {
    pub enabled: bool,
    pub require_signed_attribution: bool,
    pub require_separation_of_duties: bool,
    pub storage: StepDagStorage,
    // see: cans/plugins/tool-guard.md#Approval-authorization
    pub reviewer_ttl_seconds: u64,
}

impl Default for StepDagConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            require_signed_attribution: true,
            require_separation_of_duties: true,
            storage: StepDagStorage::ContentAddressed,
            reviewer_ttl_seconds: 300,
        }
    }
}

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepDagError {
    Disabled,
    MissingAttribution(String),
    SignatureVerificationPending(String),
    SeparationOfDuties(String),
    UnknownParent([u8; 32]),
}

impl std::fmt::Display for StepDagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => f.write_str("step dag is disabled"),
            Self::MissingAttribution(detail) => {
                write!(f, "step lacks signed attribution: {detail}")
            }
            Self::SignatureVerificationPending(detail) => {
                write!(f, "step attribution signature unverified: {detail}")
            }
            Self::SeparationOfDuties(detail) => {
                write!(f, "separation of duties rejects step: {detail}")
            }
            Self::UnknownParent(id) => write!(f, "step parent {} is unknown", hex_parent(id)),
        }
    }
}

impl std::error::Error for StepDagError {}

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
fn hex_parent(id: &[u8; 32]) -> String {
    encode_hex(id)
}

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepAttribution {
    pub reviewer: String,
    pub public_key: [u8; 32],
    pub signature: Vec<u8>,
}

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepNode {
    pub id: [u8; 32],
    pub parents: Vec<[u8; 32]>,
    pub actor: String,
    pub attribution: Option<StepAttribution>,
}

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
pub fn step_content_id(parents: &[[u8; 32]], actor: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    for parent in parents {
        hasher.update(parent);
    }
    hasher.update(actor.as_bytes());
    *hasher.finalize().as_bytes()
}

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
pub fn merkle_root_over_heads(heads: &[[u8; 32]]) -> [u8; 32] {
    if heads.is_empty() {
        return *blake3::hash(&[]).as_bytes();
    }
    let mut level: Vec<[u8; 32]> = heads.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        let mut index = 0;
        while index < level.len() {
            let left = level[index];
            let right = if index + 1 < level.len() {
                level[index + 1]
            } else {
                left
            };
            let mut hasher = blake3::Hasher::new();
            hasher.update(&[0x01]);
            hasher.update(&left);
            hasher.update(&right);
            next.push(*hasher.finalize().as_bytes());
            index += 2;
        }
        level = next;
    }
    level[0]
}

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
pub struct StepDag {
    config: StepDagConfig,
    nodes: HashMap<[u8; 32], StepNode>,
    heads: Vec<[u8; 32]>,
}

impl StepDag {
    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn new(config: StepDagConfig) -> Self {
        Self {
            config,
            nodes: HashMap::new(),
            heads: Vec::new(),
        }
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn config(&self) -> &StepDagConfig {
        &self.config
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn heads(&self) -> &[[u8; 32]] {
        &self.heads
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn get(&self, id: &[u8; 32]) -> Option<&StepNode> {
        self.nodes.get(id)
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn append(
        &mut self,
        parents: Vec<[u8; 32]>,
        actor: String,
        attribution: Option<StepAttribution>,
    ) -> Result<[u8; 32], StepDagError> {
        if !self.config.enabled {
            return Err(StepDagError::Disabled);
        }
        for parent in &parents {
            if !self.nodes.contains_key(parent) {
                return Err(StepDagError::UnknownParent(*parent));
            }
        }
        if self.config.require_separation_of_duties {
            for parent in &parents {
                if let Some(node) = self.nodes.get(parent) {
                    if node.actor == actor {
                        return Err(StepDagError::SeparationOfDuties(format!(
                            "actor {actor} already authored parent step"
                        )));
                    }
                    if node
                        .attribution
                        .as_ref()
                        .is_some_and(|attribution| attribution.reviewer == actor)
                    {
                        return Err(StepDagError::SeparationOfDuties(format!(
                            "actor {actor} already reviewed parent step"
                        )));
                    }
                }
            }
        }
        let id = step_content_id(&parents, &actor);
        if let Some(attribution) = &attribution {
            check_reviewer_attribution(attribution, &actor, self.config.reviewer_ttl_seconds)?;
            if self.config.require_signed_attribution {
                attribution.verify(&id)?;
            }
        } else if self.config.require_signed_attribution {
            return Err(StepDagError::MissingAttribution(format!(
                "step by {actor} carries no reviewer attribution"
            )));
        }
        self.heads.retain(|head| !parents.contains(head));
        self.heads.push(id);
        self.nodes.insert(
            id,
            StepNode {
                id,
                parents,
                actor,
                attribution,
            },
        );
        Ok(id)
    }

    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn merkle_root(&self) -> [u8; 32] {
        merkle_root_over_heads(&self.heads)
    }
}

impl Default for StepDag {
    fn default() -> Self {
        Self::new(StepDagConfig::default())
    }
}

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
pub fn check_reviewer_attribution(
    attribution: &StepAttribution,
    actor: &str,
    ttl_seconds: u64,
) -> Result<(), StepDagError> {
    if attribution.reviewer.is_empty() {
        return Err(StepDagError::MissingAttribution(
            "reviewer attribution is empty".to_string(),
        ));
    }
    if attribution.reviewer == actor {
        return Err(StepDagError::SeparationOfDuties(format!(
            "reviewer {} is the step actor",
            attribution.reviewer
        )));
    }
    if ttl_seconds == 0 {
        return Err(StepDagError::MissingAttribution(
            "reviewer attribution TTL is zero".to_string(),
        ));
    }
    Ok(())
}

// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
impl StepAttribution {
    // see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
    pub fn verify(&self, _step_id: &[u8; 32]) -> Result<(), StepDagError> {
        if self.signature.is_empty() {
            return Err(StepDagError::MissingAttribution(format!(
                "reviewer {} carries no signature",
                self.reviewer
            )));
        }
        // TODO: [lifecycle_checkpoint]: verify the Ed25519 reviewer signature against public_key over the step id.; slop: unsigned reviewer attribution accepted, SoD unenforced (deferred to L3)
        Err(StepDagError::SignatureVerificationPending(format!(
            "reviewer {} Ed25519 verification is deferred to L3",
            self.reviewer
        )))
    }
}
