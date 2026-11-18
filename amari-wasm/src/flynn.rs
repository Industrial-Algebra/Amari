//! WASM bindings for probabilistic contracts and formal verification
//!
//! Provides WebAssembly bindings for:
//! - SMT-LIB2 proof obligation generation and export
//! - Monte Carlo statistical verification
//! - Probabilistic value tracking
//! - Rare event classification

use amari_flynn::backend::monte_carlo::MonteCarloVerifier;
use amari_flynn::backend::smt::{self, ObligationKind, SmtProofObligation, SmtSort};
use amari_flynn::contracts::{EventVerification, VerificationResult};
use amari_flynn::prob::Prob;
use wasm_bindgen::prelude::*;

// ==================== SmtProofObligation ====================

/// WASM wrapper for SMT-LIB2 proof obligation generation
///
/// Build proof obligations and export them as SMT-LIB2 strings
/// for external solvers (Z3, CVC5, etc.) or verify statistically
/// via Monte Carlo sampling.
#[wasm_bindgen]
pub struct WasmSmtProofObligation {
    inner: SmtProofObligation,
    /// Probability captured at construction (pre/post kinds): the hold
    /// probability the Monte Carlo bridge simulates.
    probability: f64,
}

#[wasm_bindgen]
impl WasmSmtProofObligation {
    /// Create a new proof obligation
    ///
    /// # Arguments
    /// * `name` - Obligation name (used in SMT-LIB2 comments)
    /// * `description` - Human-readable description
    /// * `kind` - One of: "precondition", "postcondition", "expected_value", "concentration"
    /// * `param1` - First parameter (probability for pre/post, expected value for EV, samples for concentration)
    /// * `param2` - Second parameter (unused for pre/post, epsilon for EV and concentration)
    #[wasm_bindgen(constructor)]
    pub fn new(
        name: &str,
        description: &str,
        kind: &str,
        param1: f64,
        param2: f64,
    ) -> Result<WasmSmtProofObligation, JsValue> {
        let obligation_kind = match kind {
            "precondition" => ObligationKind::PreconditionBound {
                probability: param1,
            },
            "postcondition" => ObligationKind::PostconditionBound {
                probability: param1,
            },
            "expected_value" => ObligationKind::ExpectedValue {
                expected: param1,
                epsilon: param2,
            },
            "concentration" => ObligationKind::ConcentrationBound {
                samples: param1 as usize,
                epsilon: param2,
            },
            _ => {
                return Err(JsValue::from_str(
                    "Invalid obligation kind. Use: precondition, postcondition, expected_value, concentration",
                ))
            }
        };

        Ok(Self {
            inner: SmtProofObligation::new(name, description, obligation_kind),
            probability: param1,
        })
    }

    /// Add a variable declaration to the obligation
    ///
    /// # Arguments
    /// * `name` - Variable name
    /// * `sort` - Variable type: "Real", "Int", or "Bool"
    #[wasm_bindgen(js_name = addVariable)]
    pub fn add_variable(&mut self, name: &str, sort: &str) -> Result<(), JsValue> {
        let smt_sort = match sort {
            "Real" => SmtSort::Real,
            "Int" => SmtSort::Int,
            "Bool" => SmtSort::Bool,
            _ => return Err(JsValue::from_str("Invalid sort. Use: Real, Int, Bool")),
        };
        self.inner.add_variable(name, smt_sort);
        Ok(())
    }

    /// Add an assertion to the obligation
    ///
    /// # Arguments
    /// * `expr` - SMT-LIB2 s-expression (e.g., "(> x 0.0)")
    /// * `comment` - Optional comment (pass empty string for none)
    #[wasm_bindgen(js_name = addAssertion)]
    pub fn add_assertion(&mut self, expr: &str, comment: &str) {
        let comment_opt = if comment.is_empty() {
            None
        } else {
            Some(comment.to_string())
        };
        self.inner.add_assertion(expr, comment_opt);
    }

    /// Generate SMT-LIB2 output string
    ///
    /// Returns a complete SMT-LIB2 theory ready for Z3, CVC5, or other solvers.
    /// If `unsat`, the property holds (goal was negated for refutation).
    #[wasm_bindgen(js_name = toSmtlib2)]
    pub fn to_smtlib2(&self) -> String {
        self.inner.to_smtlib2()
    }

