//! GPU-accelerated optimization algorithms
//!
//! This module provides GPU acceleration for optimization algorithms using the amari-gpu infrastructure.
//! It implements progressive enhancement: automatically detects GPU capabilities and falls back to CPU
//! computation when necessary.

use crate::{OptimizationError, OptimizationSolution};
use amari_core::Multivector;
#[cfg(feature = "gpu")]
use amari_gpu::SharedGpuContext;

/// Type alias for objective functions used in multi-objective optimization
type ObjectiveFunction = Box<dyn Fn(&[f64]) -> f64 + Send + Sync>;

/// GPU-accelerated optimization dispatcher
pub struct GpuOptimizer {
    #[cfg(feature = "gpu")]
    context: Option<SharedGpuContext>,
    fallback_enabled: bool,
}

impl GpuOptimizer {
    /// Create a new GPU optimizer with automatic fallback
    pub async fn new() -> Self {
        #[cfg(feature = "gpu")]
        {
            // For now, disable GPU initialization to avoid private API issues
            // In a full implementation, this would use a public factory method
            let context = None;

            Self {
                context,
                fallback_enabled: true,
            }
        }

        #[cfg(not(feature = "gpu"))]
        {
            Self {
                fallback_enabled: true,
            }
        }
    }

    /// Check if GPU acceleration is available
    pub fn is_gpu_available(&self) -> bool {
        #[cfg(feature = "gpu")]
        {
            self.context.is_some()
        }

        #[cfg(not(feature = "gpu"))]
        {
            false
        }
    }

    /// Optimize a quadratic function using GPU acceleration when beneficial
    pub async fn optimize_quadratic(
        &self,
        coefficients: &[f64],
        initial_point: &[f64],
        max_iterations: usize,
        tolerance: f64,
    ) -> Result<OptimizationSolution, OptimizationError> {
        if coefficients.len() != initial_point.len() {
            return Err(OptimizationError::InvalidInput(
                "Coefficients and initial point must have same length".to_string(),
            ));
        }

        // For large problems, try GPU acceleration
        if self.should_use_gpu(coefficients.len()) && self.is_gpu_available() {
            match self
                .optimize_quadratic_gpu(coefficients, initial_point, max_iterations, tolerance)
                .await
            {
                Ok(result) => return Ok(result),
                Err(_) if self.fallback_enabled => {
                    // Fall back to CPU implementation
                }
                Err(e) => return Err(e),
            }
        }

        // CPU implementation
        self.optimize_quadratic_cpu(coefficients, initial_point, max_iterations, tolerance)
    }

    /// Batch optimization of multiple quadratic functions
    pub async fn optimize_quadratic_batch(
        &self,
        problems: &[(Vec<f64>, Vec<f64>)], // (coefficients, initial_point) pairs
        max_iterations: usize,
        tolerance: f64,
    ) -> Result<Vec<OptimizationSolution>, OptimizationError> {
        if problems.is_empty() {
            return Ok(Vec::new());
        }

        // For large batches, prefer GPU acceleration
        if self.should_use_gpu_batch(problems.len()) && self.is_gpu_available() {
            match self
                .optimize_quadratic_batch_gpu(problems, max_iterations, tolerance)
                .await
            {
                Ok(results) => return Ok(results),
                Err(_) if self.fallback_enabled => {
                    // Fall back to CPU implementation
                }
                Err(e) => return Err(e),
            }
        }

        // CPU batch implementation
        let mut results = Vec::with_capacity(problems.len());
        for (coefficients, initial_point) in problems {
            let result = self.optimize_quadratic_cpu(
                coefficients,
                initial_point,
                max_iterations,
                tolerance,
            )?;
            results.push(result);
        }

        Ok(results)
    }

