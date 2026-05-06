//! GPU-backed and GPU-ready measure theory operations.
//!
//! Current public behavior is intentionally explicit:
//! - `GpuIntegrator::integrate_uniform` evaluates built-in functions on the GPU and reduces on CPU.
//! - `GpuIntegrator::integrate_values` is a CPU reduction fallback for precomputed values.
//! - `GpuMonteCarloIntegrator` samples/evaluates built-in functions on the GPU and reduces on CPU.
//! - `GpuParametricDensity::gaussian_batch` is GPU-backed batch density evaluation.
//! - `GpuTropicalMeasure::{supremum, infimum}` are CPU reduction fallbacks.
//! - `GpuMultidimIntegrator::monte_carlo_nd` currently returns the exact volume for the constant-one integrand.

use crate::{GpuError, UnifiedGpuError};
use wgpu::util::DeviceExt;

/// GPU-backed numerical integrator for built-in one-dimensional functions.
///
/// `integrate_uniform` evaluates built-in functions on the GPU and performs the
/// final reduction on CPU. `integrate_values` is explicitly CPU-backed.
pub struct GpuIntegrator {
    device: wgpu::Device,
    queue: wgpu::Queue,
    integration_pipeline: wgpu::ComputePipeline,
}

impl GpuIntegrator {
    /// Create a new GPU integrator
    pub async fn new() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| GpuError::InitializationError("No GPU adapter found".to_string()))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Amari GPU Integrator"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::InitializationError(e.to_string()))?;

        // Create integration shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Integration Shader"),
            source: wgpu::ShaderSource::Wgsl(INTEGRATION_SHADER.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Integration Pipeline"),
            layout: None,
            module: &shader,
            entry_point: "integrate_riemann",
        });

        Ok(Self {
            device,
            queue,
            integration_pipeline: pipeline,
        })
    }

    /// Integrate a built-in function over `[a, b]` using a midpoint Riemann sum.
    ///
    /// The function is evaluated at `n` uniformly spaced midpoint samples on the
    /// GPU and reduced on CPU after readback. For custom/precomputed function
    /// values, use `integrate_values`, which is a CPU reduction fallback.
    ///
    /// Built-in `function_id` values:
    /// - `0`: `x`
    /// - `1`: `x²`
    /// - `2`: `x³`
    /// - `3`: `sin(x)`
    /// - `4`: `cos(x)`
    /// - `5`: `exp(x)`
    /// - any other value: `1`
    pub async fn integrate_uniform(
        &self,
        a: f32,
        b: f32,
        n: u32,
        function_id: u32,
    ) -> Result<f32, UnifiedGpuError> {
        if n == 0 {
            return Err(UnifiedGpuError::InvalidOperation(
                "Cannot integrate with zero sample points".to_string(),
            ));
        }

        // Create buffers for integration parameters
        let params = IntegrationParams {
            lower_bound: a,
            upper_bound: b,
            num_points: n,
            function_id,
        };

        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Integration Parameters"),
                contents: bytemuck::cast_slice(&[params]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // Create output buffer for all n results (one per thread)
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Integration Output"),
            size: (n * 4) as u64, // n floats, 4 bytes each
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create staging buffer for readback
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: (n * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.integration_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Integration Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Execute compute pass
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Integration Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Integration Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.integration_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            // Dispatch enough workgroups for n threads (256 threads per workgroup)
            let workgroup_count = n.div_ceil(256);
            compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
        }

        // Copy results to staging buffer
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, (n * 4) as u64);

        self.queue.submit(Some(encoder.finish()));

        // Read back results
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .await
            .map_err(|e| {
                UnifiedGpuError::InvalidOperation(format!("Buffer mapping failed: {}", e))
            })?
            .map_err(|e| {
                UnifiedGpuError::InvalidOperation(format!("Buffer mapping failed: {}", e))
            })?;

        // Sum all results
        let data = buffer_slice.get_mapped_range();
        let results: &[f32] = bytemuck::cast_slice(&data);
        let total_sum: f32 = results.iter().sum();

        drop(data);
        staging_buffer.unmap();

        // Multiply by dx for Riemann sum: ∫f(x)dx ≈ Σf(xᵢ)·Δx
        let dx = (b - a) / n as f32;
        Ok(total_sum * dx)
    }

    /// Integrate pre-computed function values with CPU reduction.
    ///
    /// This method is useful for custom functions computed on CPU or in tests.
    /// It is intentionally documented as CPU-backed until a validated GPU
    /// reduction kernel is added.
    pub async fn integrate_values(&self, values: &[f32], dx: f32) -> Result<f32, UnifiedGpuError> {
        let sum: f32 = values.iter().sum();
        Ok(sum * dx)
    }
}

