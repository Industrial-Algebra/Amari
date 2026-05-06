//! GPU-accelerated holographic memory operations
//!
//! This module provides GPU acceleration for Vector Symbolic Architecture (VSA)
//! operations using WebGPU/wgpu compute shaders.
//!
//! The 0.20 public v1 surface is intentionally explicit about its validated
//! scope: `GpuHolographic` operates on the canonical 256-dimensional
//! `ProductCl3x32` representation, uses GPU kernels for batch binding and
//! similarity when batch sizes cross the GPU heuristic, and keeps unbinding and
//! bundling on the CPU correctness path. `GpuOpticalField` provides GPU-backed
//! rotor binding, similarity reduction, and Lee encoding for finite optical
//! fields with matching dimensions.
//!
//! # Supported Operations
//!
//! - **Batch Binding**: GPU-accelerated geometric product for many key-value pairs
//! - **Batch Unbinding**: Parallel retrieval operations
//! - **Batch Similarity**: Efficient similarity computation across large codebooks
//! - **Batch Bundling**: Parallel superposition of multiple representations
//!
//! # Performance
//!
//! GPU acceleration is beneficial for:
//! - Batch sizes > 100 items
//! - Large codebook operations (resonator cleanup)
//! - Memory operations with many stored items
//!
//! For small operations, CPU fallback is used automatically.

use amari_holographic::{AlgebraConfig, BindingAlgebra, HolographicMemory, ProductCliffordAlgebra};
use thiserror::Error;
use wgpu::util::DeviceExt;

/// Type alias for commonly used ProductClifford algebra
pub type ProductCl3x32 = ProductCliffordAlgebra<32>;

const PRODUCT_CL3X32_DIMENSION: usize = ProductCl3x32::DIMENSION;

/// Errors specific to GPU holographic operations
#[derive(Error, Debug)]
pub enum GpuHolographicError {
    #[error("GPU initialization failed: {0}")]
    InitializationError(String),

    #[error("GPU buffer error: {0}")]
    BufferError(String),

    #[error("Shader compilation error: {0}")]
    ShaderError(String),

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Holographic algebra error: {0}")]
    AlgebraError(String),
}

/// Result type for GPU holographic operations
pub type GpuHolographicResult<T> = Result<T, GpuHolographicError>;

/// GPU-accelerated holographic operations
///
/// Provides batch operations for VSA using WebGPU compute shaders.
/// Automatically falls back to CPU for small batch sizes.
pub struct GpuHolographic {
    device: wgpu::Device,
    queue: wgpu::Queue,
    bind_pipeline: wgpu::ComputePipeline,
    similarity_pipeline: wgpu::ComputePipeline,
    bundle_pipeline: wgpu::ComputePipeline,
    dimension: usize,
}

impl GpuHolographic {
    /// Initialize GPU context for holographic operations
    ///
    /// # Arguments
    /// * `dimension` - The dimension of the algebra. The validated v1 GPU path supports
    ///   only 256-dimensional `ProductCl3x32` vectors.
    pub async fn new(dimension: usize) -> GpuHolographicResult<Self> {
        if dimension != PRODUCT_CL3X32_DIMENSION {
            return Err(GpuHolographicError::DimensionMismatch {
                expected: PRODUCT_CL3X32_DIMENSION,
                actual: dimension,
            });
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
                GpuHolographicError::InitializationError("No GPU adapter found".to_string())
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Amari Holographic GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuHolographicError::InitializationError(e.to_string()))?;

        // Create compute pipelines
        let bind_pipeline = Self::create_bind_pipeline(&device, dimension)?;
        let similarity_pipeline = Self::create_similarity_pipeline(&device, dimension)?;
        let bundle_pipeline = Self::create_bundle_pipeline(&device, dimension)?;

        Ok(Self {
            device,
            queue,
            bind_pipeline,
            similarity_pipeline,
            bundle_pipeline,
            dimension,
        })
    }

    /// Create GPU context for ProductCl3x32 (256-dimensional)
    pub async fn new_product_cl3x32() -> GpuHolographicResult<Self> {
        Self::new(256).await
    }

    /// Batch binding operation (geometric product for holographic binding)
    ///
    /// Computes `result[i] = keys[i] ⊛ values[i]` for all i.
    ///
    /// # Arguments
    /// * `keys` - Flat array of key coefficients (batch_size * dimension)
    /// * `values` - Flat array of value coefficients (batch_size * dimension)
    ///
    /// # Returns
    /// Flat array of bound pair coefficients
    pub async fn batch_bind(&self, keys: &[f64], values: &[f64]) -> GpuHolographicResult<Vec<f64>> {
        let batch_size = self.validate_flat_pair_inputs(keys, values)?;

        // For small batches, use CPU
        if batch_size < 100 {
            return self.batch_bind_cpu(keys, values);
        }

        self.batch_bind_gpu(keys, values).await
    }

    /// Batch unbinding operation.
    ///
    /// Computes `result[i] = keys[i]⁻¹ ⊛ bounds[i]` for all i. This path remains
    /// CPU-backed for correctness because the GPU inverse kernel is not part of
    /// the validated v1 surface.
    pub async fn batch_unbind(
        &self,
        keys: &[f64],
        bounds: &[f64],
    ) -> GpuHolographicResult<Vec<f64>> {
        let batch_size = self.validate_flat_pair_inputs(keys, bounds)?;

        // For small batches, use CPU
        if batch_size < 100 {
            return self.batch_unbind_cpu(keys, bounds);
        }

        // GPU unbinding requires computing inverse first
        // For now, use CPU for correctness
        self.batch_unbind_cpu(keys, bounds)
    }

    /// Batch similarity computation
    ///
    /// Computes cosine similarity between all pairs.
    ///
    /// # Arguments
    /// * `a_batch` - First set of vectors (batch_size * dimension)
    /// * `b_batch` - Second set of vectors (batch_size * dimension)
    ///
    /// # Returns
    /// Array of similarity values (batch_size)
    pub async fn batch_similarity(
        &self,
        a_batch: &[f64],
        b_batch: &[f64],
    ) -> GpuHolographicResult<Vec<f64>> {
        let batch_size = self.validate_flat_pair_inputs(a_batch, b_batch)?;

        // For small batches, use CPU
        if batch_size < 100 {
            return self.batch_similarity_cpu(a_batch, b_batch);
        }

        self.batch_similarity_gpu(a_batch, b_batch).await
    }

    /// Batch bundling operation.
    ///
    /// Computes `ProductCl3x32::bundle(..., beta = 1.0)` for each pair. This is
    /// intentionally CPU-backed in v1 so it matches `amari-holographic` soft
    /// bundling semantics instead of the older experimental element-wise shader.
    pub async fn batch_bundle(
        &self,
        a_batch: &[f64],
        b_batch: &[f64],
    ) -> GpuHolographicResult<Vec<f64>> {
        let _batch_size = self.validate_flat_pair_inputs(a_batch, b_batch)?;

        self.batch_bundle_cpu(a_batch, b_batch)
    }

