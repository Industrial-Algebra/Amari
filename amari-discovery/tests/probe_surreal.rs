// SPDX-License-Identifier: MIT OR Apache-2.0

//! Exact rational-surreal and rational-surcomplex probe parity and limits.

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    DecimalRational, DecimalSurcomplex, DiscoveryError, ProbeEngine, ProbeEngineLimits,
    ProbeExecution, RationalSurcomplexDivisionOutput, RationalSurcomplexDivisionRequest,
    RationalSurrealArithmeticOutput, RationalSurrealArithmeticRequest,
};
use amari_surcomplex::RationalSurcomplex;
use amari_surreal::RationalSurreal;
use serde_json::json;

const RATIONAL_ARITHMETIC: &str = "amari-probe:surreal:rational-arithmetic:v1";
const SURCOMPLEX_DIVISION: &str = "amari-probe:surcomplex:rational-division:v1";

fn rational(numerator: &str, denominator: &str) -> DecimalRational {
    DecimalRational {
        numerator: numerator.to_owned(),
        denominator: denominator.to_owned(),
    }
}

fn request() -> RationalSurrealArithmeticRequest {
    RationalSurrealArithmeticRequest {
        lhs: rational("1", "3"),
        rhs: rational("2", "5"),
    }
}

fn execute(engine: &ProbeEngine, request: RationalSurrealArithmeticRequest) -> ProbeExecution {
    engine
        .execute(
            &RATIONAL_ARITHMETIC.parse().unwrap(),
            &serde_json::to_value(request).unwrap(),
        )
        .unwrap()
}

fn direct(numerator: i128, denominator: i128) -> RationalSurreal {
    RationalSurreal::from_ratio(numerator, denominator).unwrap()
}

fn output(value: &RationalSurreal) -> DecimalRational {
    DecimalRational {
        numerator: value.numer().to_string(),
        denominator: value.denom().to_string(),
    }
}

fn surcomplex(real: DecimalRational, imaginary: DecimalRational) -> DecimalSurcomplex {
    DecimalSurcomplex { real, imaginary }
}

fn direct_surcomplex(value: &DecimalSurcomplex) -> RationalSurcomplex {
    RationalSurcomplex::from_parts(
        RationalSurreal::from_ratio(
            value.real.numerator.parse::<i128>().unwrap(),
            value.real.denominator.parse::<i128>().unwrap(),
        )
        .unwrap(),
        RationalSurreal::from_ratio(
            value.imaginary.numerator.parse::<i128>().unwrap(),
            value.imaginary.denominator.parse::<i128>().unwrap(),
        )
        .unwrap(),
    )
}

fn surcomplex_output(value: &RationalSurcomplex) -> DecimalSurcomplex {
    surcomplex(output(value.real()), output(value.imag()))
}

#[test]
fn rational_arithmetic_matches_exact_rational_surreal_api() {
    let lhs = direct(1, 3);
    let rhs = direct(2, 5);
    let expected = RationalSurrealArithmeticOutput {
        sum: output(&(lhs.clone() + rhs.clone())),
        difference: output(&(lhs.clone() - rhs.clone())),
        product: output(&(lhs.clone() * rhs.clone())),
        quotient: output(&lhs.checked_div(&rhs).unwrap()),
    };
    let engine = ProbeEngine::new().unwrap();
    let first = execute(&engine, request());
    let second = execute(&engine, request());
    let actual: RationalSurrealArithmeticOutput =
        serde_json::from_value(first.output.clone()).unwrap();

    assert_eq!(first, second);
    assert_eq!(actual, expected);
    assert_eq!(actual.sum, rational("11", "15"));
    assert_eq!(actual.difference, rational("-1", "15"));
    assert_eq!(actual.product, rational("2", "15"));
    assert_eq!(actual.quotient, rational("5", "6"));
}

#[test]
fn decimal_i128_boundaries_parse_and_normalize_exactly() {
    let execution = execute(
        &ProbeEngine::new().unwrap(),
        RationalSurrealArithmeticRequest {
            lhs: rational("-170141183460469231731687303715884105728", "1"),
            rhs: rational("1", "170141183460469231731687303715884105727"),
        },
    );
    let actual: RationalSurrealArithmeticOutput = serde_json::from_value(execution.output).unwrap();

    assert_eq!(
        actual.product,
        rational(
            "-170141183460469231731687303715884105728",
            "170141183460469231731687303715884105727"
        )
    );
}

