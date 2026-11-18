//! Probabilistic value wrapper
//!
//! The core `Prob<T>` type wraps values with runtime probability tracking.

use rand::{Rng, RngExt};

/// A value with associated probability
///
/// Wraps any value `T` with runtime metadata tracking its probability.
/// This is the foundation for probabilistic contracts.
///
/// # Examples
///
/// ```
/// use amari_flynn::prob::Prob;
///
/// let coin = Prob::with_probability(0.5, true);
/// assert_eq!(coin.probability(), 0.5);
/// assert_eq!(coin.into_inner(), true);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Prob<T> {
    value: T,
    probability: f64,
}

impl<T> Prob<T> {
    /// Create a probabilistic value with associated probability
    ///
    /// # Examples
    ///
    /// ```
    /// use amari_flynn::prob::Prob;
    ///
    /// let certain = Prob::new(42);
    /// assert_eq!(certain.probability(), 1.0);
    /// ```
    #[inline]
    pub fn new(value: T) -> Self {
        Self {
            value,
            probability: 1.0,
        }
    }

    /// Create a probabilistic value with specified probability
    ///
    /// # Panics
    ///
    /// Panics if probability is not in [0, 1]
    ///
    /// # Examples
    ///
    /// ```
    /// use amari_flynn::prob::Prob;
    ///
    /// let rare = Prob::with_probability(0.01, "miracle");
    /// assert_eq!(rare.probability(), 0.01);
    /// ```
    #[inline]
    pub fn with_probability(probability: f64, value: T) -> Self {
        assert!(
            (0.0..=1.0).contains(&probability),
            "Probability must be in [0, 1]"
        );
        Self { value, probability }
    }

    /// Get the probability associated with this value
    #[inline]
    pub fn probability(&self) -> f64 {
        self.probability
    }

    /// Extract the inner value
    #[inline]
    pub fn into_inner(self) -> T {
        self.value
    }

    /// Get a reference to the inner value
    #[inline]
    pub fn inner(&self) -> &T {
        &self.value
    }

    /// Map over the inner value while preserving probability
    ///
    /// # Examples
    ///
    /// ```
    /// use amari_flynn::prob::Prob;
    ///
    /// let x = Prob::with_probability(0.5, 10);
    /// let y = x.map(|v| v * 2);
    /// assert_eq!(y.into_inner(), 20);
    /// assert_eq!(y.probability(), 0.5);
    /// ```
    #[inline]
    pub fn map<U, F>(self, f: F) -> Prob<U>
    where
        F: FnOnce(T) -> U,
    {
        Prob {
            value: f(self.value),
            probability: self.probability,
        }
    }

    /// Monadic bind for probabilistic values
    ///
    /// Combines probabilities multiplicatively (assumes independence)
    #[inline]
    pub fn and_then<U, F>(self, f: F) -> Prob<U>
    where
        F: FnOnce(T) -> Prob<U>,
    {
        let result = f(self.value);
        Prob {
            value: result.value,
            probability: self.probability * result.probability,
        }
    }

    /// Sample this probabilistic value
    ///
    /// Returns Some(value) with probability p, None otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// use amari_flynn::prob::Prob;
    ///
    /// let coin = Prob::with_probability(0.5, "heads");
    /// let result = coin.sample(&mut rand::rng());
    /// // result is either Some("heads") or None
    /// ```
    #[inline]
    pub fn sample<R: Rng>(self, rng: &mut R) -> Option<T> {
        if rng.random::<f64>() < self.probability {
            Some(self.value)
        } else {
            None
        }
    }
}

/// Phantom type for probability density (future verification use)
///
/// This ghost type exists for future formal verification integration
/// where we can prove properties about probability distributions.
#[derive(Clone, Copy, Debug)]
pub struct ProbabilityDensity<T> {
    _phantom: core::marker::PhantomData<T>,
}

impl<T> ProbabilityDensity<T> {
    /// Create a phantom probability density
    pub fn new() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<T> Default for ProbabilityDensity<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn test_new_has_probability_one() {
        let p = Prob::new(42);
        assert_eq!(p.probability(), 1.0);
        assert_eq!(p.into_inner(), 42);
    }

    #[test]
    fn test_with_probability() {
        let p = Prob::with_probability(0.5, "test");
        assert_eq!(p.probability(), 0.5);
        assert_eq!(p.into_inner(), "test");
    }

    #[test]
    #[should_panic]
    fn test_invalid_probability_panics() {
        let _ = Prob::with_probability(1.5, ());
    }

    #[test]
    fn test_map_preserves_probability() {
        let p = Prob::with_probability(0.3, 10);
        let q = p.map(|x| x * 2);
        assert_eq!(q.into_inner(), 20);
        assert_eq!(q.probability(), 0.3);
    }

    #[test]
    fn test_and_then_multiplies_probabilities() {
        let p = Prob::with_probability(0.5, 10);
        let q = p.and_then(|x| Prob::with_probability(0.4, x + 5));
        assert_eq!(q.into_inner(), 15);
        assert!((q.probability() - 0.2).abs() < 1e-10);
    }
}
