// see: cans/02-plugin-model.md#Plugin-testkit
// see: cans/15-operations.md#Multi-language-parity-gate
use crate::vectors_testkit::{VectorError, VectorSuite};

pub fn canonical_vector_names() -> Result<Vec<String>, VectorError> {
    // TODO (L3) [protocol_mux]: pin the vectors.json wire format beyond {"vectors":[{"name"}]} in cans/artifacts before asserting fixture bodies here.; slop: fixture bodies asserted, wire format never pinned
    Ok(VectorSuite::fixtures()?
        .vectors
        .into_iter()
        .map(|case| case.name)
        .collect())
}
