//! GPU-accelerated probabilistic operations
//!
//! This module provides GPU acceleration for probability operations on
//! geometric algebra spaces, including:
//!
//! - **Batch sampling**: Draw many samples in parallel
//! - **Monte Carlo integration**: GPU-parallel estimators
//! - **MCMC chains**: Parallel chain execution
//! - **SDE simulation**: Parallel path generation
//!
//! # Example
//!
//! ```ignore
//! use amari_gpu::GpuProbabilistic;
//!
//! // Create GPU context
//! let gpu = GpuProbabilistic::new(256).await?;
//!
//! // Batch sample from Gaussian
//! let samples = gpu.batch_sample_gaussian(1000, &mean, &variance).await?;
//!
//! // Monte Carlo integration
//! let estimate = gpu.monte_carlo_mean(&samples).await?;
//! ```
//!
//! # Performance
//!
//! GPU acceleration provides significant speedups for:
//! - Batch sizes > 100 samples
//! - Monte Carlo with > 1000 samples
//! - Multiple MCMC chains (> 4)
//!
//! For smaller operations, CPU fallback is used automatically after GPU context
//! creation. Public methods validate dimensions, finite values, sample counts,
//! and distribution parameters before dispatch.

use thiserror::Error;
use wgpu::util::DeviceExt;

/// Errors specific to GPU probabilistic operations
#[derive(Error, Debug)]
pub enum GpuProbabilisticError {
    /// GPU device initialization failed
    #[error("GPU initialization failed: {0}")]
    InitializationFailed(String),

    /// Dimension mismatch in batch operations
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Invalid parameters for distribution
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    /// GPU computation failed
    #[error("GPU computation failed: {0}")]
    ComputationFailed(String),

    /// Buffer allocation failed
    #[error("Buffer allocation failed: {0}")]
    BufferAllocationFailed(String),
}

/// Result type for GPU probabilistic operations
pub type GpuProbabilisticResult<T> = std::result::Result<T, GpuProbabilisticError>;

/// GPU-accelerated probabilistic operations
///
/// Provides batch operations for sampling and Monte Carlo estimation
/// on multivector spaces.
pub struct GpuProbabilistic {
    device: wgpu::Device,
    queue: wgpu::Queue,
    sample_pipeline: wgpu::ComputePipeline,
    mean_pipeline: wgpu::ComputePipeline,
    variance_pipeline: wgpu::ComputePipeline,
    dimension: usize,
}