    /// Find most similar item in codebook (for resonator cleanup)
    ///
    /// # Arguments
    /// * `query` - Query vector coefficients (dimension)
    /// * `codebook` - Codebook vectors (codebook_size * dimension)
    ///
    /// # Returns
    /// (best_index, best_similarity)
    pub async fn find_most_similar(
        &self,
        query: &[f64],
        codebook: &[f64],
    ) -> GpuHolographicResult<(usize, f64)> {
        if query.len() != self.dimension {
            return Err(GpuHolographicError::DimensionMismatch {
                expected: self.dimension,
                actual: query.len(),
            });
        }

        self.validate_flat_input(query)?;
        self.validate_flat_input(codebook)?;
        let codebook_size = codebook.len() / self.dimension;
        if codebook_size == 0 {
            return Err(GpuHolographicError::DimensionMismatch {
                expected: self.dimension,
                actual: 0,
            });
        }

        // Compute similarities with all codebook items
        let queries = query.repeat(codebook_size);
        let similarities = self.batch_similarity(&queries, codebook).await?;

        // Find maximum
        let (best_idx, best_sim) = similarities
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, s)| (i, *s))
            .unwrap_or((0, 0.0));

        Ok((best_idx, best_sim))
    }

    /// Heuristic to determine if GPU should be used
    pub fn should_use_gpu(batch_size: usize) -> bool {
        batch_size >= 100
    }

    fn validate_flat_pair_inputs(&self, a: &[f64], b: &[f64]) -> GpuHolographicResult<usize> {
        if a.len() != b.len() {
            return Err(GpuHolographicError::DimensionMismatch {
                expected: a.len(),
                actual: b.len(),
            });
        }
        self.validate_flat_input(a)
    }

    fn validate_flat_input(&self, input: &[f64]) -> GpuHolographicResult<usize> {
        if !input.len().is_multiple_of(self.dimension) {
            return Err(GpuHolographicError::DimensionMismatch {
                expected: input.len().next_multiple_of(self.dimension),
                actual: input.len(),
            });
        }
        if let Some((index, _)) = input
            .iter()
            .enumerate()
            .find(|(_, value)| !value.is_finite())
        {
            return Err(GpuHolographicError::BufferError(format!(
                "holographic coefficient {index} is not finite"
            )));
        }
        Ok(input.len() / self.dimension)
    }

    // ========================================================================
    // CPU fallback implementations
    // ========================================================================

    fn batch_bind_cpu(&self, keys: &[f64], values: &[f64]) -> GpuHolographicResult<Vec<f64>> {
        let batch_size = keys.len() / self.dimension;
        let mut results = Vec::with_capacity(keys.len());

        for i in 0..batch_size {
            let start = i * self.dimension;
            let end = start + self.dimension;

            let key = ProductCl3x32::from_coefficients(&keys[start..end])
                .map_err(|e| GpuHolographicError::AlgebraError(format!("{}", e)))?;
            let value = ProductCl3x32::from_coefficients(&values[start..end])
                .map_err(|e| GpuHolographicError::AlgebraError(format!("{}", e)))?;

            let bound = key.bind(&value);
            results.extend(bound.to_coefficients());
        }

        Ok(results)
    }

    fn batch_unbind_cpu(&self, keys: &[f64], bounds: &[f64]) -> GpuHolographicResult<Vec<f64>> {
        let batch_size = keys.len() / self.dimension;
        let mut results = Vec::with_capacity(keys.len());

        for i in 0..batch_size {
            let start = i * self.dimension;
            let end = start + self.dimension;

            let key = ProductCl3x32::from_coefficients(&keys[start..end])
                .map_err(|e| GpuHolographicError::AlgebraError(format!("{}", e)))?;
            let bound = ProductCl3x32::from_coefficients(&bounds[start..end])
                .map_err(|e| GpuHolographicError::AlgebraError(format!("{}", e)))?;

            let unbound = key
                .unbind(&bound)
                .map_err(|e| GpuHolographicError::AlgebraError(format!("{}", e)))?;
            results.extend(unbound.to_coefficients());
        }

        Ok(results)
    }

    fn batch_similarity_cpu(&self, a: &[f64], b: &[f64]) -> GpuHolographicResult<Vec<f64>> {
        let batch_size = a.len() / self.dimension;
        let mut results = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let start = i * self.dimension;
            let end = start + self.dimension;

            let a_vec = ProductCl3x32::from_coefficients(&a[start..end])
                .map_err(|e| GpuHolographicError::AlgebraError(format!("{}", e)))?;
            let b_vec = ProductCl3x32::from_coefficients(&b[start..end])
                .map_err(|e| GpuHolographicError::AlgebraError(format!("{}", e)))?;

            results.push(a_vec.similarity(&b_vec));
        }

        Ok(results)
    }

    fn batch_bundle_cpu(&self, a: &[f64], b: &[f64]) -> GpuHolographicResult<Vec<f64>> {
        let batch_size = a.len() / self.dimension;
        let mut results = Vec::with_capacity(a.len());

        for i in 0..batch_size {
            let start = i * self.dimension;
            let end = start + self.dimension;

            let a_vec = ProductCl3x32::from_coefficients(&a[start..end])
                .map_err(|e| GpuHolographicError::AlgebraError(format!("{}", e)))?;
            let b_vec = ProductCl3x32::from_coefficients(&b[start..end])
                .map_err(|e| GpuHolographicError::AlgebraError(format!("{}", e)))?;

            let bundled = a_vec
                .bundle(&b_vec, 1.0)
                .map_err(|e| GpuHolographicError::AlgebraError(format!("{}", e)))?;
            results.extend(bundled.to_coefficients());
        }

        Ok(results)
    }

    // ========================================================================
    // GPU implementations
    // ========================================================================

    async fn batch_bind_gpu(&self, keys: &[f64], values: &[f64]) -> GpuHolographicResult<Vec<f64>> {
        let batch_size = keys.len() / self.dimension;

        // Convert to f32 for GPU
        let keys_f32: Vec<f32> = keys.iter().map(|&x| x as f32).collect();
        let values_f32: Vec<f32> = values.iter().map(|&x| x as f32).collect();

        // Create GPU buffers
        let keys_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Keys Buffer"),
                contents: bytemuck::cast_slice(&keys_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let values_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Values Buffer"),
                contents: bytemuck::cast_slice(&values_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let output_size = (keys.len() * std::mem::size_of::<f32>()) as u64;
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
        let bind_group_layout = self.bind_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind Compute Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: keys_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: values_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Dispatch compute shader
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Bind Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Bind Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.bind_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(batch_size as u32, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);

        self.queue.submit(Some(encoder.finish()));

        // Read back results
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .await
            .map_err(|_| GpuHolographicError::BufferError("Channel error".to_string()))?
            .map_err(|e| GpuHolographicError::BufferError(e.to_string()))?;

        let data = buffer_slice.get_mapped_range();
        let result_f32: &[f32] = bytemuck::cast_slice(&data);
        let result: Vec<f64> = result_f32.iter().map(|&x| x as f64).collect();

        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }

    async fn batch_similarity_gpu(&self, a: &[f64], b: &[f64]) -> GpuHolographicResult<Vec<f64>> {
        let batch_size = a.len() / self.dimension;

        // Convert to f32 for GPU
        let a_f32: Vec<f32> = a.iter().map(|&x| x as f32).collect();
        let b_f32: Vec<f32> = b.iter().map(|&x| x as f32).collect();

        // Create GPU buffers
        let a_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("A Buffer"),
                contents: bytemuck::cast_slice(&a_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let b_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("B Buffer"),
                contents: bytemuck::cast_slice(&b_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let output_size = (batch_size * std::mem::size_of::<f32>()) as u64;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Similarity Output Buffer"),
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
        let bind_group_layout = self.similarity_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Similarity Compute Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: a_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: b_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Dispatch compute shader
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Similarity Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Similarity Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.similarity_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(batch_size.div_ceil(64) as u32, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);

        self.queue.submit(Some(encoder.finish()));

        // Read back results
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .await
            .map_err(|_| GpuHolographicError::BufferError("Channel error".to_string()))?
            .map_err(|e| GpuHolographicError::BufferError(e.to_string()))?;

        let data = buffer_slice.get_mapped_range();
        let result_f32: &[f32] = bytemuck::cast_slice(&data);
        let result: Vec<f64> = result_f32.iter().map(|&x| x as f64).collect();

        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }

    #[allow(dead_code)] // retained as redesign-pending shader path; public v1 uses CPU bundling
    async fn batch_bundle_gpu(&self, a: &[f64], b: &[f64]) -> GpuHolographicResult<Vec<f64>> {
        let batch_size = a.len() / self.dimension;

        // Convert to f32 for GPU
        let a_f32: Vec<f32> = a.iter().map(|&x| x as f32).collect();
        let b_f32: Vec<f32> = b.iter().map(|&x| x as f32).collect();

        // Create GPU buffers
        let a_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("A Buffer"),
                contents: bytemuck::cast_slice(&a_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let b_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("B Buffer"),
                contents: bytemuck::cast_slice(&b_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let output_size = (a.len() * std::mem::size_of::<f32>()) as u64;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bundle Output Buffer"),
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
        let bind_group_layout = self.bundle_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bundle Compute Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: a_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: b_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Dispatch compute shader
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Bundle Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Bundle Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.bundle_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(batch_size as u32, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);

        self.queue.submit(Some(encoder.finish()));

        // Read back results
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .await
            .map_err(|_| GpuHolographicError::BufferError("Channel error".to_string()))?
            .map_err(|e| GpuHolographicError::BufferError(e.to_string()))?;

        let data = buffer_slice.get_mapped_range();
        let result_f32: &[f32] = bytemuck::cast_slice(&data);
        let result: Vec<f64> = result_f32.iter().map(|&x| x as f64).collect();

        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }

    // ========================================================================
    // Pipeline creation
    // ========================================================================

    fn create_bind_pipeline(
        device: &wgpu::Device,
        dimension: usize,
    ) -> GpuHolographicResult<wgpu::ComputePipeline> {
        let shader_source = Self::get_bind_shader(dimension);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Holographic Bind Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(shader_source)),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Holographic Bind Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            }),
        )
    }

    fn create_similarity_pipeline(
        device: &wgpu::Device,
        dimension: usize,
    ) -> GpuHolographicResult<wgpu::ComputePipeline> {
        let shader_source = Self::get_similarity_shader(dimension);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Holographic Similarity Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(shader_source)),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Holographic Similarity Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            }),
        )
    }

    fn create_bundle_pipeline(
        device: &wgpu::Device,
        dimension: usize,
    ) -> GpuHolographicResult<wgpu::ComputePipeline> {
        let shader_source = Self::get_bundle_shader(dimension);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Holographic Bundle Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(shader_source)),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Holographic Bundle Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            }),
        )
    }

    // ========================================================================
    // WGSL Shader generation
    // ========================================================================

    fn get_bind_shader(dimension: usize) -> String {
        format!(
            r#"
// Holographic binding shader (element-wise product for ProductClifford)
// For ProductClifford, binding is component-wise Cl3 geometric product

const DIMENSION: u32 = {dim}u;
const CL3_DIM: u32 = 8u;
const NUM_COMPONENTS: u32 = {dim}u / 8u;

@group(0) @binding(0)
var<storage, read> keys: array<f32>;

@group(0) @binding(1)
var<storage, read> values: array<f32>;

@group(0) @binding(2)
var<storage, read_write> output: array<f32>;

fn cl3_geometric_product(a0: f32, a1: f32, a2: f32, a3: f32, a4: f32, a5: f32, a6: f32, a7: f32,
                         b0: f32, b1: f32, b2: f32, b3: f32, b4: f32, b5: f32, b6: f32, b7: f32,
                         component_offset: u32) {{
    // Matches amari-holographic Cl3 coefficient order:
    // [s, e1, e2, e3, e12, e13, e23, e123].
    output[component_offset + 0u] = a0*b0 + a1*b1 + a2*b2 + a3*b3 - a4*b4 - a5*b5 - a6*b6 - a7*b7;
    output[component_offset + 1u] = a0*b1 + a1*b0 - a2*b4 - a3*b5 + a4*b2 + a5*b3 - a6*b7 - a7*b6;
    output[component_offset + 2u] = a0*b2 + a2*b0 + a1*b4 - a3*b6 - a4*b1 - a5*b7 + a6*b3 + a7*b5;
    output[component_offset + 3u] = a0*b3 + a3*b0 + a1*b5 + a2*b6 + a4*b7 - a5*b1 - a6*b2 - a7*b4;
    output[component_offset + 4u] = a0*b4 + a4*b0 + a1*b2 - a2*b1 - a3*b7 + a5*b6 - a6*b5 - a7*b3;
    output[component_offset + 5u] = a0*b5 + a5*b0 + a1*b3 - a3*b1 + a2*b7 - a4*b6 + a6*b4 + a7*b2;
    output[component_offset + 6u] = a0*b6 + a6*b0 + a2*b3 - a3*b2 - a1*b7 + a4*b5 - a5*b4 - a7*b1;
    output[component_offset + 7u] = a0*b7 + a7*b0 + a1*b6 + a2*b5 + a3*b4 + a4*b3 + a5*b2 + a6*b1;
}}

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
    let batch_idx = global_id.x;
    let offset = batch_idx * DIMENSION;

    // For each Cl3 component
    for (var comp = 0u; comp < NUM_COMPONENTS; comp = comp + 1u) {{
        let comp_offset = offset + comp * CL3_DIM;

        cl3_geometric_product(
            keys[comp_offset + 0u], keys[comp_offset + 1u], keys[comp_offset + 2u], keys[comp_offset + 3u],
            keys[comp_offset + 4u], keys[comp_offset + 5u], keys[comp_offset + 6u], keys[comp_offset + 7u],
            values[comp_offset + 0u], values[comp_offset + 1u], values[comp_offset + 2u], values[comp_offset + 3u],
            values[comp_offset + 4u], values[comp_offset + 5u], values[comp_offset + 6u], values[comp_offset + 7u],
            comp_offset
        );
    }}
}}
"#,
            dim = dimension
        )
    }

    fn get_similarity_shader(dimension: usize) -> String {
        format!(
            r#"
// Holographic similarity shader (cosine similarity)

const DIMENSION: u32 = {dim}u;

@group(0) @binding(0)
var<storage, read> a_batch: array<f32>;

@group(0) @binding(1)
var<storage, read> b_batch: array<f32>;

@group(0) @binding(2)
var<storage, read_write> output: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
    let batch_idx = global_id.x;
    if (batch_idx >= arrayLength(&output)) {{
        return;
    }}
    let offset = batch_idx * DIMENSION;

    var dot_product: f32 = 0.0;
    var norm_a_sq: f32 = 0.0;
    var norm_b_sq: f32 = 0.0;

    for (var i = 0u; i < DIMENSION; i = i + 1u) {{
        let a = a_batch[offset + i];
        let b = b_batch[offset + i];

        dot_product += a * b;
        norm_a_sq += a * a;
        norm_b_sq += b * b;
    }}

    let norm_a = sqrt(norm_a_sq);
    let norm_b = sqrt(norm_b_sq);

    var similarity: f32 = 0.0;
    if (norm_a > 1e-10 && norm_b > 1e-10) {{
        similarity = dot_product / (norm_a * norm_b);
        similarity = clamp(similarity, -1.0, 1.0);
    }}

    output[batch_idx] = similarity;
}}
"#,
            dim = dimension
        )
    }

    fn get_bundle_shader(dimension: usize) -> String {
        format!(
            r#"
// Holographic bundling shader (element-wise sum)

const DIMENSION: u32 = {dim}u;

@group(0) @binding(0)
var<storage, read> a_batch: array<f32>;

@group(0) @binding(1)
var<storage, read> b_batch: array<f32>;

@group(0) @binding(2)
var<storage, read_write> output: array<f32>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
    let batch_idx = global_id.x;
    let offset = batch_idx * DIMENSION;

    for (var i = 0u; i < DIMENSION; i = i + 1u) {{
        output[offset + i] = a_batch[offset + i] + b_batch[offset + i];
    }}
}}
"#,
            dim = dimension
        )
    }
}