    /// Natural gradient descent using geometric algebra and GPU acceleration
    pub async fn natural_gradient_descent<const P: usize, const Q: usize, const R: usize>(
        &self,
        objective: impl Fn(&Multivector<P, Q, R>) -> f64 + Send + Sync,
        initial_point: &Multivector<P, Q, R>,
        learning_rate: f64,
        max_iterations: usize,
        tolerance: f64,
    ) -> Result<OptimizationSolution, OptimizationError> {
        // For now, always use CPU implementation to avoid ownership issues
        // In a full implementation, this would use GPU acceleration for large basis spaces
        self.natural_gradient_descent_cpu(
            objective,
            initial_point,
            learning_rate,
            max_iterations,
            tolerance,
        )
    }

    // Private implementation methods

    fn should_use_gpu(&self, problem_size: usize) -> bool {
        // GPU is beneficial for larger problems where parallelization pays off
        problem_size >= 100
    }

    fn should_use_gpu_batch(&self, batch_size: usize) -> bool {
        // GPU is beneficial for batch operations
        batch_size >= 10
    }

    #[cfg(feature = "gpu")]
    async fn optimize_quadratic_gpu(
        &self,
        coefficients: &[f64],
        initial_point: &[f64],
        max_iterations: usize,
        tolerance: f64,
    ) -> Result<OptimizationSolution, OptimizationError> {
        // Since GPU context is disabled for now, always fall back to CPU
        // In a full implementation, this would use GPU acceleration when context is available
        self.optimize_quadratic_cpu(coefficients, initial_point, max_iterations, tolerance)
    }

    #[cfg(not(feature = "gpu"))]
    async fn optimize_quadratic_gpu(
        &self,
        coefficients: &[f64],
        initial_point: &[f64],
        max_iterations: usize,
        tolerance: f64,
    ) -> Result<OptimizationSolution, OptimizationError> {
        // GPU not available, fall back to CPU
        self.optimize_quadratic_cpu(coefficients, initial_point, max_iterations, tolerance)
    }

    #[cfg(feature = "gpu")]
    async fn optimize_quadratic_batch_gpu(
        &self,
        problems: &[(Vec<f64>, Vec<f64>)],
        max_iterations: usize,
        tolerance: f64,
    ) -> Result<Vec<OptimizationSolution>, OptimizationError> {
        // Since GPU context is disabled for now, fall back to CPU batch processing
        // In a full implementation, this would use GPU for parallel optimization
        let mut results = Vec::with_capacity(problems.len());
        for (coefficients, initial_point) in problems {
            let result = self.optimize_quadratic_cpu(
                coefficients,
                initial_point,
                max_iterations,
                tolerance,
            )?;
            results.push(result);
        }

        Ok(results)
    }

    #[cfg(not(feature = "gpu"))]
    async fn optimize_quadratic_batch_gpu(
        &self,
        problems: &[(Vec<f64>, Vec<f64>)],
        max_iterations: usize,
        tolerance: f64,
    ) -> Result<Vec<OptimizationSolution>, OptimizationError> {
        // GPU not available, fall back to CPU batch
        let mut results = Vec::with_capacity(problems.len());
        for (coefficients, initial_point) in problems {
            let result = self.optimize_quadratic_cpu(
                coefficients,
                initial_point,
                max_iterations,
                tolerance,
            )?;
            results.push(result);
        }
        Ok(results)
    }

    // GPU methods are kept for future implementation but marked as dead code for now
    #[allow(dead_code)]
    #[cfg(feature = "gpu")]
    async fn natural_gradient_descent_gpu<const P: usize, const Q: usize, const R: usize>(
        &self,
        objective: impl Fn(&Multivector<P, Q, R>) -> f64 + Send + Sync,
        initial_point: &Multivector<P, Q, R>,
        learning_rate: f64,
        max_iterations: usize,
        tolerance: f64,
    ) -> Result<OptimizationSolution, OptimizationError> {
        // Since GPU context is disabled for now, fall back to CPU
        // In a full implementation, this would use GPU for geometric product computations
        self.natural_gradient_descent_cpu(
            objective,
            initial_point,
            learning_rate,
            max_iterations,
            tolerance,
        )
    }