/// Parameters for GPU integration
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct IntegrationParams {
    lower_bound: f32,
    upper_bound: f32,
    num_points: u32,
    function_id: u32, // ID for built-in test functions
}

/// GPU-backed Monte Carlo integrator for built-in one-dimensional functions.
///
/// Sampling and built-in function evaluation run on the GPU; the final average
/// is reduced on CPU after readback.
pub struct GpuMonteCarloIntegrator {
    device: wgpu::Device,
    queue: wgpu::Queue,
    monte_carlo_pipeline: wgpu::ComputePipeline,
}

impl GpuMonteCarloIntegrator {
    /// Create a new Monte Carlo integrator
    pub async fn new() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| GpuError::InitializationError("No GPU adapter found".to_string()))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Monte Carlo GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::InitializationError(e.to_string()))?;

        // Create Monte Carlo shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Monte Carlo Shader"),
            source: wgpu::ShaderSource::Wgsl(MONTE_CARLO_SHADER.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Monte Carlo Pipeline"),
            layout: None,
            module: &shader,
            entry_point: "monte_carlo_integrate",
        });

        Ok(Self {
            device,
            queue,
            monte_carlo_pipeline: pipeline,
        })
    }

    /// Compute `E[X]` for `X ~ Uniform(a, b)`.
    ///
    /// Uses Monte Carlo sampling with `n` samples evaluated on the GPU and a CPU
    /// readback reduction. Use `integrate` for the other built-in functions.
    pub async fn expectation_uniform(
        &self,
        a: f32,
        b: f32,
        n: u32,
        seed: u32,
    ) -> Result<f32, UnifiedGpuError> {
        self.expectation_uniform_for_function(a, b, n, seed, 0)
            .await
    }

    async fn expectation_uniform_for_function(
        &self,
        a: f32,
        b: f32,
        n: u32,
        seed: u32,
        function_id: u32,
    ) -> Result<f32, UnifiedGpuError> {
        if n == 0 {
            return Err(UnifiedGpuError::InvalidOperation(
                "Cannot run Monte Carlo with zero samples".to_string(),
            ));
        }

        // Create parameters buffer
        let params = MonteCarloParams {
            lower_bound: a,
            upper_bound: b,
            num_samples: n,
            seed,
            function_id,
            _padding: [0; 3],
        };

        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Monte Carlo Parameters"),
                contents: bytemuck::cast_slice(&[params]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // Create output buffer for results
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Monte Carlo Output"),
            size: (n * 4) as u64, // n floats
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create staging buffer
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: (n * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.monte_carlo_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Monte Carlo Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Execute compute pass
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Monte Carlo Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Monte Carlo Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.monte_carlo_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            let workgroup_count = n.div_ceil(256);
            compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
        }

        // Copy results to staging buffer
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, (n * 4) as u64);

        self.queue.submit(Some(encoder.finish()));

        // Read back results
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .await
            .map_err(|e| {
                UnifiedGpuError::InvalidOperation(format!("Buffer mapping failed: {}", e))
            })?
            .map_err(|e| {
                UnifiedGpuError::InvalidOperation(format!("Buffer mapping failed: {}", e))
            })?;

        // Sum all results
        let data = buffer_slice.get_mapped_range();
        let results: &[f32] = bytemuck::cast_slice(&data);
        let total_sum: f32 = results.iter().sum();

        drop(data);
        staging_buffer.unmap();

        // Return average
        Ok(total_sum / n as f32)
    }

    /// Monte Carlo integration of a built-in function.
    ///
    /// Computes `∫_a^b f(x) dx` using GPU Monte Carlo sampling/evaluation and a
    /// CPU readback reduction. Uses the same built-in `function_id` mapping as
    /// `GpuIntegrator::integrate_uniform`.
    pub async fn integrate(
        &self,
        a: f32,
        b: f32,
        n: u32,
        seed: u32,
        function_id: u32,
    ) -> Result<f32, UnifiedGpuError> {
        let expectation = self
            .expectation_uniform_for_function(a, b, n, seed, function_id)
            .await?;
        // Monte Carlo integral: (b - a) * E[f(X)]
        Ok((b - a) * expectation)
    }
}