/// GPU-accelerated holographic memory
///
/// Wrapper around `HolographicMemory` that uses GPU for batch operations.
pub struct GpuHolographicMemory {
    memory: HolographicMemory<ProductCl3x32>,
    gpu: Option<GpuHolographic>,
}

impl GpuHolographicMemory {
    /// Create a new GPU-accelerated holographic memory
    pub async fn new() -> GpuHolographicResult<Self> {
        let memory = HolographicMemory::new(AlgebraConfig::default());
        let gpu = GpuHolographic::new_product_cl3x32().await.ok();

        Ok(Self { memory, gpu })
    }

    /// Store a key-value pair
    pub fn store(&mut self, key: &ProductCl3x32, value: &ProductCl3x32) {
        self.memory.store(key, value);
    }

    /// Store multiple key-value pairs.
    ///
    /// This wrapper currently delegates to `amari-holographic`'s validated CPU
    /// memory implementation. It validates equal batch lengths instead of
    /// silently dropping unpaired keys or values.
    pub async fn store_batch(
        &mut self,
        keys: &[ProductCl3x32],
        values: &[ProductCl3x32],
    ) -> GpuHolographicResult<()> {
        if keys.len() != values.len() {
            return Err(GpuHolographicError::DimensionMismatch {
                expected: keys.len(),
                actual: values.len(),
            });
        }
        let pairs: Vec<_> = keys.iter().cloned().zip(values.iter().cloned()).collect();
        self.memory.store_batch(&pairs);
        Ok(())
    }

