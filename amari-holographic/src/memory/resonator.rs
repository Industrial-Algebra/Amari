//! Codebook selection, weighted cleanup and ordered two-factor recovery.
use super::domain::{cosine, finite, key_inverse, MemoryElement};
use super::{HolographicError, HolographicResult};
use alloc::vec::Vec;

/// Linear inverse-temperature schedule. One update uses the initial endpoint.
#[derive(Clone, Debug)]
pub struct ResonatorConfig {
    pub max_iterations: usize,
    pub initial_beta: f64,
    pub final_beta: f64,
}
impl Default for ResonatorConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            initial_beta: 1.,
            final_beta: 100.,
        }
    }
}
/// Nearest available candidate, explicitly distinguished from a certified recovery.
#[derive(Clone, Debug)]
pub enum CleanupResult<A> {
    Selected { candidate: A, residual: f64 },
    NoAcceptableCandidate { best: A, residual: f64 },
}
/// Termination reason; never substitutes for the observation certificate.
#[derive(Clone, Debug)]
pub enum WeightedStatus {
    Updated { iterations: usize },
    Stalled { iterations: usize, last_delta: f64 },
    BudgetExhausted,
    RejectedAtIngress,
}
/// The actual soft mixture and its residual against the original observation.
#[derive(Clone, Debug)]
pub struct WeightedResult<A> {
    pub candidate: A,
    pub residual: f64,
    pub accepted: bool,
    pub status: WeightedStatus,
}
/// Factor estimates with a certificate on their ordered product.
#[derive(Clone, Debug)]
pub struct FactorizationResult<A> {
    pub factor_a: A,
    pub factor_b: A,
    pub iterations: usize,
    pub accepted: bool,
    pub residual: f64,
}
#[derive(Clone)]
pub struct Resonator<A> {
    codebook: Vec<A>,
    config: ResonatorConfig,
}

