// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded exact rational-surreal and rational-surcomplex probe adapters.

use serde::{Deserialize, Serialize};

#[cfg(feature = "standard-probes")]
use amari_surcomplex::RationalSurcomplex;
#[cfg(feature = "standard-probes")]
use amari_surreal::RationalSurreal;
#[cfg(feature = "standard-probes")]
use serde_json::Value;

#[cfg(feature = "standard-probes")]
use super::registry::{AdapterOutput, AdapterRegistration, EffectiveProbeLimits};
#[cfg(feature = "standard-probes")]
use crate::{DiscoveryError, DiscoveryResult, ProbeLimits, ResourceObservations, SideEffectPolicy};

#[cfg(feature = "standard-probes")]
const MAX_DECIMAL_LENGTH: usize = 40;
#[cfg(feature = "standard-probes")]
const RATIONAL_ARITHMETIC_OPERATIONS: u64 = 6;
#[cfg(feature = "standard-probes")]
const RATIONAL_ARITHMETIC_NODES: u64 = 6;
#[cfg(feature = "standard-probes")]
const SURCOMPLEX_DIVISION_OPERATIONS: u64 = 12;
#[cfg(feature = "standard-probes")]
const SURCOMPLEX_DIVISION_NODES: u64 = 7;

/// Exact rational encoded as bounded base-ten numerator and denominator strings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecimalRational {
    /// Signed base-ten numerator.
    pub numerator: String,
    /// Signed nonzero base-ten denominator.
    pub denominator: String,
}

/// Typed input for exact arithmetic over two rational surreal values.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/surreal-rational-arithmetic/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        decimal_length_limit = "numerator and denominator strings each contain 1 to 40 characters",
        decimal_strings_are_i128 = "decimal strings are signed base-ten i128 values",
        nonzero_denominators = "each rational denominator is nonzero",
        nonzero_rhs = "the right operand is nonzero so exact division is defined"
    ),
    example(
        label = "halves_and_thirds",
        value = "{\"lhs\":{\"numerator\":\"1\",\"denominator\":\"2\"},\"rhs\":{\"numerator\":\"1\",\"denominator\":\"3\"}}"
    )
)]
pub struct RationalSurrealArithmeticRequest {
    /// Left operand.
    pub lhs: DecimalRational,
    /// Right operand.
    pub rhs: DecimalRational,
}

/// Exact normalized results for the four rational field operations.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/surreal-rational-arithmetic/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        nonzero_denominators = "every result denominator is nonzero",
        normalized_exact_rationals = "every result is normalized by the exact rational-surreal implementation"
    ),
    example(
        label = "halves_and_thirds",
        value = "{\"sum\":{\"numerator\":\"5\",\"denominator\":\"6\"},\"difference\":{\"numerator\":\"1\",\"denominator\":\"6\"},\"product\":{\"numerator\":\"1\",\"denominator\":\"6\"},\"quotient\":{\"numerator\":\"3\",\"denominator\":\"2\"}}"
    )
)]
pub struct RationalSurrealArithmeticOutput {
    /// `lhs + rhs`.
    pub sum: DecimalRational,
    /// `lhs - rhs`.
    pub difference: DecimalRational,
    /// `lhs * rhs`.
    pub product: DecimalRational,
    /// `lhs / rhs`.
    pub quotient: DecimalRational,
}

/// Exact rational surcomplex value encoded as real and imaginary parts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecimalSurcomplex {
    /// Exact real component.
    pub real: DecimalRational,
    /// Exact imaginary component.
    pub imaginary: DecimalRational,
}

/// Typed input for exact rational-surcomplex division.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/surcomplex-rational-division/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        decimal_length_limit = "numerator and denominator strings each contain 1 to 40 characters",
        decimal_strings_are_i128 = "decimal strings are signed base-ten i128 values",
        nonzero_denominators = "each rational component denominator is nonzero",
        nonzero_divisor = "the divisor has a nonzero real or imaginary component"
    ),
    example(
        label = "identity_division",
        value = "{\"dividend\":{\"real\":{\"numerator\":\"1\",\"denominator\":\"1\"},\"imaginary\":{\"numerator\":\"0\",\"denominator\":\"1\"}},\"divisor\":{\"real\":{\"numerator\":\"1\",\"denominator\":\"1\"},\"imaginary\":{\"numerator\":\"0\",\"denominator\":\"1\"}}}"
    )
)]
pub struct RationalSurcomplexDivisionRequest {
    /// Rational-surcomplex dividend.
    pub dividend: DecimalSurcomplex,
    /// Nonzero rational-surcomplex divisor.
    pub divisor: DecimalSurcomplex,
}

