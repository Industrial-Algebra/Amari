//! GPU-ready differential calculus operations.
//!
//! This module provides the public `amari-gpu` calculus API and initializes the
//! WebGPU pipeline scaffolding for future calculus kernels. The current public
//! behavior is intentionally honest: calculus operations preserve CPU semantics
//! and fall back to CPU computation while the WGSL kernels are still being
//! completed and validated.
//!
//! ## Current Operations
//!
//! - **Batch Field Evaluation**: CPU-compatible batch evaluation through the GPU API
//! - **Batch Gradient**: CPU finite-difference baseline through the GPU API
//! - **Batch Divergence**: CPU finite-difference baseline through the GPU API
//! - **Batch Curl**: CPU finite-difference baseline through the GPU API

use crate::unified::{GpuContext, UnifiedGpuResult};
use amari_calculus::{ScalarField, VectorField};
use amari_core::Multivector;

/// GPU-ready differential calculus operations.
///
/// Current public methods preserve `amari-calculus` CPU semantics while GPU
/// kernels are completed and validated. The type still owns GPU pipeline
/// scaffolding so future kernels can be added without changing the public API.
pub struct GpuCalculus {
    #[allow(dead_code)] // Used for future GPU shader implementation
    context: GpuContext,
    #[allow(dead_code)] // Used for future GPU shader implementation
    gradient_pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)] // Used for future GPU shader implementation
    divergence_pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)] // Used for future GPU shader implementation
    field_eval_pipeline: wgpu::ComputePipeline,
}

impl GpuCalculus {
    /// Initialize GPU context for calculus operations
    pub async fn new() -> UnifiedGpuResult<Self> {
        let context = GpuContext::new().await?;

        // Create compute pipelines
        let gradient_pipeline = Self::create_gradient_pipeline(&context.device)?;
        let divergence_pipeline = Self::create_divergence_pipeline(&context.device)?;
        let field_eval_pipeline = Self::create_field_eval_pipeline(&context.device)?;

        Ok(Self {
            context,
            gradient_pipeline,
            divergence_pipeline,
            field_eval_pipeline,
        })
    }

    /// Batch evaluate a scalar field at multiple points.
    ///
    /// Current 0.20.0 behavior: CPU-semantic fallback through the GPU API.
    /// Future releases may route large batches to a validated WGSL kernel.
    pub async fn batch_eval_scalar_field<const P: usize, const Q: usize, const R: usize>(
        &self,
        field: &ScalarField<P, Q, R>,
        points: &[[f64; 3]],
    ) -> UnifiedGpuResult<Vec<f64>> {
        self.eval_scalar_field_gpu(field, points).await
    }

    /// Batch evaluate a vector field at multiple points.
    ///
    /// Current 0.20.0 behavior: CPU-semantic fallback through the GPU API.
    /// Future releases may route large batches to a validated WGSL kernel.
    pub async fn batch_eval_vector_field<const P: usize, const Q: usize, const R: usize>(
        &self,
        field: &VectorField<P, Q, R>,
        points: &[[f64; 3]],
    ) -> UnifiedGpuResult<Vec<Multivector<P, Q, R>>> {
        self.eval_vector_field_gpu(field, points).await
    }

    /// Batch compute gradients at multiple points.
    ///
    /// Current 0.20.0 behavior: CPU central finite differences through the GPU API.
    /// Future releases may route large batches to a validated WGSL kernel.
    pub async fn batch_gradient<const P: usize, const Q: usize, const R: usize>(
        &self,
        field: &ScalarField<P, Q, R>,
        points: &[[f64; 3]],
        h: f64,
    ) -> UnifiedGpuResult<Vec<Multivector<P, Q, R>>> {
        self.compute_gradient_gpu(field, points, h).await
    }

    /// Batch compute divergence of a vector field at multiple points.
    ///
    /// Current 0.20.0 behavior: CPU central finite differences through the GPU API.
    /// Future releases may route large batches to a validated WGSL kernel.
    pub async fn batch_divergence<const P: usize, const Q: usize, const R: usize>(
        &self,
        field: &VectorField<P, Q, R>,
        points: &[[f64; 3]],
        h: f64,
    ) -> UnifiedGpuResult<Vec<f64>> {
        self.compute_divergence_gpu(field, points, h).await
    }

    /// Batch compute curl of a vector field at multiple points.
    ///
    /// Current 0.20.0 behavior: CPU central finite differences through the GPU API.
    /// Future releases may route large batches to a validated WGSL kernel.
    pub async fn batch_curl<const P: usize, const Q: usize, const R: usize>(
        &self,
        field: &VectorField<P, Q, R>,
        points: &[[f64; 3]],
        h: f64,
    ) -> UnifiedGpuResult<Vec<Multivector<P, Q, R>>> {
        self.compute_curl_gpu(field, points, h).await
    }

    // ==================================================================
    // CPU FALLBACK IMPLEMENTATIONS
    // ==================================================================

