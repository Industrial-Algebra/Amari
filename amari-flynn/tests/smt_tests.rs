#![cfg(feature = "std")]

//! Integration tests for the SMT-LIB2 backend.

use amari_flynn::backend::smt::{
    expected_value_obligation, hoeffding_obligation, postcondition_obligation,
    precondition_obligation, ObligationKind, SmtProofObligation, SmtSort,
};
use amari_flynn::contracts::VerificationResult;

#[test]
fn test_hoeffding_obligation_roundtrip() {
    let ob = hoeffding_obligation("test_hoeffding", 1000, 0.1, 0.05);
    let smt = ob.to_smtlib2();

    // Should be well-formed SMT-LIB2
    assert!(smt.starts_with("; Proof obligation: test_hoeffding"));
    assert!(smt.contains("(set-logic QF_NRA)"));
    assert!(smt.contains("(declare-const n Real)"));
    assert!(smt.contains("(declare-const epsilon Real)"));
    assert!(smt.contains("(declare-const delta Real)"));
    assert!(smt.contains("(declare-const hoeffding_bound Real)"));
    assert!(smt.contains("(assert (= n 1000.0))"));
    assert!(smt.contains("(check-sat)"));
    assert!(smt.contains("(exit)"));
}

#[test]
fn test_precondition_obligation_structure() {
    let ob = precondition_obligation("pre_x_positive", "x > 0", 0.95);
    let smt = ob.to_smtlib2();

    assert!(smt.contains("Precondition"));
    assert!(smt.contains("x > 0"));
    assert!(smt.contains("0.95"));
    assert!(smt.contains("(declare-const p_condition Real)"));
    assert!(smt.contains("(declare-const bound Real)"));
    // Should have non-negativity and upper bound constraints
    assert!(smt.contains("(>= p_condition 0.0)"));
    assert!(smt.contains("(<= p_condition 1.0)"));
}

#[test]
fn test_postcondition_obligation_structure() {
    let ob = postcondition_obligation("post_result_nonneg", "result >= 0", 0.99);
    let smt = ob.to_smtlib2();

    assert!(smt.contains("Postcondition"));
    assert!(smt.contains("result >= 0"));
    assert!(smt.contains("0.99"));
}

#[test]
fn test_expected_value_obligation_structure() {
    let ob = expected_value_obligation("ev_mean_five", 5.0, 0.1, 10000);
    let smt = ob.to_smtlib2();

    assert!(smt.contains("(= mu 5)"));
    assert!(smt.contains("(= epsilon 0.1)"));
    assert!(smt.contains("(= n 10000.0)"));
    assert!(smt.contains("tail_prob"));
    assert!(smt.contains("exp"));
}

#[test]
fn test_custom_obligation() {
    let mut ob = SmtProofObligation::new(
        "custom",
        "Custom test obligation",
        ObligationKind::ConcentrationBound {
            samples: 500,
            epsilon: 0.05,
        },
    );
    ob.add_variable("alpha", SmtSort::Real);
    ob.add_variable("count", SmtSort::Int);
    ob.add_variable("flag", SmtSort::Bool);
    ob.add_assertion("(> alpha 0.0)", Some("alpha is positive".to_string()));
    ob.add_assertion("(= count 500)", None);

    let smt = ob.to_smtlib2();
    assert!(smt.contains("(declare-const alpha Real)"));
    assert!(smt.contains("(declare-const count Int)"));
    assert!(smt.contains("(declare-const flag Bool)"));
    assert!(smt.contains("; alpha is positive"));
    assert!(smt.contains("(assert (> alpha 0.0))"));
    assert!(smt.contains("(assert (= count 500))"));
}

#[test]
fn test_write_and_read_back() {
    let ob = hoeffding_obligation("file_roundtrip", 500, 0.05, 0.01);
    let dir = std::env::temp_dir();
    let path = dir.join("flynn_test_roundtrip.smt2");

    ob.write_to_file(&path).unwrap();
    let content = std::fs::read_to_string(&path).unwrap();

    assert!(content.contains("(set-logic QF_NRA)"));
    assert!(content.contains("file_roundtrip"));
    assert!(content.contains("(check-sat)"));

    // Clean up
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_monte_carlo_bridge_concentration() {
    // With n=10000 and epsilon=0.1, Hoeffding bound = 2*exp(-200) ≈ 0
    // This should easily verify
    let ob = hoeffding_obligation("mc_bridge", 10000, 0.1, 0.05);
    // Concentration obligations are self-checks (no system predicate).
    let result =
        ob.verify_with_monte_carlo(|| unreachable!("not sampled for concentration"), 10000);
    assert_eq!(result, VerificationResult::Verified);
}

#[test]
fn test_monte_carlo_bridge_expected_value() {
    // With n=10000 and epsilon=0.1, tail_prob is tiny -> Verified
    let ob = expected_value_obligation("mc_ev", 5.0, 0.1, 10000);
    let result =
        ob.verify_with_monte_carlo(|| unreachable!("not sampled for expected value"), 10000);
    assert_eq!(result, VerificationResult::Verified);
}

#[test]
fn test_obligation_kind_variants() {
    // Ensure all variants construct correctly and display
    let kinds = [
        ObligationKind::PreconditionBound { probability: 0.95 },
        ObligationKind::PostconditionBound { probability: 0.99 },
        ObligationKind::ExpectedValue {
            expected: 2.72,
            epsilon: 0.01,
        },
        ObligationKind::ConcentrationBound {
            samples: 1000,
            epsilon: 0.05,
        },
    ];
    for kind in &kinds {
        let display = format!("{}", kind);
        assert!(!display.is_empty());
    }
}
