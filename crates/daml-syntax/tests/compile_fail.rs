//! Compile-fail coverage for `daml-syntax` public API construction boundaries.

#[test]
fn api_shape_rejects_invalid_construction_and_matching() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