    /// Verify this obligation statistically using Monte Carlo sampling
    ///
    /// Simulates the obligation's hold event (true with the probability
    /// captured at construction) and checks P(hold) >= that probability.
    /// Returns "Verified", "Violated", or "Inconclusive"
    #[wasm_bindgen(js_name = verifyWithMonteCarlo)]
    pub fn verify_with_monte_carlo(&self, samples: usize) -> String {
        let hold_probability = self.probability;
        match self
            .inner
            .verify_with_monte_carlo(|| fastrand::f64() < hold_probability, samples)
        {
            VerificationResult::Verified => "Verified".to_string(),
            VerificationResult::Violated => "Violated".to_string(),
            VerificationResult::Inconclusive => "Inconclusive".to_string(),
        }
    }
}

// ==================== Convenience constructors ====================

/// Create a Hoeffding bound proof obligation
///
/// Generates an SMT-LIB2 theory verifying that
/// `2 * exp(-2 * n * epsilon^2) <= delta` holds.
#[wasm_bindgen(js_name = flynnHoeffdingObligation)]
pub fn flynn_hoeffding_obligation(
    name: &str,
    n: usize,
    epsilon: f64,
    delta: f64,
) -> WasmSmtProofObligation {
    WasmSmtProofObligation {
        inner: smt::hoeffding_obligation(name, n, epsilon, delta),
        // ConcentrationBound kind: verify_with_monte_carlo derives the
        // bound from n/epsilon and never samples this probability.
        probability: 0.0,
    }
}

/// Create a precondition bound proof obligation
///
/// Generates an SMT-LIB2 theory for verifying P(condition) >= probability.
#[wasm_bindgen(js_name = flynnPreconditionObligation)]
pub fn flynn_precondition_obligation(
    name: &str,
    condition: &str,
    probability: f64,
) -> WasmSmtProofObligation {
    WasmSmtProofObligation {
        inner: smt::precondition_obligation(name, condition, probability),
        probability,
    }
}

/// Create a postcondition bound proof obligation
///
/// Generates an SMT-LIB2 theory for verifying P(condition) >= probability.
#[wasm_bindgen(js_name = flynnPostconditionObligation)]
pub fn flynn_postcondition_obligation(
    name: &str,
    condition: &str,
    probability: f64,
) -> WasmSmtProofObligation {
    WasmSmtProofObligation {
        inner: smt::postcondition_obligation(name, condition, probability),
        probability,
    }
}

/// Create an expected value proof obligation
///
/// Generates an SMT-LIB2 theory for verifying E[X] = expected +/- epsilon.
#[wasm_bindgen(js_name = flynnExpectedValueObligation)]
pub fn flynn_expected_value_obligation(
    name: &str,
    expected: f64,
    epsilon: f64,
    samples: usize,
) -> WasmSmtProofObligation {
    WasmSmtProofObligation {
        inner: smt::expected_value_obligation(name, expected, epsilon, samples),
        // ExpectedValue kind: verify_with_monte_carlo derives the bound
        // from samples/epsilon and never samples this probability.
        probability: 0.0,
    }
}

// ==================== MonteCarloVerifier ====================

/// WASM wrapper for Monte Carlo statistical verification
///
/// Provides Bernoulli-parameterized verification since closures cannot
/// cross the WASM boundary. For custom predicates, generate SMT-LIB2
/// output and use an external solver.
#[wasm_bindgen]
pub struct WasmMonteCarloVerifier {
    samples: usize,
}

#[wasm_bindgen]
impl WasmMonteCarloVerifier {
    /// Create a Monte Carlo verifier with the specified sample count
    #[wasm_bindgen(constructor)]
    pub fn new(samples: usize) -> Self {
        Self { samples }
    }

    /// Estimate the probability of a Bernoulli trial
    ///
    /// Runs `num_trials` independent trials where each succeeds with
    /// `success_probability`, and returns the estimated probability
    /// along with 95% confidence bounds as [estimate, lower, upper].
    #[wasm_bindgen(js_name = estimateProbability)]
    pub fn estimate_probability(&self, num_trials: usize, success_probability: f64) -> Vec<f64> {
        let successes = (0..num_trials)
            .filter(|_| fastrand::f64() < success_probability)
            .count();
        let estimate = successes as f64 / num_trials as f64;

        // 95% confidence interval using Hoeffding
        let epsilon = (2.0 * (1.0_f64 / 0.05).ln() / num_trials as f64).sqrt();
        let lower = (estimate - epsilon).max(0.0);
        let upper = (estimate + epsilon).min(1.0);

        vec![estimate, lower, upper]
    }