    fn compute_gradient_cpu<const P: usize, const Q: usize, const R: usize>(
        &self,
        field: &ScalarField<P, Q, R>,
        point: &[f64; 3],
        h: f64,
    ) -> Multivector<P, Q, R> {
        let mut grad = Multivector::zero();

        // Central finite differences for each component
        for i in 0..3 {
            let mut p_plus = *point;
            let mut p_minus = *point;
            p_plus[i] += h;
            p_minus[i] -= h;

            let derivative = (field.evaluate(&p_plus) - field.evaluate(&p_minus)) / (2.0 * h);
            grad.set_vector_component(i, derivative);
        }

        grad
    }

    fn compute_divergence_cpu<const P: usize, const Q: usize, const R: usize>(
        &self,
        field: &VectorField<P, Q, R>,
        point: &[f64; 3],
        h: f64,
    ) -> f64 {
        let mut div = 0.0;

        // ∇·F = ∂Fx/∂x + ∂Fy/∂y + ∂Fz/∂z
        for i in 0..3 {
            let mut p_plus = *point;
            let mut p_minus = *point;
            p_plus[i] += h;
            p_minus[i] -= h;

            let f_plus = field.evaluate(&p_plus);
            let f_minus = field.evaluate(&p_minus);

            let derivative = (f_plus.vector_component(i) - f_minus.vector_component(i)) / (2.0 * h);
            div += derivative;
        }

        div
    }

    fn compute_curl_cpu<const P: usize, const Q: usize, const R: usize>(
        &self,
        field: &VectorField<P, Q, R>,
        point: &[f64; 3],
        h: f64,
    ) -> Multivector<P, Q, R> {
        // ∇∧F = (∂Fz/∂y - ∂Fy/∂z)e_x + (∂Fx/∂z - ∂Fz/∂x)e_y + (∂Fy/∂x - ∂Fx/∂y)e_z
        let mut derivatives = [[0.0; 3]; 3]; // derivatives[i][j] = ∂Fi/∂xj

        for (i, derivative_row) in derivatives.iter_mut().enumerate() {
            for (j, derivative_elem) in derivative_row.iter_mut().enumerate() {
                let mut p_plus = *point;
                let mut p_minus = *point;
                p_plus[j] += h;
                p_minus[j] -= h;

                let f_plus = field.evaluate(&p_plus);
                let f_minus = field.evaluate(&p_minus);

                *derivative_elem =
                    (f_plus.vector_component(i) - f_minus.vector_component(i)) / (2.0 * h);
            }
        }

        // In 3D, curl is represented as a bivector (wedge product result)
        let mut curl = Multivector::zero();

        // Curl components as bivector
        // curl_x = ∂Fz/∂y - ∂Fy/∂z  →  e_2 ∧ e_3
        // curl_y = ∂Fx/∂z - ∂Fz/∂x  →  e_3 ∧ e_1
        // curl_z = ∂Fy/∂x - ∂Fx/∂y  →  e_1 ∧ e_2

        // For 3D geometric algebra, bivectors are indices 3,4,5 (e_12, e_13, e_23)
        curl.set_bivector_component(0, derivatives[1][2] - derivatives[2][1]); // e_1∧e_2
        curl.set_bivector_component(1, derivatives[2][0] - derivatives[0][2]); // e_1∧e_3
        curl.set_bivector_component(2, derivatives[0][1] - derivatives[1][0]); // e_2∧e_3

        curl
    }

    // ==================================================================
    // CPU-SEMANTIC FALLBACKS FOR FUTURE GPU IMPLEMENTATIONS
    // ==================================================================

    async fn eval_scalar_field_gpu<const P: usize, const Q: usize, const R: usize>(
        &self,
        field: &ScalarField<P, Q, R>,
        points: &[[f64; 3]],
    ) -> UnifiedGpuResult<Vec<f64>> {
        // TODO(0.20.x+): replace with validated WGSL field-evaluation kernel.
        Ok(points.iter().map(|p| field.evaluate(p)).collect())
    }

    async fn eval_vector_field_gpu<const P: usize, const Q: usize, const R: usize>(
        &self,
        field: &VectorField<P, Q, R>,
        points: &[[f64; 3]],
    ) -> UnifiedGpuResult<Vec<Multivector<P, Q, R>>> {
        // TODO(0.20.x+): replace with validated WGSL vector-field kernel.
        Ok(points.iter().map(|p| field.evaluate(p)).collect())
    }

    async fn compute_gradient_gpu<const P: usize, const Q: usize, const R: usize>(
        &self,
        field: &ScalarField<P, Q, R>,
        points: &[[f64; 3]],
        h: f64,
    ) -> UnifiedGpuResult<Vec<Multivector<P, Q, R>>> {
        // TODO(0.20.x+): replace with validated WGSL finite-difference kernel.
        Ok(points
            .iter()
            .map(|p| self.compute_gradient_cpu(field, p, h))
            .collect())
    }

