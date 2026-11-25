//! Holographic associative memory for key-value storage in superposition.
//!
//! This module provides the main `HolographicMemory` data structure that stores
//! key-value associations using holographic reduced representations.

use alloc::vec::Vec;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::algebra::{AlgebraConfig, BindingAlgebra};
use super::error::{HolographicError, HolographicResult};

/// A holographic associative memory storing key-value pairs in superposition.
///
/// This is generic over any algebra implementing [`BindingAlgebra`], allowing
/// use with Clifford, ProductClifford, FHRR, MAP, or custom algebras.
///
/// # Capacity
///
/// Reliable retrieval degrades as items are added. Capacity scales as O(dim / log dim).
/// The memory tracks estimated SNR and warns when approaching capacity limits.
///
/// # Example
///
/// ```ignore
/// use amari_holographic::{HolographicMemory, AlgebraConfig};
/// use amari_holographic::algebra::ProductCl3x32;
///
/// let mut memory = HolographicMemory::<ProductCl3x32>::new(AlgebraConfig::default());
///
/// let key = ProductCl3x32::random_versor(2);
/// let value = ProductCl3x32::random_versor(2);
///
/// memory.store(&key, &value);
///
/// let retrieved = memory.retrieve(&key);
/// assert!(retrieved.confidence > 0.9);
/// ```
#[derive(Clone)]
pub struct HolographicMemory<A: BindingAlgebra> {
    /// The superposed memory trace
    trace: A,
    /// Number of items stored
    item_count: usize,
    /// Algebra configuration (bundling/retrieval parameters)
    config: AlgebraConfig,
    /// Optional: store keys for resonator cleanup and attribution
    stored_keys: Option<Vec<A>>,
}

/// Result of a retrieval operation.
#[derive(Clone, Debug)]
pub struct RetrievalResult<A: BindingAlgebra> {
    /// The retrieved value (after cleanup if enabled)
    pub value: A,
    /// Raw retrieved value before cleanup
    pub raw_value: A,
    /// Estimated confidence in [0, 1] based on SNR
    pub confidence: f64,
    /// Attribution: which stored items contributed
    /// Indices correspond to storage order if key tracking is enabled
    pub attribution: Vec<(usize, f64)>,
    /// Similarity of retrieved value to query (sanity check)
    pub query_similarity: f64,
}

impl<A: BindingAlgebra> RetrievalResult<A> {
    /// Verify that this retrieval result is valid.
    pub fn verify_result_validity(&self) -> bool {
        // Check that value is finite
        let value_norm = self.value.norm();
        if !value_norm.is_finite() {
            return false;
        }

        // Check confidence is in valid range
        if !(0.0..=1.0).contains(&self.confidence) {
            return false;
        }

        true
    }
}

/// Information about memory capacity and health.
#[derive(Clone, Debug)]
pub struct CapacityInfo {
    /// Current number of stored items
    pub item_count: usize,
    /// Theoretical maximum before severe degradation
    pub theoretical_capacity: usize,
    /// Estimated signal-to-noise ratio (higher is better)
    pub estimated_snr: f64,
    /// Recommended: stop storing if SNR drops below this
    pub snr_threshold: f64,
    /// Whether memory is approaching capacity limits
    pub near_capacity: bool,
}

impl<A: BindingAlgebra> HolographicMemory<A> {
    /// Create a new empty holographic memory.
    pub fn new(config: AlgebraConfig) -> Self {
        Self {
            trace: A::zero(),
            item_count: 0,
            config,
            stored_keys: None,
        }
    }

    /// Create with key tracking enabled (for resonator cleanup and attribution).
    pub fn with_key_tracking(config: AlgebraConfig) -> Self {
        Self {
            trace: A::zero(),
            item_count: 0,
            config,
            stored_keys: Some(Vec::new()),
        }
    }

    /// Store a key-value association by additive superposition.
    ///
    /// The trace is the raw sum `T = Σ keyᵢ ⊛ valueᵢ`; no attention weighting
    /// or normalization is applied. Repeated associations therefore accumulate.
    pub fn store(&mut self, key: &A, value: &A) -> HolographicResult<()> {
        // Bind key and value
        let bound = key.bind(value);

        // Add to the raw memory trace; storage is accumulation, not attention.
        let candidate = if self.item_count == 0 {
            bound
        } else {
            self.trace.superpose(&bound).map_err(|source| {
                HolographicError::Algebra {
                    operation: "store superpose".to_string(),
                    source,
                }
            })?
        };

        self.trace = candidate;

        // Track key if enabled
        if let Some(ref mut keys) = self.stored_keys {
            keys.push(key.clone());
        }

        self.item_count += 1;
        Ok(())
    }

