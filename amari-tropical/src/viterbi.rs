//! Viterbi algorithm using tropical algebra
//!
//! The Viterbi algorithm for finding the most likely sequence becomes
//! simple polynomial evaluation in tropical algebra.

use crate::{TropicalMatrix, TropicalMultivector, TropicalNumber};
use alloc::vec::Vec;
use num_traits::Float;

/// Viterbi decoder using tropical algebra
pub struct TropicalViterbi<T: Float> {
    transition_matrix: TropicalMatrix<T>,
    emission_matrix: TropicalMatrix<T>,
}

impl<T: Float> TropicalViterbi<T> {
    /// Create new Viterbi decoder
    pub fn new(transitions: Vec<Vec<T>>, emissions: Vec<Vec<T>>) -> Self {
        Self {
            transition_matrix: TropicalMatrix::from_log_probs(&transitions),
            emission_matrix: TropicalMatrix::from_log_probs(&emissions),
        }
    }

    /// Find most likely state sequence using tropical algebra
    pub fn decode(&self, observations: &[usize]) -> (Vec<usize>, TropicalNumber<T>) {
        let n_states = self.transition_matrix.rows;
        let n_obs = observations.len();

        if n_obs == 0 {
            return (Vec::new(), TropicalNumber::zero());
        }

        // Viterbi trellis as tropical matrix
        let mut trellis = TropicalMatrix::new(n_states, n_obs);
        let mut path = Vec::with_capacity(n_states);
        for _ in 0..n_states {
            path.push(vec![0; n_obs]);
        }

        // Initialize first column (tropical style)
        for state in 0..n_states {
            let emission_prob = self.emission_matrix.data[state][observations[0]];
            trellis.data[state][0] = emission_prob;
        }

        // Forward pass using tropical operations
        #[allow(clippy::needless_range_loop)]
        for t in 1..n_obs {
            #[allow(clippy::needless_range_loop)]
            for curr_state in 0..n_states {
                let mut best_prob = TropicalNumber::zero();
                let mut best_prev = 0;

                for prev_state in 0..n_states {
                    // Tropical: prev_prob + transition + emission
                    let prob = trellis.data[prev_state][t - 1]
                        * self.transition_matrix.data[prev_state][curr_state]
                        * self.emission_matrix.data[curr_state][observations[t]];

                    // Tropical max operation
                    if prob.value() > best_prob.value() {
                        best_prob = prob;
                        best_prev = prev_state;
                    }
                }

                trellis.data[curr_state][t] = best_prob;
                path[curr_state][t] = best_prev;
            }
        }

        // Find best final state
        let mut best_final_prob = TropicalNumber::zero();
        let mut best_final_state = 0;

        for (state, row) in trellis.data.iter().enumerate().take(n_states) {
            let prob = row[n_obs - 1];
            if prob.value() > best_final_prob.value() {
                best_final_prob = prob;
                best_final_state = state;
            }
        }

        // Backtrack to find optimal path
        let mut optimal_path = Vec::with_capacity(n_obs);
        optimal_path.extend(vec![0; n_obs]);
        optimal_path[n_obs - 1] = best_final_state;

        for t in (0..n_obs - 1).rev() {
            optimal_path[t] = path[optimal_path[t + 1]][t + 1];
        }

        (optimal_path, best_final_prob)
    }

    /// Compute forward probabilities using tropical matrix multiplication
    pub fn forward_probabilities(&self, observations: &[usize]) -> TropicalMatrix<T> {
        let n_states = self.transition_matrix.rows;
        let n_obs = observations.len();

        let mut forward = TropicalMatrix::new(n_states, n_obs);

        // Initialize
        for state in 0..n_states {
            forward.data[state][0] = self.emission_matrix.data[state][observations[0]];
        }

        // Forward recursion using tropical operations
        #[allow(clippy::needless_range_loop)]
        for t in 1..n_obs {
            for state in 0..n_states {
                let mut prob_sum = TropicalNumber::zero();

                for prev_state in 0..n_states {
                    let prob = forward.data[prev_state][t - 1]
                        * self.transition_matrix.data[prev_state][state];
                    prob_sum = prob_sum + prob;
                }

                forward.data[state][t] =
                    prob_sum * self.emission_matrix.data[state][observations[t]];
            }
        }

        forward
    }
}

/// Tropical polynomial evaluation for sequence modeling
pub struct TropicalPolynomial<T: Float> {
    coefficients: Vec<TropicalNumber<T>>,
}

