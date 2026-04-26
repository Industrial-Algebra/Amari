//! GPU-backed and GPU-ready functional analysis operations.
//!
//! Current public behavior is intentionally explicit:
//!
//! - **Matrix Operations**: GPU-backed batch matrix-vector products; matrix multiplication currently uses CPU readback fallback for correctness across independently-created GPU operators.
//! - **Spectral Decomposition**: CPU Jacobi/spectral baseline through the GPU API after matrix readback.
//! - **Functional Calculus**: CPU spectral functional calculus; batch helper is CPU-backed.
//! - **Hilbert Space Operations**: GPU-backed batch inner products and norms.
//! - **Adaptive Dispatch**: chooses GPU for validated matrix batch paths and CPU for small/fallback paths.
//!
//! # Quick Start
//!
//! ```ignore
//! use amari_gpu::functional::{GpuMatrixOperator, GpuSpectralDecomposition};
//! use amari_functional::MatrixOperator;
//!
//! // Create GPU matrix from CPU matrix
//! let gpu_matrix = GpuMatrixOperator::from_matrix_operator(&matrix).await?;
//!
//! // Batch matrix-vector products
//! let results = gpu_matrix.apply_batch(&vectors).await?;
//!
//! // Spectral decomposition currently uses the CPU spectral baseline after GPU readback.
//! let decomp = GpuSpectralDecomposition::compute(&gpu_matrix, 100, 1e-10).await?;
//! ```

use crate::GpuError;
use amari_core::Multivector;
use amari_functional::{
    Eigenpair, Eigenvalue, LinearOperator, MatrixOperator, SpectralDecomposition,
};
use bytemuck::{Pod, Zeroable};
use thiserror::Error;
use wgpu::util::DeviceExt;

/// Errors specific to GPU functional analysis operations
#[derive(Error, Debug)]
pub enum GpuFunctionalError {
    /// GPU initialization failed
    #[error("GPU initialization error: {0}")]
    InitializationError(String),

    /// Buffer operation failed
    #[error("GPU buffer error: {0}")]
    BufferError(String),

    /// Dimension mismatch in matrix operations
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Convergence failure in iterative algorithms
    #[error("Algorithm did not converge after {iterations} iterations")]
    ConvergenceError { iterations: usize },

    /// Matrix is not symmetric (required for spectral decomposition)
    #[error("Matrix must be symmetric for spectral decomposition")]
    NotSymmetric,

    /// Underlying GPU error
    #[error("GPU error: {0}")]
    GpuError(#[from] GpuError),
}

/// Result type for GPU functional operations
pub type GpuFunctionalResult<T> = Result<T, GpuFunctionalError>;

/// GPU-accelerated matrix operator for Clifford algebra spaces
///
/// Provides high-performance matrix operations using WebGPU compute shaders.
/// The matrix is stored in GPU memory for efficient batch operations.
pub struct GpuMatrixOperator<const P: usize, const Q: usize, const R: usize> {
    device: wgpu::Device,
    queue: wgpu::Queue,
    matrix_buffer: wgpu::Buffer,
    apply_pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)] // Reserved for same-device GPU matrix multiplication restoration.
    multiply_pipeline: wgpu::ComputePipeline,
    rows: usize,
    cols: usize,
}

impl<const P: usize, const Q: usize, const R: usize> GpuMatrixOperator<P, Q, R> {
    /// Dimension of the multivector space
    const DIM: usize = 1 << (P + Q + R);

    /// Create a new GPU matrix operator from a CPU MatrixOperator
    pub async fn from_matrix_operator(
        matrix: &MatrixOperator<P, Q, R>,
    ) -> GpuFunctionalResult<Self> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| {
                GpuFunctionalError::InitializationError("No GPU adapter found".to_string())
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Amari Functional GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuFunctionalError::InitializationError(e.to_string()))?;