    /// Retrieve a value by key
    pub fn retrieve(
        &self,
        key: &ProductCl3x32,
    ) -> amari_holographic::RetrievalResult<ProductCl3x32> {
        self.memory.retrieve(key)
    }

    /// Get memory statistics
    pub fn capacity_info(&self) -> amari_holographic::CapacityInfo {
        self.memory.capacity_info()
    }

    /// Clear the memory
    pub fn clear(&mut self) {
        self.memory.clear();
    }

    /// Check if GPU is available
    pub fn has_gpu(&self) -> bool {
        self.gpu.is_some()
    }
}

// ============================================================================
// GPU-accelerated Optical Field Operations
// ============================================================================

use amari_holographic::optical::{BinaryHologram, LeeEncoderConfig, OpticalRotorField};

/// GPU-accelerated optical rotor field operations.
///
/// Provides batch operations for optical fields using WebGPU compute shaders.
/// The optical field uses a SoA (Structure of Arrays) layout with separate
/// f32 arrays for scalar, bivector, and amplitude components.
///
/// # Supported Operations
///
/// - **Batch Bind**: Parallel rotor multiplication (phase addition)
/// - **Batch Similarity**: Parallel inner product computation
/// - **Batch Bundle**: Parallel weighted sum with normalization
/// - **Batch Lee Encoding**: Parallel binary hologram generation
///
/// # Performance
///
/// GPU acceleration is beneficial for:
/// - Field dimensions > 64×64 (4096 pixels)
/// - Batch operations with many fields
/// - Lee encoding of large holograms
///
/// # Example
///
/// ```ignore
/// use amari_gpu::GpuOpticalField;
/// use amari_holographic::optical::OpticalRotorField;
///
/// let gpu = GpuOpticalField::new((256, 256)).await?;
///
/// let field_a = OpticalRotorField::random((256, 256), 1);
/// let field_b = OpticalRotorField::random((256, 256), 2);
///
/// // GPU-accelerated binding
/// let bound = gpu.bind(&field_a, &field_b).await?;
///
/// // GPU-accelerated similarity
/// let sim = gpu.similarity(&field_a, &field_b).await?;
/// ```
pub struct GpuOpticalField {
    device: wgpu::Device,
    queue: wgpu::Queue,
    bind_pipeline: wgpu::ComputePipeline,
    similarity_pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)] // Bundle shader is placeholder - full implementation pending
    bundle_pipeline: wgpu::ComputePipeline,
    lee_encode_pipeline: wgpu::ComputePipeline,
    dimensions: (usize, usize),
}

impl GpuOpticalField {
    /// Initialize GPU context for optical field operations.
    ///
    /// # Arguments
    ///
    /// * `dimensions` - Field dimensions (width, height)
    pub async fn new(dimensions: (usize, usize)) -> GpuHolographicResult<Self> {
        if dimensions.0 == 0 || dimensions.1 == 0 {
            return Err(GpuHolographicError::DimensionMismatch {
                expected: 1,
                actual: 0,
            });
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
                GpuHolographicError::InitializationError("No GPU adapter found".to_string())
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Amari Optical GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuHolographicError::InitializationError(e.to_string()))?;

        // Create compute pipelines
        let bind_pipeline = Self::create_bind_pipeline(&device)?;
        let similarity_pipeline = Self::create_similarity_pipeline(&device)?;
        let bundle_pipeline = Self::create_bundle_pipeline(&device)?;
        let lee_encode_pipeline = Self::create_lee_encode_pipeline(&device)?;

        Ok(Self {
            device,
            queue,
            bind_pipeline,
            similarity_pipeline,
            bundle_pipeline,
            lee_encode_pipeline,
            dimensions,
        })
    }