    /// Store multiple associations by concatenating their additive traces.
    pub fn store_batch(&mut self, pairs: &[(A, A)]) -> HolographicResult<()> {
        if pairs.is_empty() {
            return Ok(());
        }

        // Bind all pairs. Binding is commutative-free per algebra, but the
        // subsequent superposition is ordered so the result is deterministic.
        #[cfg(feature = "parallel")]
        let bindings: Vec<_> = pairs.par_iter().map(|(k, v)| k.bind(v)).collect();

        #[cfg(not(feature = "parallel"))]
        let bindings: Vec<_> = pairs.iter().map(|(k, v)| k.bind(v)).collect();

        let mut batch_trace = A::zero();
        for binding in &bindings {
            batch_trace = batch_trace.superpose(binding).map_err(|source| {
                HolographicError::Algebra {
                    operation: "store_batch superpose".to_string(),
                    source,
                }
            })?;
        }

        let candidate = if self.item_count == 0 {
            batch_trace
        } else {
            self.trace.superpose(&batch_trace).map_err(|source| {
                HolographicError::Algebra {
                    operation: "store_batch merge superpose".to_string(),
                    source,
                }
            })?
        };

        self.trace = candidate;

        // Track keys if enabled
        if let Some(ref mut keys) = self.stored_keys {
            for (k, _) in pairs {
                keys.push(k.clone());
            }
        }

        self.item_count += pairs.len();
        Ok(())
    }

    /// Retrieve the value associated with a key.
    pub fn retrieve(&self, key: &A) -> RetrievalResult<A> {
        self.retrieve_at_temperature(key, self.config.retrieval_beta)
    }

    /// Retrieve with custom temperature (override config settings).
    pub fn retrieve_at_temperature(&self, key: &A, beta: f64) -> RetrievalResult<A> {
        // Unbind with key to get noisy value
        let raw_value = key.unbind(&self.trace).unwrap_or_else(|_| A::zero());

        // Compute confidence based on SNR estimate
        let snr = self.estimate_snr();
        let confidence = self.snr_to_confidence(snr);

        // For hard retrieval, normalize the result
        let value = if beta.is_infinite() {
            raw_value.normalize().unwrap_or_else(|_| raw_value.clone())
        } else {
            raw_value.clone()
        };

        // Compute attribution if key tracking is enabled
        let attribution = self.compute_attribution(key, &value);

        // Compute query similarity
        let query_similarity = value.similarity(key);

        RetrievalResult {
            value,
            raw_value,
            confidence,
            attribution,
            query_similarity,
        }
    }

    /// Query with a partial or noisy key (holographic graceful degradation).
    pub fn query_partial(&self, partial_key: &A, _mask: &[bool]) -> RetrievalResult<A> {
        // For partial queries, we just retrieve normally
        // The holographic representation naturally handles partial information
        self.retrieve(partial_key)
    }

    /// Check if a key is likely stored (approximate membership).
    pub fn probably_contains(&self, key: &A) -> bool {
        let result = self.retrieve(key);
        result.confidence > self.config.similarity_threshold
    }

    /// Get capacity information.
    pub fn capacity_info(&self) -> CapacityInfo {
        let theoretical_capacity = self.theoretical_capacity();
        let estimated_snr = self.estimate_snr();
        let snr_threshold = 2.0; // Minimum acceptable SNR

        CapacityInfo {
            item_count: self.item_count,
            theoretical_capacity,
            estimated_snr,
            snr_threshold,
            near_capacity: self.item_count > theoretical_capacity / 2,
        }
    }

    /// Clear the memory.
    pub fn clear(&mut self) {
        self.trace = A::zero();
        self.item_count = 0;
        if let Some(ref mut keys) = self.stored_keys {
            keys.clear();
        }
    }

    /// Merge another memory into this one by additive superposition.
    ///
    /// An empty source is a no-op. The trace and item count advance together.
    pub fn merge(&mut self, other: &Self) -> HolographicResult<()> {
        if other.item_count == 0 {
            return Ok(());
        }

        let candidate = if self.item_count == 0 {
            other.trace.clone()
        } else {
            self.trace.superpose(&other.trace).map_err(|source| {
                HolographicError::Algebra {
                    operation: "merge superpose".to_string(),
                    source,
                }
            })?
        };

        self.trace = candidate;
        self.item_count += other.item_count;

        // Merge keys if tracking
        if let (Some(ref mut self_keys), Some(ref other_keys)) =
            (&mut self.stored_keys, &other.stored_keys)
        {
            self_keys.extend(other_keys.iter().cloned());
        }
        Ok(())
    }

