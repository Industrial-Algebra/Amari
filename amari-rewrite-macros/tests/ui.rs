// SPDX-License-Identifier: MIT OR Apache-2.0

//! UI contracts for `derive(Rewritable)` and the checked construction
//! macros: diagnostics, hygiene, and renamed-crate support.

#[test]
fn rewritable_ui() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass_enum.rs");
    cases.pass("tests/ui/pass_struct_children.rs");
    cases.pass("tests/ui/pass_term_rule.rs");
    cases.pass("tests/ui/pass_relation.rs");
    cases.compile_fail("tests/ui/fail_union.rs");
    cases.compile_fail("tests/ui/fail_generic.rs");
    cases.compile_fail("tests/ui/fail_duplicate_child.rs");
    cases.compile_fail("tests/ui/fail_unknown_attribute.rs");
    cases.compile_fail("tests/ui/fail_collection_child.rs");
    cases.compile_fail("tests/ui/fail_term_variable_arguments.rs");
    cases.compile_fail("tests/ui/fail_term_literal.rs");
    cases.compile_fail("tests/ui/fail_rule_arrow.rs");
    cases.compile_fail("tests/ui/fail_rule_trailing.rs");
    cases.compile_fail("tests/ui/fail_relation_rule_arrow.rs");
    cases.compile_fail("tests/ui/fail_relation_reverse_arrow.rs");
    cases.compile_fail("tests/ui/fail_relation_missing_arrow.rs");
    cases.compile_fail("tests/ui/fail_relation_forward_variable.rs");
    cases.compile_fail("tests/ui/fail_relation_existential_scope.rs");
    cases.compile_fail("tests/ui/fail_relation_existential_absent.rs");
    cases.compile_fail("tests/ui/fail_relation_existential_incomplete.rs");
    cases.compile_fail("tests/ui/fail_relation_existential_dot.rs");
    cases.compile_fail("tests/ui/fail_relation_constraint_operator.rs");
    cases.compile_fail("tests/ui/fail_relation_constraint_operand.rs");
    cases.compile_fail("tests/ui/fail_relation_constraint_missing.rs");
    cases.compile_fail("tests/ui/fail_relation_constraint_comma.rs");
    cases.compile_fail("tests/ui/fail_relation_constraint_variable.rs");
}

/// Renamed-crate support: the fixture under `tests/renamed` depends on
/// `amari-rewrite` as `rewrite_lib`; proc-macro-crate must resolve the
/// alias. Compiled with the real cargo against path dependencies.
#[test]
fn renamed_crate_fixture_compiles() {
    use std::path::PathBuf;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest_dir.join("tests/renamed");
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../target"));

    let status = std::process::Command::new(env!("CARGO"))
        .args(["check", "--offline"])
        .current_dir(&fixture)
        .env("CARGO_TARGET_DIR", target_dir)
        .status()
        .expect("cargo check for renamed fixture should launch");
    assert!(status.success(), "renamed-crate fixture must compile");
}
