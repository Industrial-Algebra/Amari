//! Integration tests for amari-flynn procedural macros.
//!
//! Tests zero-parameter, single-parameter, and multi-parameter functions
//! with all three macros: prob_requires, prob_ensures, ensures_expected.

#![allow(unused_variables)]

use amari_flynn::contracts::VerificationResult;
use amari_flynn::ensures_expected;
use amari_flynn::prob_ensures;
use amari_flynn::prob_requires;

// ==================== prob_requires ====================

#[prob_requires(x > 0.0, 0.95)]
fn sqrt_positive(x: f64) -> f64 {
    if x > 0.0 {
        x.sqrt()
    } else {
        0.0
    }
}

#[prob_requires(x > 0.0 && y > 0.0, 0.9)]
fn product_positive(x: f64, y: f64) -> f64 {
    x * y
}

#[prob_requires(x > 0.0 && y > 0.0 && z > 0.0, 0.85)]
fn triple_product(x: f64, y: f64, z: f64) -> f64 {
    x * y * z
}

#[test]
fn test_prob_requires_single_param() {
    // Verify the function compiles and works
    assert!((sqrt_positive(4.0) - 2.0).abs() < 1e-10);
}

#[test]
fn test_prob_requires_single_param_verification_helper() {
    let result = verify_sqrt_positive_precondition(
        || rand::random::<f64>().abs() + 0.001, // always positive
        1000,
    );
    assert!(matches!(
        result,
        VerificationResult::Verified | VerificationResult::Inconclusive
    ));
}

#[test]
fn test_prob_requires_two_params() {
    assert!((product_positive(3.0, 4.0) - 12.0).abs() < 1e-10);
}

#[test]
fn test_prob_requires_two_params_verification_helper() {
    // Hold frame (audit F1 fix): the helper verifies P(condition) >= bound.
    // With always-positive inputs the condition always holds (P = 1.0),
    // so P >= 0.9 is Verified — historically this reported Violated under
    // the inverted implementation, and an earlier test CODIFIED that
    // inversion as correct ("the verifier correctly reports Violated").
    let result = verify_product_positive_precondition(
        || {
            let x = rand::random::<f64>().abs() + 0.001;
            let y = rand::random::<f64>().abs() + 0.001;
            (x, y)
        },
        4000,
    );
    assert_eq!(result, VerificationResult::Verified);
}

#[test]
fn test_prob_requires_three_params() {
    assert!((triple_product(2.0, 3.0, 4.0) - 24.0).abs() < 1e-10);
}

#[test]
fn test_prob_requires_three_params_verification_helper() {
    // Hold frame (audit F1 fix): P(condition) ≈ 1.0 with bound 0.85 →
    // Verified (historically Violated under the inverted implementation).
    let result = verify_triple_product_precondition(
        || {
            let x = rand::random::<f64>().abs() + 0.001;
            let y = rand::random::<f64>().abs() + 0.001;
            let z = rand::random::<f64>().abs() + 0.001;
            (x, y, z)
        },
        4000,
    );
    assert_eq!(result, VerificationResult::Verified);
}

// ==================== prob_ensures ====================

#[prob_ensures(result >= 0.0, 0.99)]
fn abs_value(x: f64) -> f64 {
    x.abs()
}

#[prob_ensures(result >= 0.0, 0.99)]
fn sum_abs(x: f64, y: f64) -> f64 {
    x.abs() + y.abs()
}

#[prob_ensures(result >= 0.0, 0.99)]
fn zero_param_constant() -> f64 {
    42.0
}

#[test]
fn test_prob_ensures_zero_param() {
    assert!((zero_param_constant() - 42.0).abs() < 1e-10);
}

#[test]
fn test_prob_ensures_zero_param_verification_helper() {
    let result = verify_zero_param_constant_postcondition(|| (), 1000);
    assert!(matches!(
        result,
        VerificationResult::Verified | VerificationResult::Inconclusive
    ));
}

#[test]
fn test_prob_ensures_single_param() {
    assert!((abs_value(-3.0) - 3.0).abs() < 1e-10);
}

#[test]
fn test_prob_ensures_single_param_verification_helper() {
    let result = verify_abs_value_postcondition(|| rand::random::<f64>() * 200.0 - 100.0, 1000);
    assert!(matches!(
        result,
        VerificationResult::Verified | VerificationResult::Inconclusive
    ));
}

#[test]
fn test_prob_ensures_two_params() {
    assert!((sum_abs(-3.0, 4.0) - 7.0).abs() < 1e-10);
}

#[test]
fn test_prob_ensures_two_params_verification_helper() {
    let result = verify_sum_abs_postcondition(
        || {
            let x = rand::random::<f64>() * 200.0 - 100.0;
            let y = rand::random::<f64>() * 200.0 - 100.0;
            (x, y)
        },
        1000,
    );
    assert!(matches!(
        result,
        VerificationResult::Verified | VerificationResult::Inconclusive
    ));
}

// ==================== ensures_expected ====================

#[ensures_expected(result, 0.5, 0.15)]
fn biased_coin() -> f64 {
    if rand::random::<bool>() {
        1.0
    } else {
        0.0
    }
}

#[ensures_expected(result, 0.0, 0.5)]
fn symmetric_diff(x: f64, y: f64) -> f64 {
    x - y
}

#[test]
fn test_ensures_expected_zero_param() {
    // Just verify it compiles and runs
    let _ = biased_coin();
}

#[test]
fn test_ensures_expected_zero_param_verification_helper() {
    let result = verify_biased_coin_expected_value(|| (), 10000);
    assert!(result, "Expected value of biased coin should be ~0.5");
}

#[test]
fn test_ensures_expected_two_params() {
    assert!((symmetric_diff(3.0, 3.0)).abs() < 1e-10);
}

#[test]
fn test_ensures_expected_two_params_verification_helper() {
    // Generate symmetric inputs around 0 so E[x-y] ≈ 0
    let result = verify_symmetric_diff_expected_value(
        || {
            let x = rand::random::<f64>() - 0.5;
            let y = rand::random::<f64>() - 0.5;
            (x, y)
        },
        10000,
    );
    assert!(result, "Expected value of symmetric_diff should be ~0.0");
}