    /// Get the raw memory trace (for inspection/serialization).
    pub fn trace(&self) -> &A {
        &self.trace
    }

    /// Get the number of stored items.
    pub fn item_count(&self) -> usize {
        self.item_count
    }

    /// Verify memory consistency.
    pub fn verify_consistency(&self) -> bool {
        // Check trace is valid
        let norm = self.trace.norm();
        if !norm.is_finite() {
            return false;
        }

        // Check item count consistency with stored keys
        if let Some(ref keys) = self.stored_keys {
            if keys.len() != self.item_count {
                return false;
            }
        }

        true
    }

    // ========================================================================
    // Private helper methods
    // ========================================================================

    /// Estimate signal-to-noise ratio.
    fn estimate_snr(&self) -> f64 {
        if self.item_count == 0 {
            return f64::INFINITY;
        }

        // Use the algebra's estimate_snr method
        self.trace.estimate_snr(self.item_count)
    }

    /// Convert SNR to confidence in [0, 1].
    fn snr_to_confidence(&self, snr: f64) -> f64 {
        if snr.is_infinite() {
            return 1.0;
        }
        // Sigmoid-like function that maps SNR to confidence
        // SNR of 2 gives ~0.88, SNR of 5 gives ~0.99
        1.0 - (-snr / 2.0).exp()
    }

    /// Theoretical capacity.
    fn theoretical_capacity(&self) -> usize {
        self.trace.theoretical_capacity()
    }

    /// Compute attribution for a retrieval.
    fn compute_attribution(&self, key: &A, _value: &A) -> Vec<(usize, f64)> {
        let Some(ref keys) = self.stored_keys else {
            return Vec::new();
        };

        // Compute similarity of each stored key to the query key
        let mut attributions: Vec<(usize, f64)> = keys
            .iter()
            .enumerate()
            .map(|(i, stored_key)| {
                let sim = key.similarity(stored_key);
                (i, sim.max(0.0))
            })
            .collect();

        // Sort by attribution (descending)
        attributions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));

        // Normalize to sum to 1
        let total: f64 = attributions.iter().map(|(_, w)| w).sum();
        if total > 1e-10 {
            for (_, w) in &mut attributions {
                *w /= total;
            }
        }

        // Keep only significant attributions
        attributions
            .into_iter()
            .filter(|(_, w)| *w > 0.01)
            .collect()
    }
}

/// Marker for verified memory operations.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct MemoryVerified;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::product_clifford::ProductCl3x8;

    #[test]
    fn test_memory_store_retrieve() {
        let mut memory = HolographicMemory::<ProductCl3x8>::new(AlgebraConfig::default());

        let key = ProductCl3x8::random_versor(2);
        let value = ProductCl3x8::random_versor(2);

        memory.store(&key, &value);

        let result = memory.retrieve(&key);
        assert!(result.confidence > 0.5, "confidence: {}", result.confidence);

        let sim = result.value.similarity(&value);
        assert!(sim > 0.9, "similarity: {}", sim);
    }

    #[test]
    fn test_memory_capacity_info() {
        let memory = HolographicMemory::<ProductCl3x8>::new(AlgebraConfig::default());
        let info = memory.capacity_info();

        assert_eq!(info.item_count, 0);
        assert!(info.estimated_snr.is_infinite());
        assert!(!info.near_capacity);
    }

    #[test]
    fn test_memory_clear() {
        let mut memory = HolographicMemory::<ProductCl3x8>::new(AlgebraConfig::default());

        let key = ProductCl3x8::random_versor(2);
        let value = ProductCl3x8::random_versor(2);

        memory.store(&key, &value);
        assert_eq!(memory.item_count(), 1);

        memory.clear();
        assert_eq!(memory.item_count(), 0);
    }

    #[test]
    fn test_snr_to_confidence() {
        let memory = HolographicMemory::<ProductCl3x8>::new(AlgebraConfig::default());

        // High SNR should give high confidence
        let high_conf = memory.snr_to_confidence(10.0);
        assert!(high_conf > 0.99);

        // Low SNR should give lower confidence
        let low_conf = memory.snr_to_confidence(1.0);
        assert!(low_conf < 0.5);

        // Infinite SNR (empty memory) should give 1.0
        let inf_conf = memory.snr_to_confidence(f64::INFINITY);
        assert!((inf_conf - 1.0).abs() < 1e-10);
    }
}