#[test]
fn decimal_length_and_i128_overflow_are_rejected() {
    for (input, expected) in [
        (
            json!({
                "lhs": { "numerator": "00000000000000000000000000000000000000000", "denominator": "1" },
                "rhs": { "numerator": "1", "denominator": "1" }
            }),
            "length",
        ),
        (
            json!({
                "lhs": { "numerator": "170141183460469231731687303715884105728", "denominator": "1" },
                "rhs": { "numerator": "1", "denominator": "1" }
            }),
            "i128",
        ),
    ] {
        assert!(matches!(
            ProbeEngine::new()
                .unwrap()
                .execute(&RATIONAL_ARITHMETIC.parse().unwrap(), &input),
            Err(DiscoveryError::InvalidInput(message)) if message.contains(expected)
        ));
    }
}

#[test]
fn zero_denominators_and_zero_divisors_are_rejected() {
    for (input, expected) in [
        (
            json!({
                "lhs": { "numerator": "1", "denominator": "0" },
                "rhs": { "numerator": "1", "denominator": "1" }
            }),
            "denominator",
        ),
        (
            json!({
                "lhs": { "numerator": "1", "denominator": "1" },
                "rhs": { "numerator": "0", "denominator": "7" }
            }),
            "division by zero",
        ),
    ] {
        assert!(matches!(
            ProbeEngine::new()
                .unwrap()
                .execute(&RATIONAL_ARITHMETIC.parse().unwrap(), &input),
            Err(DiscoveryError::InvalidInput(message)) if message.contains(expected)
        ));
    }
}

#[test]
fn malformed_decimal_and_unknown_fields_are_rejected() {
    for input in [
        json!({
            "lhs": { "numerator": " 1", "denominator": "2" },
            "rhs": { "numerator": "1", "denominator": "3" }
        }),
        json!({
            "lhs": { "numerator": "1", "denominator": "2", "secret": "no" },
            "rhs": { "numerator": "1", "denominator": "3" }
        }),
    ] {
        assert!(matches!(
            ProbeEngine::new()
                .unwrap()
                .execute(&RATIONAL_ARITHMETIC.parse().unwrap(), &input),
            Err(DiscoveryError::InvalidInput(_))
        ));
    }
}

#[test]
fn rational_arithmetic_cooperative_limits_are_enforced() {
    let input = serde_json::to_value(request()).unwrap();
    for (limits, expected) in [
        (
            ProbeEngineLimits {
                max_input_bytes: 8,
                ..ProbeEngineLimits::default()
            },
            "input bytes",
        ),
        (
            ProbeEngineLimits {
                max_operations: 5,
                ..ProbeEngineLimits::default()
            },
            "operations",
        ),
        (
            ProbeEngineLimits {
                max_nodes: 5,
                ..ProbeEngineLimits::default()
            },
            "nodes",
        ),
        (
            ProbeEngineLimits {
                max_iterations: 3,
                ..ProbeEngineLimits::default()
            },
            "iterations",
        ),
        (
            ProbeEngineLimits {
                max_output_bytes: 1,
                ..ProbeEngineLimits::default()
            },
            "output bytes",
        ),
    ] {
        let error = ProbeEngine::with_limits(limits)
            .unwrap()
            .execute(&RATIONAL_ARITHMETIC.parse().unwrap(), &input)
            .unwrap_err();
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
            "unexpected error for {expected}: {error}"
        );
    }
}