/// Parameters for Monte Carlo integration
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MonteCarloParams {
    lower_bound: f32,
    upper_bound: f32,
    num_samples: u32,
    seed: u32,
    function_id: u32,
    _padding: [u32; 3],
}

/// WGSL shader for numerical integration
const INTEGRATION_SHADER: &str = r#"
struct IntegrationParams {
    lower_bound: f32,
    upper_bound: f32,
    num_points: u32,
    function_id: u32,
}

@group(0) @binding(0)
var<uniform> params: IntegrationParams;

@group(0) @binding(1)
var<storage, read_write> results: array<f32>;

// Built-in test functions
fn evaluate_function(x: f32, function_id: u32) -> f32 {
    switch function_id {
        case 0u: { return x; }           // f(x) = x
        case 1u: { return x * x; }       // f(x) = x²
        case 2u: { return x * x * x; }   // f(x) = x³
        case 3u: { return sin(x); }      // f(x) = sin(x)
        case 4u: { return cos(x); }      // f(x) = cos(x)
        case 5u: { return exp(x); }      // f(x) = exp(x)
        default: { return 1.0; }         // f(x) = 1 (constant)
    }
}

@compute @workgroup_size(256)
fn integrate_riemann(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let thread_id = global_id.x;

    if thread_id >= params.num_points {
        return;
    }

    // Compute sample point (midpoint rule)
    let dx = (params.upper_bound - params.lower_bound) / f32(params.num_points);
    let x = params.lower_bound + (f32(thread_id) + 0.5) * dx;

    // Evaluate function and store result
    // Each thread writes to its own location - no atomics needed
    results[thread_id] = evaluate_function(x, params.function_id);
}
"#;

/// WGSL shader for Monte Carlo integration with PCG random number generator
const MONTE_CARLO_SHADER: &str = r#"
struct MonteCarloParams {
    lower_bound: f32,
    upper_bound: f32,
    num_samples: u32,
    seed: u32,
    function_id: u32,
    padding0: u32,
    padding1: u32,
    padding2: u32,
}

@group(0) @binding(0)
var<uniform> params: MonteCarloParams;

@group(0) @binding(1)
var<storage, read_write> results: array<f32>;

