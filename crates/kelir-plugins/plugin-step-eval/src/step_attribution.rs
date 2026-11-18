// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
use crate::step_dag::{StepAttribution, StepDagError};

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
        // TODO (L3) [lifecycle_checkpoint]: verify the Ed25519 reviewer signature against public_key over the step id.; slop: unsigned reviewer attribution accepted, SoD unenforced
        Err(StepDagError::SignatureVerificationPending(format!(
            "reviewer {} Ed25519 verification is TODO (L3)",
            self.reviewer
        )))
    }
}