    /// Get the dimensions this GPU context operates on.
    pub fn dimensions(&self) -> (usize, usize) {
        self.dimensions
    }

    /// Total number of pixels.
    pub fn field_size(&self) -> usize {
        self.dimensions.0 * self.dimensions.1
    }

    /// Heuristic to determine if GPU should be used.
    ///
    /// GPU is beneficial for fields larger than 64×64 (4096 pixels).
    pub fn should_use_gpu(field_size: usize) -> bool {
        field_size >= 4096
    }

    // ========================================================================
    // GPU Bind Operation
    // ========================================================================

    /// GPU-accelerated rotor multiplication (phase addition).
    ///
    /// Computes the geometric product of two rotor fields:
    /// ```text
    /// R_out = R_a · R_b
    /// scalar_out = scalar_a·scalar_b - bivector_a·bivector_b
    /// bivector_out = scalar_a·bivector_b + bivector_a·scalar_b
    /// amplitude_out = amplitude_a · amplitude_b
    /// ```
    pub async fn bind(
        &self,
        a: &OpticalRotorField,
        b: &OpticalRotorField,
    ) -> GpuHolographicResult<OpticalRotorField> {
        self.validate_field_pair(a, b)?;

        // For small fields, use CPU
        if !Self::should_use_gpu(self.field_size()) {
            return self.bind_cpu(a, b);
        }

        self.bind_gpu(a, b).await
    }

    fn bind_cpu(
        &self,
        a: &OpticalRotorField,
        b: &OpticalRotorField,
    ) -> GpuHolographicResult<OpticalRotorField> {
        let n = a.len();
        let mut scalar = Vec::with_capacity(n);
        let mut bivector = Vec::with_capacity(n);
        let mut amplitude = Vec::with_capacity(n);

        let a_s = a.scalars();
        let a_b = a.bivectors();
        let b_s = b.scalars();
        let b_b = b.bivectors();
        let a_amp = a.amplitudes();
        let b_amp = b.amplitudes();

        for i in 0..n {
            scalar.push(a_s[i] * b_s[i] - a_b[i] * b_b[i]);
            bivector.push(a_s[i] * b_b[i] + a_b[i] * b_s[i]);
            amplitude.push(a_amp[i] * b_amp[i]);
        }

        Ok(OpticalRotorField::new(
            // Convert from scalar/bivector to phase
            scalar
                .iter()
                .zip(bivector.iter())
                .map(|(&s, &b)| b.atan2(s))
                .collect(),
            amplitude,
            self.dimensions,
        ))
    }