// PCG (Permuted Congruential Generator) random number generator
// Based on O'Neill (2014) - fast, high-quality PRNG suitable for GPU
fn pcg_hash(input: u32) -> u32 {
    var state = input * 747796405u + 2891336453u;
    var word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

// Generate random float in [0, 1) from thread ID and iteration
fn random_f32(thread_id: u32, iteration: u32) -> f32 {
    let hash = pcg_hash(thread_id + iteration * 1000000u + params.seed);
    return f32(hash) / 4294967296.0; // 2^32
}

// Built-in test functions
fn evaluate_function(x: f32, function_id: u32) -> f32 {
    switch function_id {
        case 0u: { return x; }           // f(x) = x
        case 1u: { return x * x; }       // f(x) = x²
        case 2u: { return x * x * x; }   // f(x) = x³
        case 3u: { return sin(x); }      // f(x) = sin(x)
        case 4u: { return cos(x); }      // f(x) = cos(x)
        case 5u: { return exp(x); }      // f(x) = exp(x)
        default: { return 1.0; }         // f(x) = 1 (constant)
    }
}

@compute @workgroup_size(256)
fn monte_carlo_integrate(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let thread_id = global_id.x;

    if thread_id >= params.num_samples {
        return;
    }

    // Generate random sample point in [a, b]
    let rand_val = random_f32(thread_id, 0u);
    let x = params.lower_bound + rand_val * (params.upper_bound - params.lower_bound);

    // Evaluate built-in function at random point
    let y = evaluate_function(x, params.function_id);

    // Store result (will be averaged by CPU)
    results[thread_id] = y;
}
"#;

/// GPU-backed parametric density batch evaluation.
pub struct GpuParametricDensity {
    device: wgpu::Device,
    queue: wgpu::Queue,
    density_pipeline: wgpu::ComputePipeline,
}

impl GpuParametricDensity {
    /// Create new GPU parametric density evaluator
    pub async fn new() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| GpuError::InitializationError("No GPU adapter found".to_string()))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Parametric Density GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::InitializationError(e.to_string()))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Parametric Density Shader"),
            source: wgpu::ShaderSource::Wgsl(PARAMETRIC_DENSITY_SHADER.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Parametric Density Pipeline"),
            layout: None,
            module: &shader,
            entry_point: "evaluate_density_batch",
        });

        Ok(Self {
            device,
            queue,
            density_pipeline: pipeline,
        })
    }

    /// Batch evaluate Gaussian density on the GPU.
    ///
    /// Evaluates `N(x | μ, σ²)` for many data points in parallel.
    pub async fn gaussian_batch(
        &self,
        data: &[f32],
        mu: f32,
        sigma: f32,
    ) -> Result<Vec<f32>, UnifiedGpuError> {
        if sigma <= 0.0 {
            return Err(UnifiedGpuError::InvalidOperation(
                "Gaussian sigma must be positive".to_string(),
            ));
        }
        if data.is_empty() {
            return Ok(Vec::new());
        }

        self.evaluate_batch(data, &[mu, sigma], 0).await
    }

    /// Batch evaluate any density (internal implementation)
    async fn evaluate_batch(
        &self,
        data: &[f32],
        params: &[f32],
        _density_type: u32,
    ) -> Result<Vec<f32>, UnifiedGpuError> {
        let n = data.len();

        // Upload data and parameters
        let data_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Data Buffer"),
                contents: bytemuck::cast_slice(data),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Parameters Buffer"),
                contents: bytemuck::cast_slice(params),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Density Output"),
            size: (n * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: (n * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.density_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Density Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: data_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Execute
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Density Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Density Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.density_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            let workgroup_count = (n as u32).div_ceil(256);
            compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, (n * 4) as u64);

        self.queue.submit(Some(encoder.finish()));

        // Read back
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .await
            .map_err(|e| {
                UnifiedGpuError::InvalidOperation(format!("Buffer mapping failed: {}", e))
            })?
            .map_err(|e| {
                UnifiedGpuError::InvalidOperation(format!("Buffer mapping failed: {}", e))
            })?;

        let data_slice = buffer_slice.get_mapped_range();
        let results: Vec<f32> = bytemuck::cast_slice(&data_slice).to_vec();

        drop(data_slice);
        staging_buffer.unmap();

        Ok(results)
    }
}

/// GPU-ready tropical measure reductions.
///
/// Current 0.20.0 behavior uses CPU reductions while GPU reduction kernels are
/// pending validation.
pub struct GpuTropicalMeasure {
    _device: wgpu::Device,
    _queue: wgpu::Queue,
}

impl GpuTropicalMeasure {
    /// Create new GPU tropical measure
    pub async fn new() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| GpuError::InitializationError("No GPU adapter found".to_string()))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Tropical Measure GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::InitializationError(e.to_string()))?;

        Ok(Self {
            _device: device,
            _queue: queue,
        })
    }

    /// Compute supremum (max) using CPU reduction.
    pub async fn supremum(&self, values: &[f32]) -> Result<f32, UnifiedGpuError> {
        if values.is_empty() {
            return Err(UnifiedGpuError::InvalidOperation(
                "Cannot compute supremum of empty array".to_string(),
            ));
        }

        // TODO(0.20.x+): replace with validated GPU reduction kernel.
        Ok(*values
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap())
    }

    /// Compute infimum (min) using CPU reduction.
    pub async fn infimum(&self, values: &[f32]) -> Result<f32, UnifiedGpuError> {
        if values.is_empty() {
            return Err(UnifiedGpuError::InvalidOperation(
                "Cannot compute infimum of empty array".to_string(),
            ));
        }

        Ok(*values
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap())
    }
}

/// GPU-ready multidimensional integration scaffolding.
///
/// Current 0.20.0 behavior is limited to the exact hypercube volume, i.e. the
/// integral of the constant-one function over the supplied bounds.
pub struct GpuMultidimIntegrator {
    _device: wgpu::Device,
    _queue: wgpu::Queue,
}

