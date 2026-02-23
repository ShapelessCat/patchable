#[test]
fn derive_macro_reports_expected_failures() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/derive_fail_borrowed_fields.rs");
    tests.compile_fail("tests/ui/derive_fail_non_struct.rs");
    tests.compile_fail("tests/ui/derive_fail_patchable_field_not_simple_generic.rs");
    tests.compile_fail("tests/ui/derive_fail_patchable_unknown_parameter.rs");
    tests.compile_fail("tests/ui/derive_fail_patchable_skip_with_unknown_parameter.rs");
    tests.compile_fail("tests/ui/derive_fail_patchable_name_value_parameter.rs");
}