        // Convert matrix to f32 for GPU (row-major order)
        let matrix_data: Vec<f32> = (0..Self::DIM)
            .flat_map(|i| (0..Self::DIM).map(move |j| matrix.get(i, j) as f32))
            .collect();

        let matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Matrix Buffer"),
            contents: bytemuck::cast_slice(&matrix_data),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });

        // Create compute pipelines
        let apply_pipeline = Self::create_apply_pipeline(&device)?;
        let multiply_pipeline = Self::create_multiply_pipeline(&device)?;

        Ok(Self {
            device,
            queue,
            matrix_buffer,
            apply_pipeline,
            multiply_pipeline,
            rows: Self::DIM,
            cols: Self::DIM,
        })
    }

    /// Get the dimension of the matrix
    pub fn dimension(&self) -> usize {
        self.rows
    }

    /// Apply matrix to a batch of vectors on GPU
    ///
    /// Computes M × v for each vector v in the batch.
    /// Returns results in the same order as input.
    pub async fn apply_batch(
        &self,
        vectors: &[Multivector<P, Q, R>],
    ) -> GpuFunctionalResult<Vec<Multivector<P, Q, R>>> {
        if vectors.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = vectors.len();

        // Flatten input vectors to f32
        let input_data: Vec<f32> = vectors
            .iter()
            .flat_map(|v| v.to_vec().into_iter().map(|x| x as f32))
            .collect();

        // Create input buffer
        let input_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Input Vectors"),
                contents: bytemuck::cast_slice(&input_data),
                usage: wgpu::BufferUsages::STORAGE,
            });

        // Create output buffer
        let output_size = (batch_size * Self::DIM * std::mem::size_of::<f32>()) as u64;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Vectors"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create staging buffer for readback
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create params buffer
        let params = MatrixApplyParams {
            rows: self.rows as u32,
            cols: self.cols as u32,
            batch_size: batch_size as u32,
            _padding: 0,
        };
        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Params"),
                contents: bytemuck::cast_slice(&[params]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        // Create bind group
        let bind_group_layout = self.apply_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Apply Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.matrix_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: input_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Execute compute pass
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Apply Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Apply Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.apply_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            // One thread per output element
            let total_outputs = (batch_size * Self::DIM) as u32;
            let workgroup_count = total_outputs.div_ceil(64);
            compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
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
            .map_err(|_| GpuFunctionalError::BufferError("Channel error".to_string()))?
            .map_err(|e| GpuFunctionalError::BufferError(format!("{:?}", e)))?;

        let data = buffer_slice.get_mapped_range();
        let results_f32: &[f32] = bytemuck::cast_slice(&data);

        // Convert back to multivectors
        let results: Vec<Multivector<P, Q, R>> = results_f32
            .chunks(Self::DIM)
            .map(|chunk| {
                let coeffs: Vec<f64> = chunk.iter().map(|&x| x as f64).collect();
                Multivector::from_coefficients(coeffs)
            })
            .collect();

        drop(data);
        staging_buffer.unmap();

        Ok(results)
    }

    /// Multiply this matrix with another using CPU readback fallback.
    ///
    /// Independently-created `GpuMatrixOperator`s own independent `wgpu::Device`s,
    /// so sharing `other`'s buffer in `self`'s command encoder is not generally
    /// valid. Until a shared-context constructor is added, this method reads both
    /// matrices back and multiplies them on CPU to preserve public correctness.
    pub async fn multiply(
        &self,
        other: &GpuMatrixOperator<P, Q, R>,
    ) -> GpuFunctionalResult<MatrixOperator<P, Q, R>> {
        if self.cols != other.rows {
            return Err(GpuFunctionalError::DimensionMismatch {
                expected: self.cols,
                actual: other.rows,
            });
        }

        let left = self.to_matrix_operator().await?;
        let right = other.to_matrix_operator().await?;

        let mut entries = vec![0.0; self.rows * other.cols];
        for row in 0..self.rows {
            for col in 0..other.cols {
                let mut sum = 0.0;
                for k in 0..self.cols {
                    sum += left.get(row, k) * right.get(k, col);
                }
                entries[row * other.cols + col] = sum;
            }
        }

        MatrixOperator::new(entries, self.rows, other.cols).map_err(|e| {
            GpuFunctionalError::BufferError(format!("Failed to create matrix: {:?}", e))
        })
    }

    /// Convert back to CPU MatrixOperator
    pub async fn to_matrix_operator(&self) -> GpuFunctionalResult<MatrixOperator<P, Q, R>> {
        let size = (self.rows * self.cols * std::mem::size_of::<f32>()) as u64;

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Copy Encoder"),
            });

        encoder.copy_buffer_to_buffer(&self.matrix_buffer, 0, &staging_buffer, 0, size);

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .await
            .map_err(|_| GpuFunctionalError::BufferError("Channel error".to_string()))?
            .map_err(|e| GpuFunctionalError::BufferError(format!("{:?}", e)))?;

        let data = buffer_slice.get_mapped_range();
        let data_f32: &[f32] = bytemuck::cast_slice(&data);
        let entries: Vec<f64> = data_f32.iter().map(|&x| x as f64).collect();

        drop(data);
        staging_buffer.unmap();

        MatrixOperator::new(entries, self.rows, self.cols).map_err(|e| {
            GpuFunctionalError::BufferError(format!("Failed to create matrix: {:?}", e))
        })
    }

    /// Heuristic to determine if GPU should be used
    pub fn should_use_gpu(batch_size: usize) -> bool {
        // GPU is beneficial for batch operations with many vectors
        // or for large matrices (dimension >= 16)
        batch_size >= 100 || Self::DIM >= 16
    }

    fn create_apply_pipeline(device: &wgpu::Device) -> GpuFunctionalResult<wgpu::ComputePipeline> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Matrix Apply Shader"),
            source: wgpu::ShaderSource::Wgsl(MATRIX_VECTOR_SHADER.into()),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Matrix Apply Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "matrix_vector_multiply",
            }),
        )
    }

    fn create_multiply_pipeline(
        device: &wgpu::Device,
    ) -> GpuFunctionalResult<wgpu::ComputePipeline> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Matrix Multiply Shader"),
            source: wgpu::ShaderSource::Wgsl(MATRIX_MULTIPLY_SHADER.into()),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Matrix Multiply Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "matrix_multiply",
            }),
        )
    }
}