impl GpuMultidimIntegrator {
    /// Create new multidimensional integrator
    pub async fn new() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| GpuError::InitializationError("No GPU adapter found".to_string()))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Multidim Integration GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::InitializationError(e.to_string()))?;

        Ok(Self {
            _device: device,
            _queue: queue,
        })
    }

    /// Return the exact hypercube volume for the constant-one integrand.
    ///
    /// `num_samples` and `seed` are accepted for API continuity but are not used
    /// until multidimensional GPU Monte Carlo kernels are implemented.
    pub async fn monte_carlo_nd(
        &self,
        bounds: &[(f32, f32)],
        _num_samples: u32,
        _seed: u32,
    ) -> Result<f32, UnifiedGpuError> {
        // Compute volume of integration region
        let volume: f32 = bounds.iter().map(|(a, b)| b - a).product();

        // TODO(0.20.x+): add validated multidimensional GPU Monte Carlo kernels.
        Ok(volume)
    }
}

/// WGSL shader for parametric density evaluation
const PARAMETRIC_DENSITY_SHADER: &str = r#"
@group(0) @binding(0)
var<storage, read> data: array<f32>;

@group(0) @binding(1)
var<storage, read> params: array<f32>;

@group(0) @binding(2)
var<storage, read_write> output: array<f32>;

// Gaussian density N(x | μ, σ²)
fn gaussian_density(x: f32, mu: f32, sigma: f32) -> f32 {
    let z = (x - mu) / sigma;
    let normalization = 1.0 / (sigma * sqrt(6.28318530718)); // sqrt(2π)
    return normalization * exp(-0.5 * z * z);
}

@compute @workgroup_size(256)
fn evaluate_density_batch(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let idx = global_id.x;

    if idx >= arrayLength(&data) {
        return;
    }

    let x = data[idx];
    let mu = params[0];
    let sigma = params[1];

    output[idx] = gaussian_density(x, mu, sigma);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gpu_integration_constant() {
        // Skip GPU tests in CI environments where GPU is not available
        if std::env::var("CI").is_ok()
            || std::env::var("GITHUB_ACTIONS").is_ok()
            || std::env::var("DISPLAY").is_err()
        {
            println!("Skipping GPU test in CI environment");
            return;
        }

        // This test will fail if no GPU is available, which is expected in CI
        match GpuIntegrator::new().await {
            Ok(integrator) => {
                // Integrate f(x) = 1 over [0, 10]
                // Expected: 10
                let result = integrator
                    .integrate_uniform(0.0, 10.0, 10000, 6)
                    .await
                    .unwrap();
                assert!((result - 10.0).abs() < 0.01);
            }
            Err(GpuError::InitializationError(_)) => {
                // No GPU available - this is fine
                println!("GPU initialization failed - no GPU available");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_gpu_integration_linear() {
        // Skip GPU tests in CI environments where GPU is not available
        if std::env::var("CI").is_ok()
            || std::env::var("GITHUB_ACTIONS").is_ok()
            || std::env::var("DISPLAY").is_err()
        {
            println!("Skipping GPU test in CI environment");
            return;
        }

        // This test will fail if no GPU is available, which is expected in CI
        match GpuIntegrator::new().await {
            Ok(integrator) => {
                // Integrate f(x) = x over [0, 2]
                // Expected: 2
                let result = integrator
                    .integrate_uniform(0.0, 2.0, 10000, 0)
                    .await
                    .unwrap();
                assert!((result - 2.0).abs() < 0.01);
            }
            Err(GpuError::InitializationError(_)) => {
                // No GPU available - this is fine
                println!("GPU initialization failed - no GPU available");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_gpu_integration_quadratic() {
        // Skip GPU tests in CI environments where GPU is not available
        if std::env::var("CI").is_ok()
            || std::env::var("GITHUB_ACTIONS").is_ok()
            || std::env::var("DISPLAY").is_err()
        {
            println!("Skipping GPU test in CI environment");
            return;
        }

        // This test will fail if no GPU is available, which is expected in CI
        match GpuIntegrator::new().await {
            Ok(integrator) => {
                // Integrate f(x) = x² over [0, 2]
                // Expected: 8/3 ≈ 2.667
                let result = integrator
                    .integrate_uniform(0.0, 2.0, 10000, 1)
                    .await
                    .unwrap();
                assert!((result - 8.0 / 3.0).abs() < 0.01);
            }
            Err(GpuError::InitializationError(_)) => {
                // No GPU available - this is fine
                println!("GPU initialization failed - no GPU available");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
}