impl GpuProbabilistic {
    /// Create a new GPU probabilistic context.
    ///
    /// # Arguments
    /// * `dimension` - Dimension of the multivector space (2^n for Cl(n,0,0)); must be non-zero.
    pub async fn new(dimension: usize) -> GpuProbabilisticResult<Self> {
        if dimension == 0 {
            return Err(GpuProbabilisticError::InvalidParameters(
                "dimension must be greater than zero".to_string(),
            ));
        }

        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| {
                GpuProbabilisticError::InitializationFailed("No suitable GPU adapter".to_string())
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Probabilistic GPU"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuProbabilisticError::InitializationFailed(e.to_string()))?;

        let sample_pipeline = Self::create_sample_pipeline(&device, dimension)?;
        let mean_pipeline = Self::create_mean_pipeline(&device, dimension)?;
        let variance_pipeline = Self::create_variance_pipeline(&device, dimension)?;

        Ok(Self {
            device,
            queue,
            sample_pipeline,
            mean_pipeline,
            variance_pipeline,
            dimension,
        })
    }

    /// Get the dimension of the multivector space
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    fn validate_distribution_inputs(
        &self,
        mean: &[f64],
        std_dev: &[f64],
    ) -> GpuProbabilisticResult<()> {
        if mean.len() != self.dimension {
            return Err(GpuProbabilisticError::DimensionMismatch {
                expected: self.dimension,
                actual: mean.len(),
            });
        }
        if std_dev.len() != self.dimension {
            return Err(GpuProbabilisticError::DimensionMismatch {
                expected: self.dimension,
                actual: std_dev.len(),
            });
        }
        if mean.iter().any(|x| !x.is_finite()) {
            return Err(GpuProbabilisticError::InvalidParameters(
                "mean values must be finite".to_string(),
            ));
        }
        if std_dev.iter().any(|x| !x.is_finite() || *x < 0.0) {
            return Err(GpuProbabilisticError::InvalidParameters(
                "standard deviations must be finite and non-negative".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_samples(&self, samples: &[f64]) -> GpuProbabilisticResult<usize> {
        if samples.is_empty() {
            return Err(GpuProbabilisticError::InvalidParameters(
                "at least one sample is required".to_string(),
            ));
        }
        if !samples.len().is_multiple_of(self.dimension) {
            return Err(GpuProbabilisticError::DimensionMismatch {
                expected: self.dimension,
                actual: samples.len() % self.dimension,
            });
        }
        if samples.iter().any(|x| !x.is_finite()) {
            return Err(GpuProbabilisticError::InvalidParameters(
                "sample values must be finite".to_string(),
            ));
        }

        Ok(samples.len() / self.dimension)
    }

    /// Batch sample from a Gaussian distribution
    ///
    /// # Arguments
    /// * `num_samples` - Number of samples to draw
    /// * `mean` - Mean vector (dimension coefficients)
    /// * `std_dev` - Standard deviation per coefficient
    ///
    /// # Returns
    /// Flat array of samples: `num_samples * dimension` coefficients
    pub async fn batch_sample_gaussian(
        &self,
        num_samples: usize,
        mean: &[f64],
        std_dev: &[f64],
    ) -> GpuProbabilisticResult<Vec<f64>> {
        self.validate_distribution_inputs(mean, std_dev)?;

        // For small batches, use CPU
        if num_samples < 100 {
            return self.batch_sample_gaussian_cpu(num_samples, mean, std_dev);
        }

        self.batch_sample_gaussian_gpu(num_samples, mean, std_dev)
            .await
    }

    /// CPU fallback for Gaussian sampling
    fn batch_sample_gaussian_cpu(
        &self,
        num_samples: usize,
        mean: &[f64],
        std_dev: &[f64],
    ) -> GpuProbabilisticResult<Vec<f64>> {
        use rand::RngExt;
        use rand_distr::StandardNormal;

        let mut rng = rand::rng();
        let mut result = Vec::with_capacity(num_samples * self.dimension);

        for _ in 0..num_samples {
            for i in 0..self.dimension {
                let z: f64 = rng.sample(StandardNormal);
                result.push(mean[i] + std_dev[i] * z);
            }
        }

        Ok(result)
    }

    /// GPU implementation of Gaussian sampling
    async fn batch_sample_gaussian_gpu(
        &self,
        num_samples: usize,
        mean: &[f64],
        std_dev: &[f64],
    ) -> GpuProbabilisticResult<Vec<f64>> {
        // Convert to f32 for GPU
        let mean_f32: Vec<f32> = mean.iter().map(|&x| x as f32).collect();
        let std_dev_f32: Vec<f32> = std_dev.iter().map(|&x| x as f32).collect();

        // Generate random seeds (Box-Muller needs pairs)
        use rand::RngExt;
        let mut rng = rand::rng();
        let random_seeds: Vec<f32> = (0..num_samples * self.dimension * 2)
            .map(|_| rng.random::<f32>())
            .collect();

        // Create buffers
        let mean_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Mean Buffer"),
                contents: bytemuck::cast_slice(&mean_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let std_dev_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("StdDev Buffer"),
                contents: bytemuck::cast_slice(&std_dev_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let random_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Random Buffer"),
                contents: bytemuck::cast_slice(&random_seeds),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let output_size = (num_samples * self.dimension * std::mem::size_of::<f32>()) as u64;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Buffer"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.sample_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sample Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: mean_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: std_dev_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: random_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Execute
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Sample Encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Sample Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.sample_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let workgroups = (num_samples * self.dimension).div_ceil(256);
            pass.dispatch_workgroups(workgroups as u32, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);
        self.queue.submit(Some(encoder.finish()));

        // Read back
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .await
            .map_err(|_| GpuProbabilisticError::ComputationFailed("Map cancelled".to_string()))?
            .map_err(|e| GpuProbabilisticError::ComputationFailed(e.to_string()))?;

        let data = buffer_slice.get_mapped_range();
        let result_f32: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging_buffer.unmap();

        Ok(result_f32.iter().map(|&x| x as f64).collect())
    }

    /// Compute mean of batch samples
    ///
    /// # Arguments
    /// * `samples` - Flat array of samples (num_samples * dimension)
    ///
    /// # Returns
    /// Mean vector (dimension coefficients)
    pub async fn batch_mean(&self, samples: &[f64]) -> GpuProbabilisticResult<Vec<f64>> {
        let num_samples = self.validate_samples(samples)?;

        // For small batches, use CPU
        if num_samples < 100 {
            return self.batch_mean_cpu(samples, num_samples);
        }

        self.batch_mean_gpu(samples, num_samples).await
    }

    /// CPU fallback for mean computation
    fn batch_mean_cpu(
        &self,
        samples: &[f64],
        num_samples: usize,
    ) -> GpuProbabilisticResult<Vec<f64>> {
        let mut mean = vec![0.0; self.dimension];

        for i in 0..num_samples {
            for j in 0..self.dimension {
                mean[j] += samples[i * self.dimension + j];
            }
        }

        for m in &mut mean {
            *m /= num_samples as f64;
        }

        Ok(mean)
    }

    /// GPU implementation of mean computation
    async fn batch_mean_gpu(
        &self,
        samples: &[f64],
        num_samples: usize,
    ) -> GpuProbabilisticResult<Vec<f64>> {
        let samples_f32: Vec<f32> = samples.iter().map(|&x| x as f32).collect();

        let input_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Samples Buffer"),
                contents: bytemuck::cast_slice(&samples_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let output_size = (self.dimension * std::mem::size_of::<f32>()) as u64;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mean Output Buffer"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.mean_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Mean Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Execute
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Mean Encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Mean Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.mean_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let workgroups = self.dimension.div_ceil(256);
            pass.dispatch_workgroups(workgroups as u32, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);
        self.queue.submit(Some(encoder.finish()));

        // Read back
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .await
            .map_err(|_| GpuProbabilisticError::ComputationFailed("Map cancelled".to_string()))?
            .map_err(|e| GpuProbabilisticError::ComputationFailed(e.to_string()))?;

        let data = buffer_slice.get_mapped_range();
        let result_f32: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging_buffer.unmap();

        // Normalize by num_samples (done on CPU for simplicity)
        Ok(result_f32
            .iter()
            .map(|&x| (x as f64) / (num_samples as f64))
            .collect())
    }

    /// Compute variance of batch samples
    ///
    /// # Arguments
    /// * `samples` - Flat array of samples (num_samples * dimension)
    /// * `mean` - Mean vector (dimension coefficients)
    ///
    /// # Returns
    /// Variance vector (dimension coefficients)
    pub async fn batch_variance(
        &self,
        samples: &[f64],
        mean: &[f64],
    ) -> GpuProbabilisticResult<Vec<f64>> {
        let num_samples = self.validate_samples(samples)?;
        if num_samples < 2 {
            return Err(GpuProbabilisticError::InvalidParameters(
                "at least two samples are required for variance".to_string(),
            ));
        }
        if mean.len() != self.dimension {
            return Err(GpuProbabilisticError::DimensionMismatch {
                expected: self.dimension,
                actual: mean.len(),
            });
        }
        if mean.iter().any(|x| !x.is_finite()) {
            return Err(GpuProbabilisticError::InvalidParameters(
                "mean values must be finite".to_string(),
            ));
        }

        // For small batches, use CPU
        if num_samples < 100 {
            return self.batch_variance_cpu(samples, mean, num_samples);
        }

        self.batch_variance_gpu(samples, mean, num_samples).await
    }

    /// CPU fallback for variance computation
    fn batch_variance_cpu(
        &self,
        samples: &[f64],
        mean: &[f64],
        num_samples: usize,
    ) -> GpuProbabilisticResult<Vec<f64>> {
        let mut variance = vec![0.0; self.dimension];

        for i in 0..num_samples {
            for j in 0..self.dimension {
                let diff = samples[i * self.dimension + j] - mean[j];
                variance[j] += diff * diff;
            }
        }

        for v in &mut variance {
            *v /= (num_samples - 1) as f64; // Bessel's correction
        }

        Ok(variance)
    }

    /// GPU implementation of variance computation
    async fn batch_variance_gpu(
        &self,
        samples: &[f64],
        mean: &[f64],
        num_samples: usize,
    ) -> GpuProbabilisticResult<Vec<f64>> {
        let samples_f32: Vec<f32> = samples.iter().map(|&x| x as f32).collect();
        let mean_f32: Vec<f32> = mean.iter().map(|&x| x as f32).collect();

        let samples_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Samples Buffer"),
                contents: bytemuck::cast_slice(&samples_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let mean_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Mean Buffer"),
                contents: bytemuck::cast_slice(&mean_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let output_size = (self.dimension * std::mem::size_of::<f32>()) as u64;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Variance Output Buffer"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.variance_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Variance Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: samples_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: mean_buffer.as_entire_binding(),
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
                label: Some("Variance Encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Variance Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.variance_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let workgroups = self.dimension.div_ceil(256);
            pass.dispatch_workgroups(workgroups as u32, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);
        self.queue.submit(Some(encoder.finish()));

        // Read back
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .await
            .map_err(|_| GpuProbabilisticError::ComputationFailed("Map cancelled".to_string()))?
            .map_err(|e| GpuProbabilisticError::ComputationFailed(e.to_string()))?;

        let data = buffer_slice.get_mapped_range();
        let result_f32: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging_buffer.unmap();

        // Apply Bessel's correction
        let correction = (num_samples - 1) as f64;
        Ok(result_f32
            .iter()
            .map(|&x| (x as f64) / correction)
            .collect())
    }

    // Pipeline creation helpers

    fn create_sample_pipeline(
        device: &wgpu::Device,
        dimension: usize,
    ) -> GpuProbabilisticResult<wgpu::ComputePipeline> {
        let shader_source = format!(
            r#"
@group(0) @binding(0) var<storage, read> mean: array<f32>;
@group(0) @binding(1) var<storage, read> std_dev: array<f32>;
@group(0) @binding(2) var<storage, read> random: array<f32>;
@group(0) @binding(3) var<storage, read_write> output: array<f32>;

const DIM: u32 = {dimension}u;
const PI: f32 = 3.14159265359;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let idx = id.x;
    if (idx >= arrayLength(&output)) {{
        return;
    }}
    let dim_idx = idx % DIM;

    // Box-Muller transform
    let u1 = random[idx * 2u];
    let u2 = random[idx * 2u + 1u];

    let r = sqrt(-2.0 * log(max(u1, 0.0001)));
    let theta = 2.0 * PI * u2;
    let z = r * cos(theta);

    output[idx] = mean[dim_idx] + std_dev[dim_idx] * z;
}}
"#,
            dimension = dimension
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sample Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(shader_source)),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Sample Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            }),
        )
    }

    fn create_mean_pipeline(
        device: &wgpu::Device,
        dimension: usize,
    ) -> GpuProbabilisticResult<wgpu::ComputePipeline> {
        let shader_source = format!(
            r#"
@group(0) @binding(0) var<storage, read> samples: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;

const DIM: u32 = {dimension}u;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let dim_idx = id.x;
    if dim_idx >= DIM {{
        return;
    }}

    let num_samples = arrayLength(&samples) / DIM;
    var sum: f32 = 0.0;

    for (var i: u32 = 0u; i < num_samples; i = i + 1u) {{
        sum = sum + samples[i * DIM + dim_idx];
    }}

    output[dim_idx] = sum;
}}
"#,
            dimension = dimension
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Mean Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(shader_source)),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Mean Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            }),
        )
    }

    fn create_variance_pipeline(
        device: &wgpu::Device,
        dimension: usize,
    ) -> GpuProbabilisticResult<wgpu::ComputePipeline> {
        let shader_source = format!(
            r#"
@group(0) @binding(0) var<storage, read> samples: array<f32>;
@group(0) @binding(1) var<storage, read> mean: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;

const DIM: u32 = {dimension}u;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let dim_idx = id.x;
    if dim_idx >= DIM {{
        return;
    }}

    let num_samples = arrayLength(&samples) / DIM;
    var sum_sq: f32 = 0.0;
    let m = mean[dim_idx];

    for (var i: u32 = 0u; i < num_samples; i = i + 1u) {{
        let diff = samples[i * DIM + dim_idx] - m;
        sum_sq = sum_sq + diff * diff;
    }}

    output[dim_idx] = sum_sq;
}}
"#,
            dimension = dimension
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Variance Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(shader_source)),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Variance Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore] // Requires GPU
    #[tokio::test]
    async fn test_gpu_probabilistic_creation() {
        let gpu = GpuProbabilistic::new(8).await;
        assert!(gpu.is_ok());
    }

    #[ignore] // Requires GPU
    #[tokio::test]
    async fn test_batch_sample_gaussian() {
        let gpu = GpuProbabilistic::new(8).await.unwrap();
        let mean = vec![0.0; 8];
        let std_dev = vec![1.0; 8];

        let samples = gpu.batch_sample_gaussian(1000, &mean, &std_dev).await;
        assert!(samples.is_ok());
        let samples = samples.unwrap();
        assert_eq!(samples.len(), 1000 * 8);
    }

    #[ignore] // Requires GPU - tests CPU fallback path for small batches
    #[tokio::test]
    async fn test_batch_sample_gaussian_cpu_fallback() {
        let gpu = GpuProbabilistic::new(8).await.unwrap();
        let mean = vec![0.0; 8];
        let std_dev = vec![1.0; 8];

        // Small batch uses CPU
        let samples = gpu.batch_sample_gaussian(50, &mean, &std_dev).await;
        assert!(samples.is_ok());
        let samples = samples.unwrap();
        assert_eq!(samples.len(), 50 * 8);
    }

    #[ignore] // Requires GPU - tests CPU fallback path for small batches
    #[tokio::test]
    async fn test_batch_mean_cpu() {
        let gpu = GpuProbabilistic::new(4).await.unwrap();

        // Create samples with known mean
        let samples = vec![
            1.0, 2.0, 3.0, 4.0, // sample 1
            2.0, 3.0, 4.0, 5.0, // sample 2
            3.0, 4.0, 5.0, 6.0, // sample 3
        ];

        let mean = gpu.batch_mean(&samples).await.unwrap();
        assert_eq!(mean.len(), 4);
        assert!((mean[0] - 2.0).abs() < 0.001);
        assert!((mean[1] - 3.0).abs() < 0.001);
        assert!((mean[2] - 4.0).abs() < 0.001);
        assert!((mean[3] - 5.0).abs() < 0.001);
    }

    #[ignore] // Requires GPU - tests CPU fallback path for small batches
    #[tokio::test]
    async fn test_batch_variance_cpu() {
        let gpu = GpuProbabilistic::new(2).await.unwrap();

        // Samples with known variance
        let samples = vec![
            1.0, 1.0, // sample 1
            2.0, 2.0, // sample 2
            3.0, 3.0, // sample 3
        ];
        let mean = vec![2.0, 2.0];

        let variance = gpu.batch_variance(&samples, &mean).await.unwrap();
        assert_eq!(variance.len(), 2);
        // Variance of [1,2,3] = 1.0 with Bessel's correction
        assert!((variance[0] - 1.0).abs() < 0.001);
        assert!((variance[1] - 1.0).abs() < 0.001);
    }
}