/// GPU-ready spectral decomposition wrapper.
///
/// Current 0.20.0 behavior reads the matrix back from GPU storage and uses the
/// `amari-functional` CPU spectral baseline. This is deliberately documented as
/// CPU-backed until a validated GPU eigensolver is added.
pub struct GpuSpectralDecomposition<const P: usize, const Q: usize, const R: usize> {
    eigenvalues: Vec<f64>,
    eigenvectors: Vec<Multivector<P, Q, R>>,
    is_complete: bool,
}

impl<const P: usize, const Q: usize, const R: usize> GpuSpectralDecomposition<P, Q, R> {
    /// Compute spectral decomposition with the CPU spectral baseline.
    ///
    /// The input matrix is read back from GPU storage, checked for symmetry, and
    /// decomposed with `amari-functional`'s CPU Jacobi/spectral implementation.
    pub async fn compute(
        matrix: &GpuMatrixOperator<P, Q, R>,
        max_iterations: usize,
        tolerance: f64,
    ) -> GpuFunctionalResult<Self> {
        let cpu_matrix = matrix.to_matrix_operator().await?;

        if !cpu_matrix.is_symmetric(tolerance) {
            return Err(GpuFunctionalError::NotSymmetric);
        }

        let decomposition =
            amari_functional::spectral_decompose(&cpu_matrix, max_iterations, tolerance).map_err(
                |_e| GpuFunctionalError::ConvergenceError {
                    iterations: max_iterations,
                },
            )?;

        let eigenvalues: Vec<f64> = decomposition
            .eigenpairs()
            .iter()
            .map(|pair| pair.eigenvalue.value)
            .collect();
        let eigenvectors: Vec<Multivector<P, Q, R>> = decomposition
            .eigenpairs()
            .iter()
            .map(|pair| pair.eigenvector.clone())
            .collect();

        Ok(Self {
            eigenvalues,
            eigenvectors,
            is_complete: decomposition.is_complete(),
        })
    }