    /// Verify that P(success) >= bound using Monte Carlo sampling
    ///
    /// Runs Bernoulli trials with the given success probability and checks
    /// the hold frame: whether P(success) is at least the bound.
    /// Returns "Verified", "Violated", or "Inconclusive".
    #[wasm_bindgen(js_name = verifyProbabilityAtLeast)]
    pub fn verify_probability_at_least(&self, success_probability: f64, bound: f64) -> String {
        let verifier = MonteCarloVerifier::new(self.samples);
        let result =
            verifier.verify_probability_at_least(|| fastrand::f64() < success_probability, bound);
        match result {
            VerificationResult::Verified => "Verified".to_string(),
            VerificationResult::Violated => "Violated".to_string(),
            VerificationResult::Inconclusive => "Inconclusive".to_string(),
        }
    }
}

// ==================== Prob ====================

/// WASM wrapper for probabilistic values
///
/// Wraps an f64 value with associated probability metadata.
/// Supports mapping, composition, and probability tracking.
#[wasm_bindgen]
pub struct WasmProb {
    inner: Prob<f64>,
}

#[wasm_bindgen]
impl WasmProb {
    /// Create a certain value (probability = 1.0)
    #[wasm_bindgen(constructor)]
    pub fn new(value: f64) -> Self {
        Self {
            inner: Prob::new(value),
        }
    }

    /// Create a value with specified probability
    ///
    /// Probability must be in [0, 1].
    #[wasm_bindgen(js_name = withProbability)]
    pub fn with_probability(probability: f64, value: f64) -> Result<WasmProb, JsValue> {
        if !(0.0..=1.0).contains(&probability) {
            return Err(JsValue::from_str("Probability must be in [0, 1]"));
        }
        Ok(Self {
            inner: Prob::with_probability(probability, value),
        })
    }

    /// Get the probability associated with this value
    pub fn probability(&self) -> f64 {
        self.inner.probability()
    }

    /// Get the wrapped value
    pub fn value(&self) -> f64 {
        *self.inner.inner()
    }

    /// Map the value by multiplying with a factor, preserving probability
    pub fn map(&self, factor: f64) -> WasmProb {
        let p = self.inner.probability();
        let v = *self.inner.inner();
        WasmProb {
            inner: Prob::with_probability(p, v * factor),
        }
    }

    /// Combine with another probabilistic value (independent)
    ///
    /// Multiplies probabilities and values together.
    #[wasm_bindgen(js_name = andThen)]
    pub fn and_then(&self, other_probability: f64, other_value: f64) -> Result<WasmProb, JsValue> {
        if !(0.0..=1.0).contains(&other_probability) {
            return Err(JsValue::from_str("Probability must be in [0, 1]"));
        }
        let combined_prob = self.inner.probability() * other_probability;
        let combined_value = *self.inner.inner() * other_value;
        Ok(WasmProb {
            inner: Prob::with_probability(combined_prob, combined_value),
        })
    }

    /// Sample this probabilistic value
    ///
    /// Returns the value with probability p, or NaN otherwise.
    pub fn sample(&self) -> f64 {
        if fastrand::f64() < self.inner.probability() {
            *self.inner.inner()
        } else {
            f64::NAN
        }
    }
}

// ==================== RareEvent ====================

/// WASM wrapper for rare event tracking
///
/// Represents events with low but non-zero probability,
/// distinguished from impossible events (P=0).
#[wasm_bindgen]
pub struct WasmRareEvent {
    probability: f64,
    description: String,
}

#[wasm_bindgen]
impl WasmRareEvent {
    /// Create a rare event with given probability and description
    ///
    /// Probability must be in (0, 1).
    #[wasm_bindgen(constructor)]
    pub fn new(probability: f64, description: &str) -> Result<WasmRareEvent, JsValue> {
        if probability <= 0.0 || probability >= 1.0 {
            return Err(JsValue::from_str(
                "Rare event probability must be in (0, 1)",
            ));
        }
        Ok(Self {
            probability,
            description: description.to_string(),
        })
    }

    /// Get the probability of this rare event
    pub fn probability(&self) -> f64 {
        self.probability
    }

    /// Get the event description
    pub fn description(&self) -> String {
        self.description.clone()
    }

    /// Check if this event is rare (probability < threshold)
    #[wasm_bindgen(js_name = isRare)]
    pub fn is_rare(&self, threshold: f64) -> bool {
        self.probability < threshold
    }