    async fn bind_gpu(
        &self,
        a: &OpticalRotorField,
        b: &OpticalRotorField,
    ) -> GpuHolographicResult<OpticalRotorField> {
        let n = a.len();

        // Create input buffers (interleaved: scalar, bivector, amplitude for each pixel)
        let a_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Optical A Scalar Buffer"),
                contents: bytemuck::cast_slice(a.scalars()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let a_biv_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Optical A Bivector Buffer"),
                contents: bytemuck::cast_slice(a.bivectors()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let a_amp_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Optical A Amplitude Buffer"),
                contents: bytemuck::cast_slice(a.amplitudes()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let b_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Optical B Scalar Buffer"),
                contents: bytemuck::cast_slice(b.scalars()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let b_biv_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Optical B Bivector Buffer"),
                contents: bytemuck::cast_slice(b.bivectors()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let b_amp_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Optical B Amplitude Buffer"),
                contents: bytemuck::cast_slice(b.amplitudes()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let component_size = (n * std::mem::size_of::<f32>()) as u64;
        let output_size = component_size * 3;

        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Optical Bind Output Buffer"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Optical Bind Staging"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.bind_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Optical Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: a_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: a_biv_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: a_amp_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: b_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: b_biv_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: b_amp_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Dispatch
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Optical Bind Encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Optical Bind Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.bind_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let workgroups = n.div_ceil(256) as u32;
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging, 0, output_size);

        self.queue.submit(Some(encoder.finish()));

        // Read back results as [scalar..., bivector..., amplitude...]
        let combined = self.read_buffer(&staging, n * 3).await?;
        let scalar_result = &combined[0..n];
        let bivector_result = &combined[n..2 * n];
        let amplitude_result = combined[2 * n..3 * n].to_vec();

        // Convert to phase
        let phase: Vec<f32> = scalar_result
            .iter()
            .zip(bivector_result.iter())
            .map(|(&s, &b)| b.atan2(s))
            .collect();

        Ok(OpticalRotorField::new(
            phase,
            amplitude_result,
            self.dimensions,
        ))
    }

    // ========================================================================
    // GPU Similarity Operation
    // ========================================================================

    /// GPU-accelerated similarity computation.
    ///
    /// Computes the normalized inner product of two rotor fields.
    pub async fn similarity(
        &self,
        a: &OpticalRotorField,
        b: &OpticalRotorField,
    ) -> GpuHolographicResult<f32> {
        self.validate_field_pair(a, b)?;

        // For small fields, use CPU
        if !Self::should_use_gpu(self.field_size()) {
            return self.similarity_cpu(a, b);
        }

        self.similarity_gpu(a, b).await
    }

    fn similarity_cpu(
        &self,
        a: &OpticalRotorField,
        b: &OpticalRotorField,
    ) -> GpuHolographicResult<f32> {
        let n = a.len();
        let mut sum = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        let a_s = a.scalars();
        let a_biv = a.bivectors();
        let b_s = b.scalars();
        let b_biv = b.bivectors();
        let a_amp = a.amplitudes();
        let b_amp = b.amplitudes();

        for i in 0..n {
            // R_a† · R_b scalar part = a_s·b_s + a_b·b_b
            let inner = a_s[i] * b_s[i] + a_biv[i] * b_biv[i];
            sum += a_amp[i] * b_amp[i] * inner;
            norm_a += a_amp[i] * a_amp[i];
            norm_b += b_amp[i] * b_amp[i];
        }

        let denom = (norm_a * norm_b).sqrt();
        if denom > 1e-10 {
            Ok(sum / denom)
        } else {
            Ok(0.0)
        }
    }

    async fn similarity_gpu(
        &self,
        a: &OpticalRotorField,
        b: &OpticalRotorField,
    ) -> GpuHolographicResult<f32> {
        let n = a.len();

        // Create input buffers
        let a_s_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("A Scalar"),
                contents: bytemuck::cast_slice(a.scalars()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let a_b_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("A Bivector"),
                contents: bytemuck::cast_slice(a.bivectors()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let a_amp_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("A Amplitude"),
                contents: bytemuck::cast_slice(a.amplitudes()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let b_s_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("B Scalar"),
                contents: bytemuck::cast_slice(b.scalars()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let b_b_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("B Bivector"),
                contents: bytemuck::cast_slice(b.bivectors()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let b_amp_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("B Amplitude"),
                contents: bytemuck::cast_slice(b.amplitudes()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        // Output buffer for partial sums (one per workgroup)
        let num_workgroups = n.div_ceil(256);
        let partial_size = (num_workgroups * 3 * std::mem::size_of::<f32>()) as u64;

        let partial_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Partial Sums"),
            size: partial_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging"),
            size: partial_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.similarity_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Similarity Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: a_s_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: a_b_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: a_amp_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: b_s_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: b_b_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: b_amp_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: partial_buffer.as_entire_binding(),
                },
            ],
        });

        // Dispatch
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Similarity Encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Similarity Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.similarity_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(num_workgroups as u32, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&partial_buffer, 0, &staging, 0, partial_size);
        self.queue.submit(Some(encoder.finish()));

        // Read partial results and reduce on CPU
        let partials = self.read_buffer(&staging, num_workgroups * 3).await?;

        let mut sum = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..num_workgroups {
            sum += partials[i * 3];
            norm_a += partials[i * 3 + 1];
            norm_b += partials[i * 3 + 2];
        }

        let denom = (norm_a * norm_b).sqrt();
        if denom > 1e-10 {
            Ok(sum / denom)
        } else {
            Ok(0.0)
        }
    }

    // ========================================================================
    // GPU Lee Hologram Encoding
    // ========================================================================

    /// GPU-accelerated Lee hologram encoding.
    ///
    /// Encodes an optical rotor field as a binary hologram pattern.
    ///
    /// # Arguments
    ///
    /// * `field` - Input optical rotor field
    /// * `config` - Lee encoder configuration
    pub async fn encode_lee(
        &self,
        field: &OpticalRotorField,
        config: &LeeEncoderConfig,
    ) -> GpuHolographicResult<BinaryHologram> {
        if config.dimensions != self.dimensions {
            return Err(GpuHolographicError::DimensionMismatch {
                expected: self.field_size(),
                actual: config.dimensions.0 * config.dimensions.1,
            });
        }
        self.validate_field(field)?;

        // For small fields, use CPU
        if !Self::should_use_gpu(field.len()) {
            return self.encode_lee_cpu(field, config);
        }

        self.encode_lee_gpu(field, config).await
    }

    fn encode_lee_cpu(
        &self,
        field: &OpticalRotorField,
        config: &LeeEncoderConfig,
    ) -> GpuHolographicResult<BinaryHologram> {
        use amari_holographic::optical::GeometricLeeEncoder;
        let encoder = GeometricLeeEncoder::new(config.clone());
        Ok(encoder.encode(field))
    }

    async fn encode_lee_gpu(
        &self,
        field: &OpticalRotorField,
        config: &LeeEncoderConfig,
    ) -> GpuHolographicResult<BinaryHologram> {
        let n = field.len();
        let (w, h) = config.dimensions;

        // Compute carrier on CPU (it's a simple linear function)
        let cos_angle = config.carrier_angle.cos();
        let sin_angle = config.carrier_angle.sin();
        let mut carrier_s = Vec::with_capacity(n);
        let mut carrier_b = Vec::with_capacity(n);

        for y in 0..h {
            for x in 0..w {
                let t = x as f32 * cos_angle + y as f32 * sin_angle;
                let p = std::f32::consts::TAU * config.carrier_frequency * t;
                carrier_s.push(p.cos());
                carrier_b.push(p.sin());
            }
        }

        // Create buffers
        let field_s_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Field Scalar"),
                contents: bytemuck::cast_slice(field.scalars()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let field_b_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Field Bivector"),
                contents: bytemuck::cast_slice(field.bivectors()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let field_amp_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Field Amplitude"),
                contents: bytemuck::cast_slice(field.amplitudes()),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let carrier_s_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Carrier Scalar"),
                contents: bytemuck::cast_slice(&carrier_s),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let carrier_b_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Carrier Bivector"),
                contents: bytemuck::cast_slice(&carrier_b),
                usage: wgpu::BufferUsages::STORAGE,
            });

        // Output: u32 packed bits
        let output_words = n.div_ceil(32);
        let output_size = (output_words * std::mem::size_of::<u32>()) as u64;

        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lee Output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.lee_encode_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lee Encode Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: field_s_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: field_b_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: field_amp_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: carrier_s_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: carrier_b_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Dispatch
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Lee Encode Encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Lee Encode Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.lee_encode_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(output_words.div_ceil(64) as u32, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging, 0, output_size);
        self.queue.submit(Some(encoder.finish()));

        // Read result and convert to BinaryHologram
        let packed = self.read_buffer_u32(&staging, output_words).await?;

        // Convert to bools
        let mut pattern = Vec::with_capacity(n);
        for i in 0..n {
            let word = i / 32;
            let bit = i % 32;
            pattern.push((packed[word] >> bit) & 1 != 0);
        }

        Ok(BinaryHologram::from_bools(&pattern, config.dimensions))
    }

    // ========================================================================
    // GPU Batch Bind Operation
    // ========================================================================

    /// GPU-accelerated batch binding of multiple field pairs.
    ///
    /// Computes `results[i] = bind(a_batch[i], b_batch[i])` for all i.
    pub async fn batch_bind(
        &self,
        a_batch: &[OpticalRotorField],
        b_batch: &[OpticalRotorField],
    ) -> GpuHolographicResult<Vec<OpticalRotorField>> {
        if a_batch.len() != b_batch.len() {
            return Err(GpuHolographicError::DimensionMismatch {
                expected: a_batch.len(),
                actual: b_batch.len(),
            });
        }

        for (a, b) in a_batch.iter().zip(b_batch.iter()) {
            self.validate_field_pair(a, b)?;
        }

        let mut results = Vec::with_capacity(a_batch.len());
        for (a, b) in a_batch.iter().zip(b_batch.iter()) {
            results.push(self.bind(a, b).await?);
        }
        Ok(results)
    }

    /// GPU-accelerated batch similarity of multiple field pairs.
    ///
    /// Computes `results[i] = similarity(a_batch[i], b_batch[i])` for all i.
    pub async fn batch_similarity(
        &self,
        a_batch: &[OpticalRotorField],
        b_batch: &[OpticalRotorField],
    ) -> GpuHolographicResult<Vec<f32>> {
        if a_batch.len() != b_batch.len() {
            return Err(GpuHolographicError::DimensionMismatch {
                expected: a_batch.len(),
                actual: b_batch.len(),
            });
        }

        for (a, b) in a_batch.iter().zip(b_batch.iter()) {
            self.validate_field_pair(a, b)?;
        }

        let mut results = Vec::with_capacity(a_batch.len());
        for (a, b) in a_batch.iter().zip(b_batch.iter()) {
            results.push(self.similarity(a, b).await?);
        }
        Ok(results)
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    fn validate_field_pair(
        &self,
        a: &OpticalRotorField,
        b: &OpticalRotorField,
    ) -> GpuHolographicResult<()> {
        self.validate_field(a)?;
        self.validate_field(b)
    }

    fn validate_field(&self, field: &OpticalRotorField) -> GpuHolographicResult<()> {
        if field.dimensions() != self.dimensions {
            return Err(GpuHolographicError::DimensionMismatch {
                expected: self.field_size(),
                actual: field.len(),
            });
        }
        if field.len() != self.field_size() {
            return Err(GpuHolographicError::DimensionMismatch {
                expected: self.field_size(),
                actual: field.len(),
            });
        }

        let finite = field
            .scalars()
            .iter()
            .chain(field.bivectors().iter())
            .chain(field.amplitudes().iter())
            .all(|value| value.is_finite());
        if !finite {
            return Err(GpuHolographicError::BufferError(
                "optical field contains non-finite values".to_string(),
            ));
        }

        Ok(())
    }

    async fn read_buffer(
        &self,
        buffer: &wgpu::Buffer,
        len: usize,
    ) -> GpuHolographicResult<Vec<f32>> {
        let slice = buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);

        receiver
            .await
            .map_err(|_| GpuHolographicError::BufferError("Channel error".to_string()))?
            .map_err(|e| GpuHolographicError::BufferError(e.to_string()))?;

        let data = slice.get_mapped_range();
        let result: Vec<f32> = bytemuck::cast_slice(&data)[..len].to_vec();
        drop(data);
        buffer.unmap();

        Ok(result)
    }

    async fn read_buffer_u32(
        &self,
        buffer: &wgpu::Buffer,
        len: usize,
    ) -> GpuHolographicResult<Vec<u32>> {
        let slice = buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);

        receiver
            .await
            .map_err(|_| GpuHolographicError::BufferError("Channel error".to_string()))?
            .map_err(|e| GpuHolographicError::BufferError(e.to_string()))?;

        let data = slice.get_mapped_range();
        let result: Vec<u32> = bytemuck::cast_slice(&data)[..len].to_vec();
        drop(data);
        buffer.unmap();

        Ok(result)
    }

    // ========================================================================
    // Pipeline Creation
    // ========================================================================

    fn create_bind_pipeline(device: &wgpu::Device) -> GpuHolographicResult<wgpu::ComputePipeline> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Optical Bind Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(OPTICAL_BIND_SHADER)),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Optical Bind Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            }),
        )
    }

    fn create_similarity_pipeline(
        device: &wgpu::Device,
    ) -> GpuHolographicResult<wgpu::ComputePipeline> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Optical Similarity Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(OPTICAL_SIMILARITY_SHADER)),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Optical Similarity Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            }),
        )
    }

    fn create_bundle_pipeline(
        device: &wgpu::Device,
    ) -> GpuHolographicResult<wgpu::ComputePipeline> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Optical Bundle Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(OPTICAL_BUNDLE_SHADER)),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Optical Bundle Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            }),
        )
    }

    fn create_lee_encode_pipeline(
        device: &wgpu::Device,
    ) -> GpuHolographicResult<wgpu::ComputePipeline> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lee Encode Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(LEE_ENCODE_SHADER)),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Lee Encode Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            }),
        )
    }
}