/// Exact normalized result of rational-surcomplex division.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/surcomplex-rational-division/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        nonzero_denominators = "every quotient component denominator is nonzero",
        normalized_exact_rationals = "quotient components are normalized by the exact rational-surcomplex implementation"
    ),
    example(
        label = "identity_division",
        value = "{\"quotient\":{\"real\":{\"numerator\":\"1\",\"denominator\":\"1\"},\"imaginary\":{\"numerator\":\"0\",\"denominator\":\"1\"}}}"
    )
)]
pub struct RationalSurcomplexDivisionOutput {
    /// Exact quotient `dividend / divisor`.
    pub quotient: DecimalSurcomplex,
}

#[cfg(feature = "standard-probes")]
pub(super) fn rational_arithmetic_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:surreal:rational-arithmetic:v1".parse()?,
        capability_id: "amari:amari-surreal:rational:exact-arithmetic".parse()?,
        input_schema: "amari.discovery/probe/surreal-rational-arithmetic/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/surreal-rational-arithmetic/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 16_384,
            max_output_bytes: 16_384,
            max_operations: 10_000,
            timeout_millis: 1_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_rational_arithmetic,
    })
}

#[cfg(feature = "standard-probes")]
pub(super) fn surcomplex_division_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:surcomplex:rational-division:v1".parse()?,
        capability_id: "amari:amari-surcomplex:rational:exact-division".parse()?,
        input_schema: "amari.discovery/probe/surcomplex-rational-division/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/surcomplex-rational-division/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 16_384,
            max_output_bytes: 16_384,
            max_operations: 10_000,
            timeout_millis: 1_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_surcomplex_division,
    })
}

#[cfg(feature = "standard-probes")]
fn execute_rational_arithmetic(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: RationalSurrealArithmeticRequest = serde_json::from_value(input.clone()).map_err(
        |error| {
            DiscoveryError::InvalidInput(format!(
                "rational arithmetic request requires decimal numerator and denominator strings: {error}"
            ))
        },
    )?;
    let resources = validate_rational_arithmetic(&request, limits)?;
    let lhs = parse_rational(&request.lhs, "lhs")?;
    let rhs = parse_rational(&request.rhs, "rhs")?;
    let quotient = lhs.checked_div(&rhs).map_err(|_| {
        DiscoveryError::InvalidInput("rational arithmetic division by zero".to_owned())
    })?;

    Ok(AdapterOutput {
        resources,
        output: serde_json::to_value(RationalSurrealArithmeticOutput {
            sum: decimal_rational(&(lhs.clone() + rhs.clone())),
            difference: decimal_rational(&(lhs.clone() - rhs.clone())),
            product: decimal_rational(&(lhs * rhs)),
            quotient: decimal_rational(&quotient),
        })?,
    })
}

#[cfg(feature = "standard-probes")]
fn execute_surcomplex_division(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: RationalSurcomplexDivisionRequest = serde_json::from_value(input.clone())
        .map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "rational surcomplex division requires decimal component strings: {error}"
            ))
        })?;
    let resources = validate_surcomplex_division(&request, limits)?;
    let dividend = parse_surcomplex(&request.dividend, "dividend")?;
    let divisor = parse_surcomplex(&request.divisor, "divisor")?;
    let quotient = dividend.checked_div(&divisor).map_err(|_| {
        DiscoveryError::InvalidInput("rational surcomplex division by zero".to_owned())
    })?;

    Ok(AdapterOutput {
        resources,
        output: serde_json::to_value(RationalSurcomplexDivisionOutput {
            quotient: decimal_surcomplex(&quotient),
        })?,
    })
}