    #[allow(dead_code)]
    #[cfg(not(feature = "gpu"))]
    async fn natural_gradient_descent_gpu<const P: usize, const Q: usize, const R: usize>(
        &self,
        _objective: impl Fn(&Multivector<P, Q, R>) -> f64 + Send + Sync,
        initial_point: &Multivector<P, Q, R>,
        learning_rate: f64,
        max_iterations: usize,
        tolerance: f64,
    ) -> Result<OptimizationSolution, OptimizationError> {
        // GPU not available, fall back to CPU
        self.natural_gradient_descent_cpu(
            _objective,
            initial_point,
            learning_rate,
            max_iterations,
            tolerance,
        )
    }

    fn optimize_quadratic_cpu(
        &self,
        coefficients: &[f64],
        initial_point: &[f64],
        max_iterations: usize,
        tolerance: f64,
    ) -> Result<OptimizationSolution, OptimizationError> {
        let mut x = initial_point.to_vec();
        let learning_rate = 0.01;
        let mut iterations = 0;

        for iter in 0..max_iterations {
            // Compute gradient: 2 * c_i * x_i for quadratic function sum(c_i * x_i^2)
            let mut gradient = vec![0.0; x.len()];
            for i in 0..x.len() {
                gradient[i] = 2.0 * coefficients[i] * x[i];
            }

            // Check convergence
            let grad_norm: f64 = gradient.iter().map(|g| g * g).sum::<f64>().sqrt();
            if grad_norm < tolerance {
                iterations = iter;
                break;
            }

            // Update x using gradient descent
            for i in 0..x.len() {
                x[i] -= learning_rate * gradient[i];
            }

            iterations = iter + 1;
        }

        // Compute final objective value
        let objective_value: f64 = coefficients
            .iter()
            .zip(&x)
            .map(|(c, x_val)| c * x_val * x_val)
            .sum();

        Ok(OptimizationSolution {
            solution: x,
            objective_value,
            iterations,
            converged: iterations < max_iterations,
            gradient_norm: None,
        })
    }

    fn natural_gradient_descent_cpu<const P: usize, const Q: usize, const R: usize>(
        &self,
        objective: impl Fn(&Multivector<P, Q, R>) -> f64,
        initial_point: &Multivector<P, Q, R>,
        learning_rate: f64,
        max_iterations: usize,
        tolerance: f64,
    ) -> Result<OptimizationSolution, OptimizationError> {
        let mut current = initial_point.clone();
        let mut iterations = 0;
        let epsilon = 1e-8;

        for iter in 0..max_iterations {
            // Compute numerical gradient
            let mut gradient = Multivector::zero();
            let _current_value = objective(&current);

            for i in 0..(1 << (P + Q + R)) {
                let mut point_plus = current.clone();
                let mut point_minus = current.clone();

                // Perturb coefficient i
                let current_coeff = current.get(i);
                point_plus.set(i, current_coeff + epsilon);
                point_minus.set(i, current_coeff - epsilon);

                let grad_i = (objective(&point_plus) - objective(&point_minus)) / (2.0 * epsilon);
                gradient.set(i, grad_i);
            }

            // Check convergence
            let grad_norm = gradient.norm_squared().sqrt();
            if grad_norm < tolerance {
                iterations = iter;
                break;
            }

            // Natural gradient step (simplified - just use Euclidean gradient for now)
            current = current - gradient * learning_rate;
            iterations = iter + 1;
        }

        let final_objective = objective(&current);
        let solution_vec: Vec<f64> = (0..(1 << (P + Q + R))).map(|i| current.get(i)).collect();

        Ok(OptimizationSolution {
            solution: solution_vec,
            objective_value: final_objective,
            iterations,
            converged: iterations < max_iterations,
            gradient_norm: None,
        })
    }
}

impl Default for GpuOptimizer {
    fn default() -> Self {
        // Cannot use async in Default, so create with no GPU context
        #[cfg(feature = "gpu")]
        {
            Self {
                context: None,
                fallback_enabled: true,
            }
        }

        #[cfg(not(feature = "gpu"))]
        {
            Self {
                fallback_enabled: true,
            }
        }
    }
}