    /// Get eigenvalues
    pub fn eigenvalues(&self) -> &[f64] {
        &self.eigenvalues
    }

    /// Get eigenvectors
    pub fn eigenvectors(&self) -> &[Multivector<P, Q, R>] {
        &self.eigenvectors
    }

    /// Check if decomposition is complete
    pub fn is_complete(&self) -> bool {
        self.is_complete
    }

    /// Spectral radius (maximum absolute eigenvalue)
    pub fn spectral_radius(&self) -> f64 {
        self.eigenvalues
            .iter()
            .map(|&e| e.abs())
            .fold(0.0, f64::max)
    }

    /// Condition number (ratio of largest to smallest absolute eigenvalue)
    pub fn condition_number(&self) -> Option<f64> {
        let min = self
            .eigenvalues
            .iter()
            .map(|&e| e.abs())
            .filter(|&e| e > 1e-14)
            .fold(f64::INFINITY, f64::min);

        if min == f64::INFINITY {
            return None;
        }

        let max = self.spectral_radius();
        Some(max / min)
    }

    /// Check if positive definite (all eigenvalues positive)
    pub fn is_positive_definite(&self) -> bool {
        self.eigenvalues.iter().all(|&e| e > 0.0)
    }

    /// Check if positive semi-definite (all eigenvalues non-negative)
    pub fn is_positive_semidefinite(&self) -> bool {
        self.eigenvalues.iter().all(|&e| e >= -1e-14)
    }

    /// Apply a function to the operator via functional calculus
    ///
    /// For f(A) where A = Σᵢ λᵢ |vᵢ⟩⟨vᵢ|, computes f(A)x = Σᵢ f(λᵢ) ⟨vᵢ,x⟩ vᵢ
    pub fn apply_function<F>(&self, f: F, x: &Multivector<P, Q, R>) -> Multivector<P, Q, R>
    where
        F: Fn(f64) -> f64,
    {
        let x_coeffs = x.to_vec();
        let mut result = Multivector::<P, Q, R>::zero();

        for (eigenvalue, eigenvector) in self.eigenvalues.iter().zip(self.eigenvectors.iter()) {
            let v_coeffs = eigenvector.to_vec();

            // Inner product ⟨vᵢ, x⟩
            let inner_product: f64 = v_coeffs
                .iter()
                .zip(x_coeffs.iter())
                .map(|(a, b)| a * b)
                .sum();

            // f(λᵢ) ⟨vᵢ, x⟩ vᵢ
            let f_lambda = f(*eigenvalue);
            let scaled = eigenvector.clone() * (f_lambda * inner_product);
            result = result + scaled;
        }

        result
    }

    /// Batch apply function to multiple vectors using the CPU spectral baseline.
    pub async fn apply_function_batch<F>(
        &self,
        f: F,
        vectors: &[Multivector<P, Q, R>],
    ) -> Vec<Multivector<P, Q, R>>
    where
        F: Fn(f64) -> f64,
    {
        // TODO(0.20.x+): replace with validated GPU batch inner products/projections.
        vectors.iter().map(|x| self.apply_function(&f, x)).collect()
    }

    /// Convert to CPU SpectralDecomposition
    pub fn to_spectral_decomposition(&self) -> SpectralDecomposition<P, Q, R> {
        let eigenpairs: Vec<Eigenpair<Multivector<P, Q, R>>> = self
            .eigenvalues
            .iter()
            .zip(self.eigenvectors.iter())
            .map(|(&value, eigenvector)| Eigenpair {
                eigenvalue: Eigenvalue {
                    value,
                    multiplicity: None,
                },
                eigenvector: eigenvector.clone(),
            })
            .collect();

        SpectralDecomposition::new(eigenpairs)
    }
}