fn certificate<A: MemoryElement>(obs: &A, candidate: &A) -> HolographicResult<(bool, f64)> {
    let a = finite(obs)?;
    let b = finite(candidate)?;
    if a.len() != b.len() {
        return Err(HolographicError::DimensionMismatch {
            expected: a.len(),
            actual: b.len(),
        });
    }
    let (mut accepted, mut max) = (true, 0_f64);
    for (x, y) in a.iter().zip(b) {
        let err = (x - y).abs();
        accepted &= err <= 1e-10 + 1e-8 * x.abs();
        max = max.max(err);
    }
    Ok((accepted, max))
}
impl<A: MemoryElement> Resonator<A> {
    /// Reject empty books, invalid budgets and invalid schedules. Infinite endpoints
    /// mean hard selection at that endpoint, never arithmetic on infinities.
    pub fn new(codebook: Vec<A>, config: ResonatorConfig) -> HolographicResult<Self> {
        if codebook.is_empty() {
            return Err(HolographicError::EmptyCodebook);
        }
        if config.max_iterations == 0 {
            return Err(HolographicError::ResonatorDidNotConverge(0));
        }
        for beta in [config.initial_beta, config.final_beta] {
            if beta.is_nan() || beta < 0. {
                return Err(HolographicError::InvalidTemperature(beta));
            }
        }
        if config.final_beta < config.initial_beta {
            return Err(HolographicError::InvalidTemperature(config.final_beta));
        }
        for word in &codebook {
            finite(word)?;
        }
        Ok(Self { codebook, config })
    }
    pub fn codebook_size(&self) -> usize {
        self.codebook.len()
    }
    pub fn get_codebook_item(&self, i: usize) -> Option<&A> {
        self.codebook.get(i)
    }
    fn scores(&self, input: &A) -> HolographicResult<Vec<f64>> {
        let obs = finite(input)?;
        self.codebook
            .iter()
            .map(|w| cosine(&obs, &finite(w)?))
            .collect()
    }
    fn argmax(&self, scores: &[f64]) -> usize {
        let mut best = 0;
        for (i, &s) in scores.iter().enumerate().skip(1) {
            if s > scores[best]
                || (s == scores[best]
                    && self.codebook[i].full_coefficients()
                        < self.codebook[best].full_coefficients())
            {
                best = i;
            }
        }
        best
    }
    /// Full coefficient cosine; the lexicographically smallest coefficient tuple wins ties.
    pub fn cleanup_nearest(&self, input: &A) -> HolographicResult<CleanupResult<A>> {
        let best = self.codebook[self.argmax(&self.scores(input)?)].clone();
        let (ok, residual) = certificate(input, &best)?;
        Ok(if ok {
            CleanupResult::Selected {
                candidate: best,
                residual,
            }
        } else {
            CleanupResult::NoAcceptableCandidate { best, residual }
        })
    }
    /// Iterative weighted sum on the entire representation. Constant infinity selects
    /// the deterministic maximizer. A finite-to-infinite schedule switches at its last step.
    pub fn cleanup_weighted(&self, input: &A) -> HolographicResult<WeightedResult<A>> {
        let mut current = input.clone();
        let cfg = &self.config;
        for i in 0..cfg.max_iterations {
            let iterations = i + 1;
            let beta = if i == 0 {
                cfg.initial_beta
            } else if iterations == cfg.max_iterations {
                cfg.final_beta
            } else if cfg.final_beta.is_infinite() {
                cfg.initial_beta
            } else {
                cfg.initial_beta
                    + (cfg.final_beta - cfg.initial_beta)
                        * (i as f64 / (cfg.max_iterations - 1) as f64)
            };
            let scores = self.scores(&current)?;
            let best = self.argmax(&scores);
            let next = if beta.is_infinite() {
                self.codebook[best].clone()
            } else {
                let weights: Vec<_> = scores
                    .iter()
                    .map(|s| (beta * (s - scores[best])).exp())
                    .collect();
                let sum: f64 = weights.iter().sum();
                let mut acc = A::zero();
                for (w, word) in weights.iter().zip(&self.codebook) {
                    let term = word
                        .scale(w / sum)
                        .map_err(|source| HolographicError::Algebra {
                            operation: "weighted scale".into(),
                            source,
                        })?;
                    acc = acc
                        .superpose(&term)
                        .map_err(|source| HolographicError::Algebra {
                            operation: "weighted superposition".into(),
                            source,
                        })?;
                }
                acc
            };
            let (accepted, residual) = certificate(input, &next)?;
            let (still, last_delta) = certificate(&current, &next)?;
            let status = if accepted {
                WeightedStatus::Updated { iterations }
            } else if still {
                WeightedStatus::Stalled {
                    iterations,
                    last_delta,
                }
            } else if iterations == cfg.max_iterations {
                WeightedStatus::BudgetExhausted
            } else {
                current = next;
                continue;
            };
            return Ok(WeightedResult {
                candidate: next,
                residual,
                accepted,
                status,
            });
        }
        Err(HolographicError::ResonatorDidNotConverge(0))
    }
    /// For T=A·B, estimate B as A⁻¹·T, then A as T·B⁻¹. No uniqueness claim.
    pub fn factorize(&self, bound: &A, other: &Self) -> HolographicResult<FactorizationResult<A>> {
        finite(bound)?;
        let left = self
            .codebook
            .iter()
            .map(key_inverse)
            .collect::<HolographicResult<Vec<_>>>()?;
        let right = other
            .codebook
            .iter()
            .map(key_inverse)
            .collect::<HolographicResult<Vec<_>>>()?;
        let mut a = 0;
        for iterations in 1..=self.config.max_iterations {
            let b = other.argmax(&other.scores(&left[a].bind(bound))?);
            let next = self.argmax(&self.scores(&bound.bind(&right[b]))?);
            let (accepted, residual) =
                certificate(bound, &self.codebook[next].bind(&other.codebook[b]))?;
            if accepted || next == a || iterations == self.config.max_iterations {
                return Ok(FactorizationResult {
                    factor_a: self.codebook[next].clone(),
                    factor_b: other.codebook[b].clone(),
                    iterations,
                    accepted,
                    residual,
                });
            }
            a = next;
        }
        Err(HolographicError::ResonatorDidNotConverge(0))
    }
}