#[cfg(feature = "standard-probes")]
fn validate_rational_arithmetic(
    request: &RationalSurrealArithmeticRequest,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<ResourceObservations> {
    let iterations = [&request.lhs, &request.rhs]
        .into_iter()
        .try_fold(0_u64, |total, value| {
            total
                .checked_add(validated_decimal_length(value)?)
                .ok_or_else(|| {
                    DiscoveryError::LimitExceeded("rational decimal length overflow".to_owned())
                })
        })?;

    enforce(
        "operations",
        RATIONAL_ARITHMETIC_OPERATIONS,
        limits.max_operations,
    )?;
    enforce("nodes", RATIONAL_ARITHMETIC_NODES, limits.max_nodes)?;
    enforce("iterations", iterations, limits.max_iterations)?;

    Ok(ResourceObservations {
        operations: RATIONAL_ARITHMETIC_OPERATIONS,
        nodes: RATIONAL_ARITHMETIC_NODES,
        iterations,
        bytes: 0,
    })
}

#[cfg(feature = "standard-probes")]
fn validate_surcomplex_division(
    request: &RationalSurcomplexDivisionRequest,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<ResourceObservations> {
    let iterations = [
        &request.dividend.real,
        &request.dividend.imaginary,
        &request.divisor.real,
        &request.divisor.imaginary,
    ]
    .into_iter()
    .try_fold(0_u64, |total, value| {
        total
            .checked_add(validated_decimal_length(value)?)
            .ok_or_else(|| {
                DiscoveryError::LimitExceeded("surcomplex decimal length overflow".to_owned())
            })
    })?;

    enforce_surcomplex(
        "operations",
        SURCOMPLEX_DIVISION_OPERATIONS,
        limits.max_operations,
    )?;
    enforce_surcomplex("nodes", SURCOMPLEX_DIVISION_NODES, limits.max_nodes)?;
    enforce_surcomplex("iterations", iterations, limits.max_iterations)?;

    Ok(ResourceObservations {
        operations: SURCOMPLEX_DIVISION_OPERATIONS,
        nodes: SURCOMPLEX_DIVISION_NODES,
        iterations,
        bytes: 0,
    })
}

#[cfg(feature = "standard-probes")]
fn validated_decimal_length(value: &DecimalRational) -> DiscoveryResult<u64> {
    validate_decimal(&value.numerator, "numerator")?;
    validate_decimal(&value.denominator, "denominator")?;
    let length = value
        .numerator
        .len()
        .checked_add(value.denominator.len())
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("rational decimal length overflow".to_owned())
        })?;
    u64::try_from(length)
        .map_err(|_| DiscoveryError::LimitExceeded("rational decimal length overflow".to_owned()))
}

#[cfg(feature = "standard-probes")]
fn validate_decimal(value: &str, field: &str) -> DiscoveryResult<()> {
    if value.is_empty() || value.len() > MAX_DECIMAL_LENGTH {
        return Err(DiscoveryError::InvalidInput(format!(
            "rational {field} decimal length {} is outside 1..={MAX_DECIMAL_LENGTH}",
            value.len()
        )));
    }
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(DiscoveryError::InvalidInput(format!(
            "rational {field} must be a base-ten i128 string"
        )));
    }
    Ok(())
}

#[cfg(feature = "standard-probes")]
fn parse_rational(value: &DecimalRational, operand: &str) -> DiscoveryResult<RationalSurreal> {
    let numerator = parse_i128(&value.numerator, operand, "numerator")?;
    let denominator = parse_i128(&value.denominator, operand, "denominator")?;
    if denominator == 0 {
        return Err(DiscoveryError::InvalidInput(format!(
            "rational {operand} denominator must be nonzero"
        )));
    }
    RationalSurreal::from_ratio(numerator, denominator).map_err(|_| {
        DiscoveryError::InvalidInput(format!("rational {operand} denominator must be nonzero"))
    })
}

#[cfg(feature = "standard-probes")]
fn parse_surcomplex(
    value: &DecimalSurcomplex,
    operand: &str,
) -> DiscoveryResult<RationalSurcomplex> {
    Ok(RationalSurcomplex::from_parts(
        parse_rational(&value.real, &format!("{operand} real"))?,
        parse_rational(&value.imaginary, &format!("{operand} imaginary"))?,
    ))
}

#[cfg(feature = "standard-probes")]
fn parse_i128(value: &str, operand: &str, field: &str) -> DiscoveryResult<i128> {
    value.parse::<i128>().map_err(|_| {
        DiscoveryError::InvalidInput(format!(
            "rational {operand} {field} is outside the i128 range"
        ))
    })
}

#[cfg(feature = "standard-probes")]
fn decimal_rational(value: &RationalSurreal) -> DecimalRational {
    DecimalRational {
        numerator: value.numer().to_string(),
        denominator: value.denom().to_string(),
    }
}

#[cfg(feature = "standard-probes")]
fn decimal_surcomplex(value: &RationalSurcomplex) -> DecimalSurcomplex {
    DecimalSurcomplex {
        real: decimal_rational(value.real()),
        imaginary: decimal_rational(value.imag()),
    }
}

#[cfg(feature = "standard-probes")]
fn enforce(kind: &str, observed: u64, maximum: u64) -> DiscoveryResult<()> {
    if observed <= maximum {
        Ok(())
    } else {
        Err(DiscoveryError::LimitExceeded(format!(
            "rational arithmetic {kind} {observed} exceeds limit {maximum}"
        )))
    }
}

#[cfg(feature = "standard-probes")]
fn enforce_surcomplex(kind: &str, observed: u64, maximum: u64) -> DiscoveryResult<()> {
    if observed <= maximum {
        Ok(())
    } else {
        Err(DiscoveryError::LimitExceeded(format!(
            "rational surcomplex division {kind} {observed} exceeds limit {maximum}"
        )))
    }
}