/// GPU-accelerated Hilbert space operations
///
/// Provides batch inner products, norms, and projections.
pub struct GpuHilbertSpace<const P: usize, const Q: usize, const R: usize> {
    device: wgpu::Device,
    queue: wgpu::Queue,
    inner_product_pipeline: wgpu::ComputePipeline,
}

impl<const P: usize, const Q: usize, const R: usize> GpuHilbertSpace<P, Q, R> {
    /// Dimension of the multivector space
    const DIM: usize = 1 << (P + Q + R);

    /// Create a new GPU Hilbert space
    pub async fn new() -> GpuFunctionalResult<Self> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| {
                GpuFunctionalError::InitializationError("No GPU adapter found".to_string())
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Amari Hilbert GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuFunctionalError::InitializationError(e.to_string()))?;

        let inner_product_pipeline = Self::create_inner_product_pipeline(&device)?;

        Ok(Self {
            device,
            queue,
            inner_product_pipeline,
        })
    }

    /// Compute batch inner products: ⟨xᵢ, yᵢ⟩ for each pair
    pub async fn inner_product_batch(
        &self,
        xs: &[Multivector<P, Q, R>],
        ys: &[Multivector<P, Q, R>],
    ) -> GpuFunctionalResult<Vec<f64>> {
        if xs.len() != ys.len() {
            return Err(GpuFunctionalError::DimensionMismatch {
                expected: xs.len(),
                actual: ys.len(),
            });
        }

        if xs.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = xs.len();

        // Flatten input vectors
        let x_data: Vec<f32> = xs
            .iter()
            .flat_map(|v| v.to_vec().into_iter().map(|x| x as f32))
            .collect();
        let y_data: Vec<f32> = ys
            .iter()
            .flat_map(|v| v.to_vec().into_iter().map(|x| x as f32))
            .collect();

        let x_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("X Vectors"),
                contents: bytemuck::cast_slice(&x_data),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let y_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Y Vectors"),
                contents: bytemuck::cast_slice(&y_data),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let output_size = (batch_size * std::mem::size_of::<f32>()) as u64;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Inner Products"),
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

        let params = InnerProductParams {
            dimension: Self::DIM as u32,
            batch_size: batch_size as u32,
        };
        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Params"),
                contents: bytemuck::cast_slice(&[params]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let bind_group_layout = self.inner_product_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Inner Product Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: x_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: y_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Inner Product Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Inner Product Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.inner_product_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            let workgroup_count = (batch_size as u32).div_ceil(256);
            compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .await
            .map_err(|_| GpuFunctionalError::BufferError("Channel error".to_string()))?
            .map_err(|e| GpuFunctionalError::BufferError(format!("{:?}", e)))?;

        let data = buffer_slice.get_mapped_range();
        let results_f32: &[f32] = bytemuck::cast_slice(&data);
        let results: Vec<f64> = results_f32.iter().map(|&x| x as f64).collect();

        drop(data);
        staging_buffer.unmap();

        Ok(results)
    }

    /// Compute batch norms: ||xᵢ|| for each vector
    pub async fn norm_batch(
        &self,
        vectors: &[Multivector<P, Q, R>],
    ) -> GpuFunctionalResult<Vec<f64>> {
        let norms_sq = self.inner_product_batch(vectors, vectors).await?;
        Ok(norms_sq.iter().map(|&x| x.sqrt()).collect())
    }

    fn create_inner_product_pipeline(
        device: &wgpu::Device,
    ) -> GpuFunctionalResult<wgpu::ComputePipeline> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Inner Product Shader"),
            source: wgpu::ShaderSource::Wgsl(INNER_PRODUCT_SHADER.into()),
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Inner Product Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "batch_inner_product",
            }),
        )
    }
}

