// see: cans/02-plugin-model.md#Plugin-testkit
// see: cans/15-operations.md#Multi-language-parity-gate
use crate::vectors::{VectorError, VectorSuite};

pub fn canonical_vector_names() -> Result<Vec<String>, VectorError> {
    // TODO (L3): pin the vectors.json wire format beyond {"vectors":[{"name"}]} in cans/artifacts before asserting fixture bodies here.
    Ok(VectorSuite::fixtures()?
        .vectors
        .into_iter()
        .map(|case| case.name)
        .collect())
}
