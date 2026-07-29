// SPDX-License-Identifier: MIT OR Apache-2.0

#[test]
fn wire_contract_ui() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass_simple.rs");
    cases.pass("tests/ui/pass_nested.rs");
    cases.pass("tests/ui/pass_tagged_enum.rs");
    cases.pass("tests/ui/pass_unit_enum.rs");
    cases.pass("tests/ui/pass_shapes.rs");
    cases.compile_fail("tests/ui/fail_missing_id.rs");
    cases.compile_fail("tests/ui/fail_invalid_role.rs");
    cases.compile_fail("tests/ui/fail_tuple_struct.rs");
    cases.compile_fail("tests/ui/fail_unit_struct.rs");
    cases.compile_fail("tests/ui/fail_unsupported_serde.rs");
    cases.compile_fail("tests/ui/fail_generic.rs");
    cases.compile_fail("tests/ui/fail_malformed_id.rs");
}