/// Adaptive CPU/GPU dispatcher for functional analysis.
///
/// Matrix batch application uses GPU for large batches when available and CPU for
/// small batches. Spectral decomposition currently returns the CPU spectral
/// baseline even when routed through the GPU matrix wrapper.
pub struct AdaptiveFunctionalCompute<const P: usize, const Q: usize, const R: usize> {
    gpu_available: bool,
}

impl<const P: usize, const Q: usize, const R: usize> AdaptiveFunctionalCompute<P, Q, R> {
    /// Create adaptive compute with GPU detection
    pub async fn new() -> Self {
        // Try to initialize GPU
        let instance = wgpu::Instance::default();
        let gpu_available = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .is_some();

        Self { gpu_available }
    }

    /// Check if GPU is available
    pub fn has_gpu(&self) -> bool {
        self.gpu_available
    }

    /// Compute spectral decomposition with adaptive dispatch
    pub async fn spectral_decompose(
        &self,
        matrix: &MatrixOperator<P, Q, R>,
        max_iterations: usize,
        tolerance: f64,
    ) -> GpuFunctionalResult<SpectralDecomposition<P, Q, R>> {
        if self.gpu_available && GpuMatrixOperator::<P, Q, R>::should_use_gpu(1) {
            let gpu_matrix = GpuMatrixOperator::from_matrix_operator(matrix).await?;
            let gpu_decomp =
                GpuSpectralDecomposition::compute(&gpu_matrix, max_iterations, tolerance).await?;
            Ok(gpu_decomp.to_spectral_decomposition())
        } else {
            // CPU fallback
            amari_functional::spectral_decompose(matrix, max_iterations, tolerance).map_err(|_e| {
                GpuFunctionalError::ConvergenceError {
                    iterations: max_iterations,
                }
            })
        }
    }

    /// Batch matrix-vector products with adaptive dispatch
    pub async fn apply_batch(
        &self,
        matrix: &MatrixOperator<P, Q, R>,
        vectors: &[Multivector<P, Q, R>],
    ) -> GpuFunctionalResult<Vec<Multivector<P, Q, R>>> {
        if self.gpu_available && GpuMatrixOperator::<P, Q, R>::should_use_gpu(vectors.len()) {
            let gpu_matrix = GpuMatrixOperator::from_matrix_operator(matrix).await?;
            gpu_matrix.apply_batch(vectors).await
        } else {
            // CPU fallback
            vectors
                .iter()
                .map(|v| {
                    matrix.apply(v).map_err(|_| {
                        GpuFunctionalError::BufferError("Matrix apply failed".to_string())
                    })
                })
                .collect()
        }
    }
}