// ============================================================================
// WGSL Compute Shaders for Optical Operations
// ============================================================================

/// Optical rotor bind shader.
///
/// Computes element-wise rotor product:
/// scalar_out = a_s * b_s - a_b * b_b
/// bivector_out = a_s * b_b + a_b * b_s
/// amplitude_out = a_amp * b_amp
const OPTICAL_BIND_SHADER: &str = r#"
@group(0) @binding(0) var<storage, read> a_scalar: array<f32>;
@group(0) @binding(1) var<storage, read> a_bivector: array<f32>;
@group(0) @binding(2) var<storage, read> a_amplitude: array<f32>;
@group(0) @binding(3) var<storage, read> b_scalar: array<f32>;
@group(0) @binding(4) var<storage, read> b_bivector: array<f32>;
@group(0) @binding(5) var<storage, read> b_amplitude: array<f32>;
@group(0) @binding(6) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    let n = arrayLength(&a_scalar);
    if (idx >= n) {
        return;
    }

    let a_s = a_scalar[idx];
    let a_b = a_bivector[idx];
    let b_s = b_scalar[idx];
    let b_b = b_bivector[idx];

    // Rotor product: (a_s + a_b*e12)(b_s + b_b*e12).
    // Packed output layout: [scalar..., bivector..., amplitude...].
    output[idx] = a_s * b_s - a_b * b_b;
    output[n + idx] = a_s * b_b + a_b * b_s;
    output[2u * n + idx] = a_amplitude[idx] * b_amplitude[idx];
}
"#;

/// Optical similarity shader with workgroup reduction.
///
/// Computes partial sums of: inner_product, norm_a_sq, norm_b_sq
const OPTICAL_SIMILARITY_SHADER: &str = r#"
@group(0) @binding(0) var<storage, read> a_scalar: array<f32>;
@group(0) @binding(1) var<storage, read> a_bivector: array<f32>;
@group(0) @binding(2) var<storage, read> a_amplitude: array<f32>;
@group(0) @binding(3) var<storage, read> b_scalar: array<f32>;
@group(0) @binding(4) var<storage, read> b_bivector: array<f32>;
@group(0) @binding(5) var<storage, read> b_amplitude: array<f32>;
@group(0) @binding(6) var<storage, read_write> partials: array<f32>;

var<workgroup> shared_sum: array<f32, 256>;
var<workgroup> shared_norm_a: array<f32, 256>;
var<workgroup> shared_norm_b: array<f32, 256>;

@compute @workgroup_size(256)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) wg_id: vec3<u32>
) {
    let idx = global_id.x;
    let lid = local_id.x;
    let n = arrayLength(&a_scalar);

    // Load and compute local values
    var local_sum: f32 = 0.0;
    var local_norm_a: f32 = 0.0;
    var local_norm_b: f32 = 0.0;

    if (idx < n) {
        let a_s = a_scalar[idx];
        let a_b = a_bivector[idx];
        let b_s = b_scalar[idx];
        let b_b = b_bivector[idx];
        let a_amp = a_amplitude[idx];
        let b_amp = b_amplitude[idx];

        // Inner product: a_s*b_s + a_b*b_b (note: + because of conjugate)
        let inner = a_s * b_s + a_b * b_b;
        local_sum = a_amp * b_amp * inner;
        local_norm_a = a_amp * a_amp;
        local_norm_b = b_amp * b_amp;
    }

    shared_sum[lid] = local_sum;
    shared_norm_a[lid] = local_norm_a;
    shared_norm_b[lid] = local_norm_b;
    workgroupBarrier();

    // Tree reduction
    for (var s = 128u; s > 0u; s = s >> 1u) {
        if (lid < s) {
            shared_sum[lid] += shared_sum[lid + s];
            shared_norm_a[lid] += shared_norm_a[lid + s];
            shared_norm_b[lid] += shared_norm_b[lid + s];
        }
        workgroupBarrier();
    }

    // Write partial result
    if (lid == 0u) {
        let wg = wg_id.x;
        partials[wg * 3u] = shared_sum[0];
        partials[wg * 3u + 1u] = shared_norm_a[0];
        partials[wg * 3u + 2u] = shared_norm_b[0];
    }
}
"#;