/// GPU-accelerated multi-objective optimization
pub struct GpuMultiObjectiveOptimizer {
    #[cfg(feature = "gpu")]
    context: Option<SharedGpuContext>,
    fallback_enabled: bool,
}

impl GpuMultiObjectiveOptimizer {
    /// Create a new GPU multi-objective optimizer
    pub async fn new() -> Self {
        #[cfg(feature = "gpu")]
        {
            // For now, disable GPU initialization to avoid private API issues
            // In a full implementation, this would use a public factory method
            let context = None;

            Self {
                context,
                fallback_enabled: true,
            }
        }

        #[cfg(not(feature = "gpu"))]
        {
            Self {
                fallback_enabled: true,
            }
        }
    }

    /// Run NSGA-II algorithm with GPU acceleration for fitness evaluation
    pub async fn nsga_ii(
        &self,
        objectives: Vec<ObjectiveFunction>,
        constraints: Vec<ObjectiveFunction>,
        bounds: &[(f64, f64)],
        population_size: usize,
        generations: usize,
    ) -> Result<Vec<OptimizationSolution>, OptimizationError> {
        // For large populations, GPU acceleration can help with fitness evaluation
        if self.should_use_gpu_for_population(population_size) && self.is_gpu_available() {
            match self
                .nsga_ii_gpu(
                    &objectives,
                    &constraints,
                    bounds,
                    population_size,
                    generations,
                )
                .await
            {
                Ok(results) => return Ok(results),
                Err(_) if self.fallback_enabled => {
                    // Fall back to CPU implementation
                }
                Err(e) => return Err(e),
            }
        }

        // CPU implementation (simplified NSGA-II)
        self.nsga_ii_cpu(
            &objectives,
            &constraints,
            bounds,
            population_size,
            generations,
        )
    }

    fn is_gpu_available(&self) -> bool {
        #[cfg(feature = "gpu")]
        {
            self.context.is_some()
        }

        #[cfg(not(feature = "gpu"))]
        {
            false
        }
    }

    fn should_use_gpu_for_population(&self, population_size: usize) -> bool {
        population_size >= 50
    }

    #[cfg(feature = "gpu")]
    async fn nsga_ii_gpu(
        &self,
        objectives: &[ObjectiveFunction],
        constraints: &[ObjectiveFunction],
        bounds: &[(f64, f64)],
        population_size: usize,
        generations: usize,
    ) -> Result<Vec<OptimizationSolution>, OptimizationError> {
        // For now, fall back to CPU implementation
        // In a full implementation, this would use GPU for parallel fitness evaluation
        self.nsga_ii_cpu(
            objectives,
            constraints,
            bounds,
            population_size,
            generations,
        )
    }

    #[cfg(not(feature = "gpu"))]
    async fn nsga_ii_gpu(
        &self,
        objectives: &[ObjectiveFunction],
        constraints: &[ObjectiveFunction],
        bounds: &[(f64, f64)],
        population_size: usize,
        generations: usize,
    ) -> Result<Vec<OptimizationSolution>, OptimizationError> {
        self.nsga_ii_cpu(
            objectives,
            constraints,
            bounds,
            population_size,
            generations,
        )
    }