// ============================================================================
// Shader parameter structs
// ============================================================================

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct MatrixApplyParams {
    rows: u32,
    cols: u32,
    batch_size: u32,
    _padding: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct InnerProductParams {
    dimension: u32,
    batch_size: u32,
}

// ============================================================================
// WGSL Compute Shaders
// ============================================================================

/// WGSL shader for batch matrix-vector multiplication
const MATRIX_VECTOR_SHADER: &str = r#"
struct Params {
    rows: u32,
    cols: u32,
    batch_size: u32,
    _padding: u32,
}

@group(0) @binding(0)
var<uniform> params: Params;

@group(0) @binding(1)
var<storage, read> matrix: array<f32>;

@group(0) @binding(2)
var<storage, read> input_vectors: array<f32>;

@group(0) @binding(3)
var<storage, read_write> output_vectors: array<f32>;

@compute @workgroup_size(64)
fn matrix_vector_multiply(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let output_idx = global_id.x;
    let total_outputs = params.batch_size * params.rows;

    if output_idx >= total_outputs {
        return;
    }

    // Determine which vector and which row
    let vector_idx = output_idx / params.rows;
    let row = output_idx % params.rows;

    // Compute dot product of matrix row with input vector
    var sum: f32 = 0.0;
    let vector_offset = vector_idx * params.cols;

    for (var col: u32 = 0u; col < params.cols; col = col + 1u) {
        let matrix_idx = row * params.cols + col;
        let vector_val = input_vectors[vector_offset + col];
        sum = sum + matrix[matrix_idx] * vector_val;
    }

    output_vectors[output_idx] = sum;
}
"#;

/// WGSL shader for matrix multiplication
const MATRIX_MULTIPLY_SHADER: &str = r#"
struct Params {
    m: u32,  // rows of A and result
    n: u32,  // cols of B and result
    k: u32,  // cols of A, rows of B
    _padding: u32,
}

@group(0) @binding(0)
var<uniform> params: Params;

@group(0) @binding(1)
var<storage, read> matrix_a: array<f32>;

@group(0) @binding(2)
var<storage, read> matrix_b: array<f32>;

@group(0) @binding(3)
var<storage, read_write> result: array<f32>;

@compute @workgroup_size(64)
fn matrix_multiply(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let output_idx = global_id.x;
    let total_elements = params.m * params.n;

    if output_idx >= total_elements {
        return;
    }

    // Determine row and column
    let row = output_idx / params.n;
    let col = output_idx % params.n;

    // Compute dot product
    var sum: f32 = 0.0;
    for (var i: u32 = 0u; i < params.k; i = i + 1u) {
        let a_idx = row * params.k + i;
        let b_idx = i * params.n + col;
        sum = sum + matrix_a[a_idx] * matrix_b[b_idx];
    }

    result[output_idx] = sum;
}
"#;

/// WGSL shader for batch inner products
const INNER_PRODUCT_SHADER: &str = r#"
struct Params {
    dimension: u32,
    batch_size: u32,
}

@group(0) @binding(0)
var<uniform> params: Params;

@group(0) @binding(1)
var<storage, read> x_vectors: array<f32>;

@group(0) @binding(2)
var<storage, read> y_vectors: array<f32>;

@group(0) @binding(3)
var<storage, read_write> results: array<f32>;

@compute @workgroup_size(256)
fn batch_inner_product(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pair_idx = global_id.x;

    if pair_idx >= params.batch_size {
        return;
    }

    // Compute inner product for this pair
    var sum: f32 = 0.0;
    let offset = pair_idx * params.dimension;

    for (var i: u32 = 0u; i < params.dimension; i = i + 1u) {
        sum = sum + x_vectors[offset + i] * y_vectors[offset + i];
    }

    results[pair_idx] = sum;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_use_gpu() {
        assert!(!GpuMatrixOperator::<2, 0, 0>::should_use_gpu(10));
        assert!(GpuMatrixOperator::<2, 0, 0>::should_use_gpu(1000));
    }

    #[tokio::test]
    async fn test_gpu_functional_creation() {
        // Skip GPU tests in CI environments
        if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
            println!("Skipping GPU test in CI environment");
            return;
        }

        let matrix = MatrixOperator::<2, 0, 0>::identity();
        match GpuMatrixOperator::from_matrix_operator(&matrix).await {
            Ok(gpu_matrix) => {
                assert_eq!(gpu_matrix.dimension(), 4);
            }
            Err(GpuFunctionalError::InitializationError(_)) => {
                println!("GPU not available - skipping test");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_adaptive_compute() {
        let adaptive = AdaptiveFunctionalCompute::<2, 0, 0>::new().await;
        // Should work even without GPU (falls back to CPU)
        let matrix = MatrixOperator::<2, 0, 0>::diagonal(&[1.0, 2.0, 3.0, 4.0]).unwrap();
        let result = adaptive.spectral_decompose(&matrix, 100, 1e-10).await;
        assert!(result.is_ok());

        let decomp = result.unwrap();
        let eigenvalues = decomp.eigenvalues();
        assert_eq!(eigenvalues.len(), 4);
    }
}