/// Optical bundle shader (placeholder - full implementation would handle variable-length input).
const OPTICAL_BUNDLE_SHADER: &str = r#"
@group(0) @binding(0) var<storage, read> input_scalar: array<f32>;
@group(0) @binding(1) var<storage, read> input_bivector: array<f32>;
@group(0) @binding(2) var<storage, read> input_amplitude: array<f32>;
@group(0) @binding(3) var<storage, read> weights: array<f32>;
@group(0) @binding(4) var<storage, read_write> out_scalar: array<f32>;
@group(0) @binding(5) var<storage, read_write> out_bivector: array<f32>;
@group(0) @binding(6) var<storage, read_write> out_amplitude: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Placeholder - full implementation would handle multi-field bundle
    let idx = global_id.x;
    if (idx >= arrayLength(&input_scalar)) {
        return;
    }
    out_scalar[idx] = input_scalar[idx];
    out_bivector[idx] = input_bivector[idx];
    out_amplitude[idx] = input_amplitude[idx];
}
"#;

/// Lee hologram encoding shader.
///
/// Each thread handles 32 pixels and packs results into a u32.
const LEE_ENCODE_SHADER: &str = r#"
@group(0) @binding(0) var<storage, read> field_scalar: array<f32>;
@group(0) @binding(1) var<storage, read> field_bivector: array<f32>;
@group(0) @binding(2) var<storage, read> field_amplitude: array<f32>;
@group(0) @binding(3) var<storage, read> carrier_scalar: array<f32>;
@group(0) @binding(4) var<storage, read> carrier_bivector: array<f32>;
@group(0) @binding(5) var<storage, read_write> output: array<u32>;

const PI: f32 = 3.14159265359;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let word_idx = global_id.x;
    if (word_idx >= arrayLength(&output)) {
        return;
    }

    let n = arrayLength(&field_scalar);
    var packed: u32 = 0u;

    for (var bit = 0u; bit < 32u; bit = bit + 1u) {
        let pixel_idx = word_idx * 32u + bit;
        if (pixel_idx >= n) {
            break;
        }

        let f_s = field_scalar[pixel_idx];
        let f_b = field_bivector[pixel_idx];
        let c_s = carrier_scalar[pixel_idx];
        let c_b = carrier_bivector[pixel_idx];
        let amp = field_amplitude[pixel_idx];

        // Modulated scalar = c_s * f_s - c_b * f_b
        let modulated_scalar = c_s * f_s - c_b * f_b;

        // Threshold = cos(π * amplitude)
        let threshold = cos(PI * amp);

        if (modulated_scalar > threshold) {
            packed = packed | (1u << bit);
        }
    }

    output[word_idx] = packed;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_use_gpu() {
        assert!(!GpuHolographic::should_use_gpu(10));
        assert!(GpuHolographic::should_use_gpu(100));
        assert!(GpuHolographic::should_use_gpu(1000));
    }

    #[ignore] // Requires GPU initialization
    #[tokio::test]
    async fn test_batch_similarity_cpu() {
        // Test CPU fallback
        let dim = 256;

        // Create test data
        let a: Vec<f64> = (0..dim).map(|i| (i as f64) / dim as f64).collect();
        let b = a.clone();

        // Use CPU path directly (small batch)
        if let Ok(gpu) = GpuHolographic::new(dim).await {
            let result = gpu.batch_similarity_cpu(&a, &b).unwrap();
            assert_eq!(result.len(), 1);
            // Self-similarity should be close to 1.0
            assert!((result[0] - 1.0).abs() < 0.01);
        }
    }

    #[ignore] // Requires GPU initialization
    #[tokio::test]
    async fn test_batch_bundle_cpu() {
        // Test CPU fallback for bundling
        let dim = 256;

        // Use small values to avoid numerical issues
        let a: Vec<f64> = (0..dim).map(|i| (i as f64) / 1000.0).collect();
        let b: Vec<f64> = (0..dim).map(|i| (i as f64) / 500.0).collect();

        if let Ok(gpu) = GpuHolographic::new(dim).await {
            let result = gpu.batch_bundle_cpu(&a, &b).unwrap();
            assert_eq!(result.len(), dim);
            // Check sum - bundle adds corresponding coefficients
            for i in 0..dim {
                assert!(
                    (result[i] - (a[i] + b[i])).abs() < 0.01,
                    "Mismatch at {}: {} vs {}",
                    i,
                    result[i],
                    a[i] + b[i]
                );
            }
        }
    }

    // ========================================================================
    // GPU Optical Field Tests
    // ========================================================================

    #[test]
    fn test_optical_should_use_gpu() {
        // Field size < 4096 should use CPU
        assert!(!GpuOpticalField::should_use_gpu(100));
        assert!(!GpuOpticalField::should_use_gpu(4095));

        // Field size >= 4096 should use GPU
        assert!(GpuOpticalField::should_use_gpu(4096));
        assert!(GpuOpticalField::should_use_gpu(65536)); // 256x256
    }

    #[ignore] // Requires GPU initialization
    #[tokio::test]
    async fn test_optical_bind_cpu() {
        // Test CPU fallback for bind operation (small field)
        let dims = (16, 16); // 256 pixels - uses CPU fallback

        if let Ok(gpu) = GpuOpticalField::new(dims).await {
            // Create two fields with known phases
            let field_a = OpticalRotorField::uniform(0.0, 1.0, dims);
            let field_b = OpticalRotorField::uniform(std::f32::consts::FRAC_PI_4, 1.0, dims);

            let bound = gpu.bind(&field_a, &field_b).await.unwrap();

            // Binding adds phases: 0 + π/4 = π/4
            for y in 0..16 {
                for x in 0..16 {
                    let phase = bound.phase_at(x, y);
                    assert!(
                        (phase - std::f32::consts::FRAC_PI_4).abs() < 1e-5,
                        "Phase mismatch at ({}, {}): {}",
                        x,
                        y,
                        phase
                    );
                }
            }
        }
    }

    #[ignore] // Requires GPU initialization
    #[tokio::test]
    async fn test_optical_similarity_cpu() {
        // Test CPU fallback for similarity
        let dims = (16, 16);

        if let Ok(gpu) = GpuOpticalField::new(dims).await {
            let field_a = OpticalRotorField::random(dims, 42);
            let field_b = field_a.clone();

            // Self-similarity should be 1.0
            let sim = gpu.similarity(&field_a, &field_b).await.unwrap();
            assert!(
                (sim - 1.0).abs() < 1e-5,
                "Self-similarity should be 1.0, got {}",
                sim
            );

            // Different fields should have low similarity
            let field_c = OpticalRotorField::random(dims, 99);
            let sim2 = gpu.similarity(&field_a, &field_c).await.unwrap();
            assert!(
                sim2.abs() < 0.3,
                "Random fields should have low similarity, got {}",
                sim2
            );
        }
    }

    #[ignore] // Requires GPU initialization
    #[tokio::test]
    async fn test_optical_lee_encode_cpu() {
        // Test CPU fallback for Lee encoding
        let dims = (32, 32);

        if let Ok(gpu) = GpuOpticalField::new(dims).await {
            let field = OpticalRotorField::uniform(0.0, 0.5, dims);
            let config = LeeEncoderConfig::new(dims, 0.25);

            let hologram = gpu.encode_lee(&field, &config).await.unwrap();

            assert_eq!(hologram.dimensions(), dims);

            // Fill factor should be reasonable
            let fill = hologram.fill_factor();
            assert!(fill > 0.2 && fill < 0.8, "Unexpected fill factor: {}", fill);
        }
    }
}
