#[test]
fn non_exhaustive_dtos_reject_external_struct_literals() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
    t.pass("tests/compile_pass/*.rs");
}
