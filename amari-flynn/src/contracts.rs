//! Probabilistic contract traits and types

/// Trait for probabilistic contracts
///
/// Contracts that can be verified statistically
pub trait ProbabilisticContract {
    /// Verify the contract holds with given confidence
    fn verify(&self, confidence: f64) -> VerificationResult;
}

/// Trait for statistical properties
pub trait StatisticalProperty {
    /// The type of value this property applies to
    type Value;

    /// Check if the property holds for a given value
    fn holds(&self, value: &Self::Value) -> bool;
}

/// Result of verification
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationResult {
    /// Contract verified to hold
    Verified,
    /// Contract violated
    Violated,
    /// Inconclusive (insufficient evidence)
    Inconclusive,
}

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::String;

/// Represents a rare but possible event
///
/// Rare events (P << 1) are tracked separately from impossible events (P = 0)
#[cfg(feature = "alloc")]
#[derive(Clone, Debug)]
pub struct RareEvent<T> {
    probability: f64,
    description: String,
    _phantom: core::marker::PhantomData<T>,
}

#[cfg(feature = "alloc")]
impl<T> RareEvent<T> {
    /// Create a rare event with given probability and description
    pub fn new(probability: f64, description: impl Into<String>) -> Self {
        assert!(
            (0.0..1.0).contains(&probability),
            "Rare event probability must be in (0, 1)"
        );
        Self {
            probability,
            description: description.into(),
            _phantom: core::marker::PhantomData,
        }
    }

    /// Get the probability of this rare event
    pub fn probability(&self) -> f64 {
        self.probability
    }

    /// Get the description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Check if this event is rare (P < threshold)
    pub fn is_rare(&self, threshold: f64) -> bool {
        self.probability < threshold
    }
}

/// Event verification status
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventVerification {
    /// Event is impossible (P = 0)
    Impossible,
    /// Event is rare but possible (0 < P << 1)
    Rare,
    /// Event is probable (P >= threshold)
    Probable,
}

impl EventVerification {
    /// Classify event based on probability
    pub fn classify(probability: f64, rare_threshold: f64) -> Self {
        if probability == 0.0 {
            EventVerification::Impossible
        } else if probability < rare_threshold {
            EventVerification::Rare
        } else {
            EventVerification::Probable
        }
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;

    #[test]
    fn test_rare_event() {
        let event = RareEvent::<()>::new(0.001, "critical_hit");
        assert_eq!(event.probability(), 0.001);
        assert!(event.is_rare(0.01));
        assert!(!event.is_rare(0.0001));
    }

    #[test]
    fn test_event_classification() {
        assert_eq!(
            EventVerification::classify(0.0, 0.01),
            EventVerification::Impossible
        );
        assert_eq!(
            EventVerification::classify(0.005, 0.01),
            EventVerification::Rare
        );
        assert_eq!(
            EventVerification::classify(0.5, 0.01),
            EventVerification::Probable
        );
    }
}