#[test]
fn surcomplex_division_matches_exact_api_and_known_reciprocal() {
    let request = RationalSurcomplexDivisionRequest {
        dividend: surcomplex(rational("1", "1"), rational("0", "1")),
        divisor: surcomplex(rational("1", "1"), rational("1", "2")),
    };
    let expected = direct_surcomplex(&request.dividend)
        .checked_div(&direct_surcomplex(&request.divisor))
        .unwrap();
    let input = serde_json::to_value(&request).unwrap();
    let engine = ProbeEngine::new().unwrap();
    let first = engine
        .execute(&SURCOMPLEX_DIVISION.parse().unwrap(), &input)
        .unwrap();
    let second = engine
        .execute(&SURCOMPLEX_DIVISION.parse().unwrap(), &input)
        .unwrap();
    let actual: RationalSurcomplexDivisionOutput =
        serde_json::from_value(first.output.clone()).unwrap();

    assert_eq!(first, second);
    assert_eq!(actual.quotient, surcomplex_output(&expected));
    assert_eq!(
        actual.quotient,
        surcomplex(rational("4", "5"), rational("-2", "5"))
    );
}

#[test]
fn surcomplex_zero_divisor_is_rejected() {
    let input = json!({
        "dividend": {
            "real": { "numerator": "1", "denominator": "1" },
            "imaginary": { "numerator": "0", "denominator": "1" }
        },
        "divisor": {
            "real": { "numerator": "0", "denominator": "1" },
            "imaginary": { "numerator": "0", "denominator": "1" }
        }
    });

    assert!(matches!(
        ProbeEngine::new()
            .unwrap()
            .execute(&SURCOMPLEX_DIVISION.parse().unwrap(), &input),
        Err(DiscoveryError::InvalidInput(message)) if message.contains("division by zero")
    ));
}

#[test]
fn surcomplex_components_reuse_bounded_decimal_validation() {
    for (input, expected) in [
        (
            json!({
                "dividend": {
                    "real": { "numerator": "00000000000000000000000000000000000000000", "denominator": "1" },
                    "imaginary": { "numerator": "0", "denominator": "1" }
                },
                "divisor": {
                    "real": { "numerator": "1", "denominator": "1" },
                    "imaginary": { "numerator": "0", "denominator": "1" }
                }
            }),
            "length",
        ),
        (
            json!({
                "dividend": {
                    "real": { "numerator": "1", "denominator": "0" },
                    "imaginary": { "numerator": "0", "denominator": "1" }
                },
                "divisor": {
                    "real": { "numerator": "1", "denominator": "1" },
                    "imaginary": { "numerator": "0", "denominator": "1" }
                }
            }),
            "denominator",
        ),
        (
            json!({
                "dividend": {
                    "real": { "numerator": "170141183460469231731687303715884105728", "denominator": "1" },
                    "imaginary": { "numerator": "0", "denominator": "1" }
                },
                "divisor": {
                    "real": { "numerator": "1", "denominator": "1" },
                    "imaginary": { "numerator": "0", "denominator": "1" }
                }
            }),
            "i128",
        ),
    ] {
        assert!(matches!(
            ProbeEngine::new()
                .unwrap()
                .execute(&SURCOMPLEX_DIVISION.parse().unwrap(), &input),
            Err(DiscoveryError::InvalidInput(message)) if message.contains(expected)
        ));
    }
}

#[test]
fn surcomplex_division_cooperative_limits_are_enforced() {
    let input = serde_json::to_value(RationalSurcomplexDivisionRequest {
        dividend: surcomplex(rational("1", "1"), rational("0", "1")),
        divisor: surcomplex(rational("1", "1"), rational("1", "2")),
    })
    .unwrap();
    for (limits, expected) in [
        (
            ProbeEngineLimits {
                max_input_bytes: 8,
                ..ProbeEngineLimits::default()
            },
            "input bytes",
        ),
        (
            ProbeEngineLimits {
                max_operations: 11,
                ..ProbeEngineLimits::default()
            },
            "operations",
        ),
        (
            ProbeEngineLimits {
                max_nodes: 6,
                ..ProbeEngineLimits::default()
            },
            "nodes",
        ),
        (
            ProbeEngineLimits {
                max_iterations: 7,
                ..ProbeEngineLimits::default()
            },
            "iterations",
        ),
        (
            ProbeEngineLimits {
                max_output_bytes: 1,
                ..ProbeEngineLimits::default()
            },
            "output bytes",
        ),
    ] {
        let error = ProbeEngine::with_limits(limits)
            .unwrap()
            .execute(&SURCOMPLEX_DIVISION.parse().unwrap(), &input)
            .unwrap_err();
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
            "unexpected error for {expected}: {error}"
        );
    }
}
