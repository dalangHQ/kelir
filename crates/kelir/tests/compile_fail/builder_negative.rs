// see: cans/02-plugin-model.md#Unified-builder
// see: cans/02-plugin-model.md#Nested-scopes
// see: cans/02-plugin-model.md#One-validation,-at-seal-and-boot
// see: fluent-chain.work-order.md Part 1 §4.4 (comment-pinned negative
#[test]
fn builder_negative() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/cases/*.rs");
}