impl<T: Float> TropicalPolynomial<T> {
    /// Create from coefficients
    pub fn new(coeffs: Vec<T>) -> Self {
        Self {
            coefficients: coeffs.into_iter().map(TropicalNumber::new).collect(),
        }
    }

    /// Evaluate polynomial at point x
    pub fn evaluate(&self, x: TropicalNumber<T>) -> TropicalNumber<T> {
        if self.coefficients.is_empty() {
            return TropicalNumber::zero();
        }

        let mut result = self.coefficients[0];
        let mut x_power = TropicalNumber::tropical_one();

        for &coeff in self.coefficients.iter().skip(1) {
            x_power = x_power.tropical_mul(&x);
            result = result.tropical_add(&coeff.tropical_mul(&x_power));
        }

        result
    }

    /// Find roots using tropical geometry
    pub fn tropical_roots(&self) -> Vec<TropicalNumber<T>> {
        // In tropical algebra, roots correspond to breakpoints
        // of the piecewise-linear function
        let mut roots = Vec::new();

        // Simplified root finding - in practice this would use
        // more sophisticated tropical geometry algorithms
        for i in 0..self.coefficients.len() - 1 {
            for j in i + 1..self.coefficients.len() {
                if !self.coefficients[i].is_zero() && !self.coefficients[j].is_zero() {
                    // Tropical root: where two terms balance
                    let root_val = (self.coefficients[j].value() - self.coefficients[i].value())
                        / T::from(j - i).unwrap();
                    roots.push(TropicalNumber::new(root_val));
                }
            }
        }

        roots
    }
}

/// Tropical convex hull for attention patterns
pub fn tropical_convex_hull<T: Float>(points: &[TropicalMultivector<T, 2, 0, 0>]) -> Vec<usize> {
    // Simplified tropical convex hull computation
    // In tropical geometry, the convex hull has a different structure

    if points.is_empty() {
        return Vec::new();
    }

    let mut hull_indices = Vec::new();

    // Find points that maximize different linear functionals
    for i in 0..points.len() {
        let mut is_extreme = true;

        for j in 0..points.len() {
            if i == j {
                continue;
            }

            // Check if point i can be written as tropical combination of others
            // This is a simplified check
            let diff = points[i].geometric_product(&points[j]);
            if diff.tropical_norm().value() < T::epsilon() {
                is_extreme = false;
                break;
            }
        }

        if is_extreme {
            hull_indices.push(i);
        }
    }

    hull_indices
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use approx::assert_relative_eq;

    #[test]
    fn test_viterbi_decoding() {
        // Simple 2-state HMM
        let transitions = vec![
            vec![-1.0, -2.0], // log probabilities
            vec![-2.0, -1.0],
        ];

        let emissions = vec![
            vec![-1.0, -3.0], // state 0 emissions
            vec![-3.0, -1.0], // state 1 emissions
        ];

        let viterbi = TropicalViterbi::new(transitions, emissions);
        let observations = vec![0, 1, 0];

        let (path, prob) = viterbi.decode(&observations);

        assert_eq!(path.len(), 3);
        assert!(!prob.is_zero());

        // Verify path makes sense
        assert!(path[0] < 2);
        assert!(path[1] < 2);
        assert!(path[2] < 2);
    }

    #[test]
    fn test_tropical_polynomial() {
        let poly = TropicalPolynomial::new(vec![0.0, 1.0, 2.0]); // 0 + x + 2x^2

        let x = TropicalNumber::new(1.0);
        let result = poly.evaluate(x);

        // Should be max(0, 1+1, 2+1+1) = max(0, 2, 4) = 4
        assert_relative_eq!(result.value(), 4.0, epsilon = 1e-10);

        // Test root finding
        let roots = poly.tropical_roots();
        assert!(!roots.is_empty());
    }

    #[test]
    fn test_forward_probabilities() {
        let transitions = vec![vec![-1.0, -2.0], vec![-2.0, -1.0]];

        let emissions = vec![vec![-1.0, -3.0], vec![-3.0, -1.0]];

        let viterbi = TropicalViterbi::new(transitions, emissions);
        let observations = vec![0, 1];

        let forward = viterbi.forward_probabilities(&observations);

        // Check dimensions
        assert_eq!(forward.rows, 2);
        assert_eq!(forward.cols, 2);

        // All probabilities should be valid (not zero/neg-inf)
        for row in &forward.data {
            for value in row {
                assert!(!value.is_zero());
            }
        }
    }
}
