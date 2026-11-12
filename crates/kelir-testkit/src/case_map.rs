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

// see: cans/artifacts/test-plan/l3-slop-register.yaml#entries
#[derive(serde::Deserialize)]
pub struct SlopRegister {
    #[serde(default)]
    pub entries: Vec<SlopEntry>,
    #[serde(default)]
    pub code_only: Vec<SlopCodeOnly>,
}

// see: cans/artifacts/test-plan/l3-slop-register.yaml#entries
#[derive(serde::Deserialize)]
pub struct SlopEntry {
    pub case_id: String,
    pub status: String,
    pub domain: String,
    pub target_crate: String,
    pub slop_risks: Vec<String>,
    pub must_not: Vec<String>,
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub code_todos: Vec<String>,
}

// see: cans/artifacts/test-plan/l3-slop-register.yaml#code_only
#[derive(serde::Deserialize)]
pub struct SlopCodeOnly {
    pub domain: String,
    pub target_crate: String,
    pub slop_risks: Vec<String>,
    pub must_not: Vec<String>,
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub code_todos: Vec<String>,
}
