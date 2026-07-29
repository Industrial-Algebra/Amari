// SPDX-License-Identifier: MIT OR Apache-2.0

#[test]
fn wire_contract_ui() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass_simple.rs");
    cases.compile_fail("tests/ui/fail_missing_id.rs");
}