    async fn compute_divergence_gpu<const P: usize, const Q: usize, const R: usize>(
        &self,
        field: &VectorField<P, Q, R>,
        points: &[[f64; 3]],
        h: f64,
    ) -> UnifiedGpuResult<Vec<f64>> {
        // TODO(0.20.x+): replace with validated WGSL divergence kernel.
        Ok(points
            .iter()
            .map(|p| self.compute_divergence_cpu(field, p, h))
            .collect())
    }

    async fn compute_curl_gpu<const P: usize, const Q: usize, const R: usize>(
        &self,
        field: &VectorField<P, Q, R>,
        points: &[[f64; 3]],
        h: f64,
    ) -> UnifiedGpuResult<Vec<Multivector<P, Q, R>>> {
        // TODO(0.20.x+): replace with validated WGSL curl kernel.
        Ok(points
            .iter()
            .map(|p| self.compute_curl_cpu(field, p, h))
            .collect())
    }

    // ==================================================================
    // PIPELINE CREATION
    // ==================================================================

    fn create_gradient_pipeline(device: &wgpu::Device) -> UnifiedGpuResult<wgpu::ComputePipeline> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Gradient Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(GRADIENT_SHADER)),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Gradient Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            }),
        )
    }

    fn create_divergence_pipeline(
        device: &wgpu::Device,
    ) -> UnifiedGpuResult<wgpu::ComputePipeline> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Divergence Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(DIVERGENCE_SHADER)),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Divergence Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            }),
        )
    }

    fn create_field_eval_pipeline(
        device: &wgpu::Device,
    ) -> UnifiedGpuResult<wgpu::ComputePipeline> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Field Evaluation Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(FIELD_EVAL_SHADER)),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Field Evaluation Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            }),
        )
    }
}

// ==================================================================
// WGSL COMPUTE SHADERS
// ==================================================================

/// WGSL shader for batch gradient computation
const GRADIENT_SHADER: &str = r#"
@group(0) @binding(0)
var<storage, read> points: array<vec3<f32>>;

@group(0) @binding(1)
var<storage, read_write> gradients: array<vec3<f32>>;

@group(0) @binding(2)
var<uniform> params: ComputeParams;

struct ComputeParams {
    h: f32,
    batch_size: u32,
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= params.batch_size) {
        return;
    }

    let point = points[idx];
    var grad = vec3<f32>(0.0, 0.0, 0.0);

    // Central finite differences for each component
    // TODO: Implement field evaluation (requires passing field function/data)

    gradients[idx] = grad;
}
"#;

/// WGSL shader for batch divergence computation
const DIVERGENCE_SHADER: &str = r#"
@group(0) @binding(0)
var<storage, read> points: array<vec3<f32>>;

@group(0) @binding(1)
var<storage, read_write> divergences: array<f32>;

@group(0) @binding(2)
var<uniform> params: ComputeParams;

struct ComputeParams {
    h: f32,
    batch_size: u32,
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= params.batch_size) {
        return;
    }

    // Compute divergence using finite differences
    // TODO: Implement vector field evaluation

    divergences[idx] = 0.0;
}
"#;

/// WGSL shader for batch field evaluation
const FIELD_EVAL_SHADER: &str = r#"
@group(0) @binding(0)
var<storage, read> points: array<vec3<f32>>;

@group(0) @binding(1)
var<storage, read_write> values: array<f32>;

@group(0) @binding(2)
var<uniform> params: ComputeParams;

struct ComputeParams {
    batch_size: u32,
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= params.batch_size) {
        return;
    }

    let point = points[idx];

    // TODO: Evaluate field at point
    // This requires passing field coefficients or evaluating on GPU

    values[idx] = 0.0;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gpu_calculus_creation() {
        // Skip GPU tests in CI environments
        if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
            println!("Skipping GPU test in CI environment");
            return;
        }

        match GpuCalculus::new().await {
            Ok(_gpu_calc) => println!("GPU calculus initialized successfully"),
            Err(e) => println!("GPU initialization failed (expected in CI): {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_batch_gradient_cpu_fallback() {
        // Test CPU fallback path with small batch
        let field = ScalarField::<3, 0, 0>::new(|coords| coords[0].powi(2) + coords[1].powi(2));

        let points = vec![[1.0, 1.0, 0.0], [2.0, 2.0, 0.0]];

        // This should use CPU fallback (batch_size < 500)
        if let Ok(gpu_calc) = GpuCalculus::new().await {
            let gradients = gpu_calc.batch_gradient(&field, &points, 1e-5).await;
            match gradients {
                Ok(grads) => {
                    assert_eq!(grads.len(), 2);
                    // Gradient of x² + y² at (1,1) should be approximately (2, 2, 0)
                    assert!((grads[0].vector_component(0) - 2.0).abs() < 0.01);
                    assert!((grads[0].vector_component(1) - 2.0).abs() < 0.01);
                }
                Err(_) => println!("GPU not available, skipping gradient test"),
            }
        }
    }
}