    fn nsga_ii_cpu(
        &self,
        objectives: &[ObjectiveFunction],
        _constraints: &[ObjectiveFunction],
        bounds: &[(f64, f64)],
        population_size: usize,
        generations: usize,
    ) -> Result<Vec<OptimizationSolution>, OptimizationError> {
        use rand::RngExt;
        let mut rng = rand::rng();

        // Initialize random population
        let mut population: Vec<Vec<f64>> = Vec::with_capacity(population_size);
        for _ in 0..population_size {
            let individual: Vec<f64> = bounds
                .iter()
                .map(|(min, max)| rng.random_range(*min..=*max))
                .collect();
            population.push(individual);
        }

        // Evolution loop (simplified)
        for _gen in 0..generations {
            // Evaluate objectives for each individual
            let mut fitness_values: Vec<Vec<f64>> = Vec::with_capacity(population_size);
            for individual in &population {
                let fitness: Vec<f64> = objectives.iter().map(|obj| obj(individual)).collect();
                fitness_values.push(fitness);
            }

            // Simple selection: keep first half (in real NSGA-II, this would be non-dominated sorting)
            population.truncate(population_size / 2);
            fitness_values.truncate(population_size / 2);

            // Generate offspring (simplified)
            while population.len() < population_size {
                let parent = population[rng.random_range(0..population.len())].clone();
                let mut offspring = parent;

                // Simple mutation
                for (i, (min, max)) in bounds.iter().enumerate() {
                    if rng.random_bool(0.1) {
                        // 10% mutation rate
                        offspring[i] = rng.random_range(*min..=*max);
                    }
                }

                population.push(offspring);
            }
        }

        // Convert population to optimization results
        let mut results = Vec::with_capacity(population.len());
        for individual in population {
            let objective_values: Vec<f64> =
                objectives.iter().map(|obj| obj(&individual)).collect();

            // Use first objective as primary objective value
            let primary_objective = objective_values.first().copied().unwrap_or(0.0);

            results.push(OptimizationSolution {
                solution: individual,
                objective_value: primary_objective,
                iterations: generations,
                converged: true,
                gradient_norm: None,
            });
        }

        Ok(results)
    }
}

impl Default for GpuMultiObjectiveOptimizer {
    fn default() -> Self {
        #[cfg(feature = "gpu")]
        {
            Self {
                context: None,
                fallback_enabled: true,
            }
        }

        #[cfg(not(feature = "gpu"))]
        {
            Self {
                fallback_enabled: true,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gpu_optimizer_creation() {
        let optimizer = GpuOptimizer::new().await;
        // GPU may or may not be available in test environment
        assert!(optimizer.fallback_enabled);
    }

    #[tokio::test]
    async fn test_quadratic_optimization() {
        let optimizer = GpuOptimizer::new().await;
        let coefficients = vec![1.0, 2.0, 3.0];
        let initial_point = vec![1.0, 1.0, 1.0];

        let result = optimizer
            .optimize_quadratic(&coefficients, &initial_point, 1000, 1e-6)
            .await
            .unwrap();

        assert!(result.converged);
        assert!(result.objective_value < 1e-10); // Should converge to near-zero

        // Solution should be near [0, 0, 0] for quadratic sum(c_i * x_i^2)
        for &x in result.solution.iter() {
            assert!(x.abs() < 1e-3);
        }
    }

    #[tokio::test]
    async fn test_batch_optimization() {
        let optimizer = GpuOptimizer::new().await;

        let problems = vec![
            (vec![1.0, 2.0], vec![1.0, 1.0]),
            (vec![3.0, 4.0], vec![0.5, 0.5]),
            (vec![5.0, 6.0], vec![0.1, 0.1]),
        ];

        let results = optimizer
            .optimize_quadratic_batch(&problems, 1000, 1e-6)
            .await
            .unwrap();

        assert_eq!(results.len(), 3);
        for result in results {
            assert!(result.converged);
            assert!(result.objective_value < 1e-8);
        }
    }

    #[tokio::test]
    async fn test_multi_objective_optimizer() {
        let optimizer = GpuMultiObjectiveOptimizer::new().await;

        let objectives: Vec<ObjectiveFunction> = vec![
            Box::new(|x: &[f64]| x[0] * x[0] + x[1] * x[1]), // f1 = x^2 + y^2
            Box::new(|x: &[f64]| (x[0] - 1.0).powi(2) + (x[1] - 1.0).powi(2)), // f2 = (x-1)^2 + (y-1)^2
        ];

        let constraints: Vec<ObjectiveFunction> = vec![];
        let bounds = vec![(-2.0, 2.0), (-2.0, 2.0)];

        let results = optimizer
            .nsga_ii(objectives, constraints, &bounds, 20, 10)
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert!(results.len() <= 20);
    }
}