    /// Classify event based on probability with the given rare threshold
    ///
    /// Returns "Impossible" (P=0), "Rare" (P < threshold), or "Probable" (P >= threshold)
    pub fn classify(&self, rare_threshold: f64) -> String {
        match EventVerification::classify(self.probability, rare_threshold) {
            EventVerification::Impossible => "Impossible".to_string(),
            EventVerification::Rare => "Rare".to_string(),
            EventVerification::Probable => "Probable".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smt_obligation_constructor() {
        let ob = WasmSmtProofObligation::new("test", "Test obligation", "precondition", 0.95, 0.0);
        assert!(ob.is_ok());
    }

    #[test]
    fn test_smt_to_smtlib2() {
        let ob = flynn_hoeffding_obligation("test_hoeffding", 1000, 0.1, 0.05);
        let smt = ob.to_smtlib2();
        assert!(smt.contains("(set-logic QF_NRA)"));
        assert!(smt.contains("(check-sat)"));
        assert!(smt.contains("test_hoeffding"));
    }

    #[test]
    fn test_smt_add_variable_and_assertion() {
        let mut ob =
            WasmSmtProofObligation::new("custom", "Custom test", "concentration", 500.0, 0.05)
                .unwrap();
        assert!(ob.add_variable("alpha", "Real").is_ok());
        assert!(ob.add_variable("flag", "Bool").is_ok());
        ob.add_assertion("(> alpha 0.0)", "alpha is positive");
        let smt = ob.to_smtlib2();
        assert!(smt.contains("(declare-const alpha Real)"));
        assert!(smt.contains("(declare-const flag Bool)"));
        assert!(smt.contains("; alpha is positive"));
    }

    #[test]
    fn test_smt_verify_monte_carlo() {
        let ob = flynn_hoeffding_obligation("mc_test", 10000, 0.1, 0.05);
        let result = ob.verify_with_monte_carlo(10000);
        assert_eq!(result, "Verified");
    }

    #[test]
    fn test_convenience_constructors() {
        let pre = flynn_precondition_obligation("pre", "x > 0", 0.95);
        assert!(pre.to_smtlib2().contains("Precondition"));

        let post = flynn_postcondition_obligation("post", "result >= 0", 0.99);
        assert!(post.to_smtlib2().contains("Postcondition"));

        let ev = flynn_expected_value_obligation("ev", 5.0, 0.1, 10000);
        assert!(ev.to_smtlib2().contains("mu"));
    }

    #[test]
    fn test_monte_carlo_verifier() {
        let verifier = WasmMonteCarloVerifier::new(10000);
        let est = verifier.estimate_probability(10000, 0.5);
        assert_eq!(est.len(), 3);
        assert!(est[0] > 0.4 && est[0] < 0.6);
        assert!(est[1] <= est[0]);
        assert!(est[2] >= est[0]);
    }

    #[test]
    fn test_prob_new() {
        let p = WasmProb::new(42.0);
        assert_eq!(p.probability(), 1.0);
        assert_eq!(p.value(), 42.0);
    }

    #[test]
    fn test_prob_with_probability() {
        let p = WasmProb::with_probability(0.5, 10.0).unwrap();
        assert_eq!(p.probability(), 0.5);
        assert_eq!(p.value(), 10.0);
    }

    #[test]
    fn test_prob_map() {
        let p = WasmProb::with_probability(0.3, 10.0).unwrap();
        let q = p.map(2.0);
        assert_eq!(q.value(), 20.0);
        assert_eq!(q.probability(), 0.3);
    }

    #[test]
    fn test_prob_and_then() {
        let p = WasmProb::with_probability(0.5, 10.0).unwrap();
        let q = p.and_then(0.4, 3.0).unwrap();
        assert!((q.probability() - 0.2).abs() < 1e-10);
        assert!((q.value() - 30.0).abs() < 1e-10);
    }

    #[test]
    fn test_rare_event() {
        let event = WasmRareEvent::new(0.001, "critical_hit").unwrap();
        assert_eq!(event.probability(), 0.001);
        assert_eq!(event.description(), "critical_hit");
        assert!(event.is_rare(0.01));
        assert!(!event.is_rare(0.0001));
    }

    #[test]
    fn test_rare_event_classify() {
        let event = WasmRareEvent::new(0.005, "rare_event").unwrap();
        assert_eq!(event.classify(0.01), "Rare");
        assert_eq!(event.classify(0.001), "Probable");
    }

    // Note: Error-path tests (invalid kind, invalid sort, invalid probability)
    // are skipped in native tests because JsValue::from_str() panics outside WASM.
    // These would be tested with wasm_bindgen_test in a WASM environment.
}
