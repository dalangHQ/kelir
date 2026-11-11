// see: cans/artifacts/test-plan/case-to-test-map.yaml#cases
// see: cans/artifacts/test-plan/case-to-test-map.yaml#harness_tests
#[derive(serde::Deserialize)]
pub struct CaseMap {
    pub cases: Vec<MapEntry>,
    #[serde(default)]
    pub harness_tests: Vec<HarnessTest>,
}

// see: cans/artifacts/test-plan/case-to-test-map.yaml#harness_tests
#[derive(serde::Deserialize)]
pub struct HarnessTest {
    pub file: String,
    pub reason: String,
}

// see: cans/artifacts/test-plan/case-to-test-map.yaml#cases
#[derive(serde::Deserialize)]
pub struct MapEntry {
    pub case_id: String,
    pub status: String,
    pub test_file: Option<String>,
    pub test_fn: Option<String>,
    pub reason: Option<String>,
}
