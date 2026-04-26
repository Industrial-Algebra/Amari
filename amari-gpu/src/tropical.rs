//! GPU acceleration for tropical algebra operations
//!
//! This module provides GPU-accelerated implementations of tropical algebra
//! operations including matrix multiplication, neural network attention,
//! and Viterbi algorithm computation using WebGPU compute shaders.

#[cfg(feature = "tropical")]
#[allow(unused_imports)]
use amari_tropical::{TropicalError, TropicalMatrix, TropicalMultivector, TropicalNumber};

#[cfg(feature = "tropical")]
use bytemuck::{Pod, Zeroable};

#[cfg(feature = "tropical")]
use num_traits::Float;

#[cfg(feature = "tropical")]
use std::collections::HashMap;

#[cfg(feature = "tropical")]
use wgpu::{util::DeviceExt, Buffer, BufferUsages};

#[cfg(feature = "tropical")]
use thiserror::Error;

/// GPU-specific error types for tropical algebra operations
#[cfg(feature = "tropical")]
#[derive(Error, Debug)]
pub enum TropicalGpuError {
    #[error("GPU initialization failed: {0}")]
    InitializationError(String),

    #[error("Shader compilation failed: {0}")]
    ShaderCompilation(String),

    #[error("Buffer operation failed: {0}")]
    BufferError(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Memory allocation failed: {0}")]
    MemoryAllocation(String),

    #[error("Tropical algebra error: {0}")]
    TropicalError(#[from] TropicalError),
}

#[cfg(feature = "tropical")]
pub type TropicalGpuResult<T> = Result<T, TropicalGpuError>;

/// GPU buffer representation for tropical numbers
#[cfg(feature = "tropical")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuTropicalNumber {
    pub value: f32,
}

#[cfg(feature = "tropical")]
impl From<TropicalNumber<f32>> for GpuTropicalNumber {
    fn from(t: TropicalNumber<f32>) -> Self {
        Self { value: t.value() }
    }
}

#[cfg(feature = "tropical")]
impl From<GpuTropicalNumber> for TropicalNumber<f32> {
    fn from(gpu: GpuTropicalNumber) -> Self {
        TropicalNumber::new(gpu.value)
    }
}

/// GPU buffer representation for tropical matrices
#[cfg(feature = "tropical")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuTropicalMatrixHeader {
    pub rows: u32,
    pub cols: u32,
    pub _padding: [u32; 2], // Ensure 16-byte alignment
}

/// GPU parameter block for tropical matrix multiplication
#[cfg(feature = "tropical")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuTropicalMatMulParams {
    pub rows_a: u32,
    pub cols_a: u32,
    pub cols_b: u32,
    pub _padding: u32,
}

/// GPU parameter block for tropical winner-takes-all attention scores.
#[cfg(feature = "tropical")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuTropicalAttentionParams {
    pub rows: u32,
    pub cols: u32,
    pub _padding: [u32; 2],
}

/// GPU context for tropical algebra operations
#[cfg(feature = "tropical")]
pub struct TropicalGpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    #[allow(dead_code)]
    shader_cache: HashMap<String, wgpu::ComputePipeline>,
}

#[cfg(feature = "tropical")]
impl TropicalGpuContext {
    /// Initialize GPU context with WebGPU
    pub async fn new() -> TropicalGpuResult<Self> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| {
                TropicalGpuError::InitializationError("No GPU adapter found".to_string())
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Amari Tropical GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| TropicalGpuError::InitializationError(e.to_string()))?;

        Ok(Self {
            device,
            queue,
            shader_cache: HashMap::new(),
        })
    }

    /// Create buffer with data
    pub fn create_buffer_with_data<T: bytemuck::Pod>(
        &self,
        label: &str,
        data: &[T],
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(data),
                usage,
            })
    }

    /// Read buffer data back to CPU
    pub async fn read_buffer<T: bytemuck::Pod + Clone>(
        &self,
        buffer: &wgpu::Buffer,
        size: u64,
    ) -> TropicalGpuResult<Vec<T>> {
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Copy Encoder"),
            });

        encoder.copy_buffer_to_buffer(buffer, 0, &staging_buffer, 0, size);
        self.queue.submit([encoder.finish()]);

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });

        self.device.poll(wgpu::Maintain::Wait);

        pollster::block_on(rx)
            .map_err(|_| TropicalGpuError::BufferError("Buffer read timeout".to_string()))?
            .map_err(|e| TropicalGpuError::BufferError(format!("Buffer map failed: {}", e)))?;

        let data = buffer_slice.get_mapped_range();
        let result: Vec<T> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }
}

/// Redesign-pending trait-based tropical GPU surface.
///
/// This block is intentionally isolated from the emerging v1 API, which is centered
/// on explicit `TropicalGpuOps` methods rather than placeholder-heavy type-attached GPU APIs.
#[cfg(feature = "tropical")]
#[allow(dead_code)]
mod redesign_pending {
    use super::*;

    pub trait TropicalGpuAccelerated<T> {
        /// Convert data to GPU buffer format
        fn to_gpu_buffer(&self, context: &TropicalGpuContext) -> TropicalGpuResult<wgpu::Buffer>;

        /// Reconstruct data from GPU buffer
        fn from_gpu_buffer(
            buffer: &wgpu::Buffer,
            context: &TropicalGpuContext,
        ) -> TropicalGpuResult<T>;

        /// Execute GPU operation with specified parameters
        fn gpu_operation(
            &self,
            operation: &str,
            context: &TropicalGpuContext,
            params: &HashMap<String, GpuParameter>,
        ) -> TropicalGpuResult<T>;
    }

    #[derive(Debug, Clone)]
    pub enum GpuParameter {
        Float(f32),
        Integer(i32),
        UnsignedInteger(u32),
        Buffer(String),
        Array(Vec<f32>),
    }

    impl<T: Float> TropicalGpuAccelerated<TropicalNumber<T>> for TropicalNumber<T>
    where
        T: bytemuck::Pod + Into<f32> + From<f32>,
    {
        fn to_gpu_buffer(&self, context: &TropicalGpuContext) -> TropicalGpuResult<Buffer> {
            let gpu_data = GpuTropicalNumber {
                value: self.value().into(),
            };

            let buffer = context.create_buffer_with_data(
                "TropicalNumber Buffer",
                &[gpu_data],
                BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            );

            Ok(buffer)
        }

        fn from_gpu_buffer(
            buffer: &Buffer,
            context: &TropicalGpuContext,
        ) -> TropicalGpuResult<TropicalNumber<T>> {
            let gpu_data: Vec<GpuTropicalNumber> = pollster::block_on(
                context.read_buffer(buffer, std::mem::size_of::<GpuTropicalNumber>() as u64),
            )?;

            if gpu_data.is_empty() {
                return Err(TropicalGpuError::InvalidOperation(
                    "Empty buffer data".to_string(),
                ));
            }

            Ok(TropicalNumber::new(<T as From<f32>>::from(
                gpu_data[0].value,
            )))
        }

        fn gpu_operation(
            &self,
            operation: &str,
            _context: &TropicalGpuContext,
            params: &HashMap<String, GpuParameter>,
        ) -> TropicalGpuResult<TropicalNumber<T>> {
            match operation {
                "tropical_add" => {
                    if let Some(GpuParameter::Buffer(_other_buffer_id)) = params.get("other") {
                        Ok(*self)
                    } else {
                        Err(TropicalGpuError::InvalidOperation(
                            "Missing 'other' parameter for tropical_add".to_string(),
                        ))
                    }
                }
                "tropical_mul" => {
                    if let Some(GpuParameter::Buffer(_other_buffer_id)) = params.get("other") {
                        Ok(*self)
                    } else {
                        Err(TropicalGpuError::InvalidOperation(
                            "Missing 'other' parameter for tropical_mul".to_string(),
                        ))
                    }
                }
                "tropical_pow" => {
                    if let Some(GpuParameter::Float(scalar)) = params.get("scalar") {
                        Ok(self.tropical_pow(<T as From<f32>>::from(*scalar)))
                    } else {
                        Err(TropicalGpuError::InvalidOperation(
                            "Missing 'scalar' parameter for tropical_pow".to_string(),
                        ))
                    }
                }
                _ => Err(TropicalGpuError::InvalidOperation(format!(
                    "Unknown operation: {}",
                    operation
                ))),
            }
        }
    }

    impl<T: Float> TropicalGpuAccelerated<TropicalMatrix<T>> for TropicalMatrix<T>
    where
        T: bytemuck::Pod + Into<f32> + From<f32>,
    {
        fn to_gpu_buffer(&self, context: &TropicalGpuContext) -> TropicalGpuResult<Buffer> {
            let header = GpuTropicalMatrixHeader {
                rows: self.rows as u32,
                cols: self.cols as u32,
                _padding: [0; 2],
            };

            let mut gpu_data = Vec::with_capacity(self.rows * self.cols);
            for row in &self.data {
                for &element in row {
                    gpu_data.push(GpuTropicalNumber {
                        value: element.value().into(),
                    });
                }
            }

            let mut buffer_data = Vec::new();
            buffer_data.extend_from_slice(bytemuck::cast_slice(&[header]));
            buffer_data.extend_from_slice(bytemuck::cast_slice(&gpu_data));

            let buffer = context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("TropicalMatrix Buffer"),
                    contents: &buffer_data,
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                });

            Ok(buffer)
        }

        fn from_gpu_buffer(
            buffer: &Buffer,
            context: &TropicalGpuContext,
        ) -> TropicalGpuResult<TropicalMatrix<T>> {
            let header_data: Vec<GpuTropicalMatrixHeader> = pollster::block_on(context.read_buffer(
                buffer,
                std::mem::size_of::<GpuTropicalMatrixHeader>() as u64,
            ))?;

            if header_data.is_empty() {
                return Err(TropicalGpuError::InvalidOperation(
                    "Empty header data".to_string(),
                ));
            }

            let header = header_data[0];
            let rows = header.rows as usize;
            let cols = header.cols as usize;
            let total_size = std::mem::size_of::<GpuTropicalMatrixHeader>()
                + rows * cols * std::mem::size_of::<GpuTropicalNumber>();

            let full_data: Vec<u8> =
                pollster::block_on(context.read_buffer(buffer, total_size as u64))?;

            let data_offset = std::mem::size_of::<GpuTropicalMatrixHeader>();
            let data_slice = &full_data[data_offset..];
            let gpu_numbers: &[GpuTropicalNumber] = bytemuck::cast_slice(data_slice);

            let mut matrix = TropicalMatrix::new(rows, cols);
            for i in 0..rows {
                for j in 0..cols {
                    let idx = i * cols + j;
                    if idx < gpu_numbers.len() {
                        matrix.data[i][j] =
                            TropicalNumber::new(<T as From<f32>>::from(gpu_numbers[idx].value));
                    }
                }
            }

            Ok(matrix)
        }

        fn gpu_operation(
            &self,
            operation: &str,
            _context: &TropicalGpuContext,
            params: &HashMap<String, GpuParameter>,
        ) -> TropicalGpuResult<TropicalMatrix<T>> {
            match operation {
                "tropical_matrix_multiply" => {
                    if !params.contains_key("other") {
                        return Err(TropicalGpuError::InvalidOperation(
                            "Missing 'other' matrix parameter".to_string(),
                        ));
                    }
                    Err(TropicalGpuError::InvalidOperation(
                        "Trait-based tropical_matrix_multiply is redesign-pending; use TropicalGpuOps::matrix_multiply instead".to_string(),
                    ))
                }
                "viterbi" => {
                    if !params.contains_key("emissions") {
                        return Err(TropicalGpuError::InvalidOperation(
                            "Missing 'emissions' parameter".to_string(),
                        ));
                    }
                    Err(TropicalGpuError::InvalidOperation(
                        "GPU Viterbi is redesign-pending".to_string(),
                    ))
                }
                "attention_scores" => Err(TropicalGpuError::InvalidOperation(
                    "GPU attention scores are redesign-pending".to_string(),
                )),
                _ => Err(TropicalGpuError::InvalidOperation(format!(
                    "Unknown operation: {}",
                    operation
                ))),
            }
        }
    }

    impl<T: Float, const P: usize, const Q: usize, const R: usize>
        TropicalGpuAccelerated<TropicalMultivector<T, P, Q, R>> for TropicalMultivector<T, P, Q, R>
    where
        T: bytemuck::Pod + Into<f32> + From<f32>,
    {
        fn to_gpu_buffer(&self, context: &TropicalGpuContext) -> TropicalGpuResult<Buffer> {
            let gpu_data: Vec<GpuTropicalNumber> = (0..self.dim())
                .filter_map(|i| self.get(i).ok())
                .map(|coeff| GpuTropicalNumber {
                    value: coeff.value().into(),
                })
                .collect();

            let buffer = context.create_buffer_with_data(
                "TropicalMultivector Buffer",
                &gpu_data,
                BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            );

            Ok(buffer)
        }

        fn from_gpu_buffer(
            buffer: &Buffer,
            context: &TropicalGpuContext,
        ) -> TropicalGpuResult<TropicalMultivector<T, P, Q, R>> {
            let basis_count = 1usize << (P + Q + R);
            let gpu_data: Vec<GpuTropicalNumber> = pollster::block_on(context.read_buffer(
                buffer,
                (basis_count * std::mem::size_of::<GpuTropicalNumber>()) as u64,
            ))?;

            let coefficients: Vec<T> = gpu_data
                .into_iter()
                .map(|gpu_num| <T as From<f32>>::from(gpu_num.value))
                .collect();

            TropicalMultivector::from_components(coefficients)
                .map_err(TropicalGpuError::TropicalError)
        }

        fn gpu_operation(
            &self,
            operation: &str,
            _context: &TropicalGpuContext,
            params: &HashMap<String, GpuParameter>,
        ) -> TropicalGpuResult<TropicalMultivector<T, P, Q, R>> {
            match operation {
                "geometric_product" => {
                    if !params.contains_key("other") {
                        return Err(TropicalGpuError::InvalidOperation(
                            "Missing 'other' parameter for geometric_product".to_string(),
                        ));
                    }
                    Err(TropicalGpuError::InvalidOperation(
                        "GPU tropical geometric product is redesign-pending".to_string(),
                    ))
                }
                "tropical_add" => {
                    if !params.contains_key("other") {
                        return Err(TropicalGpuError::InvalidOperation(
                            "Missing 'other' parameter for tropical_add".to_string(),
                        ));
                    }
                    Err(TropicalGpuError::InvalidOperation(
                        "GPU tropical_add is redesign-pending".to_string(),
                    ))
                }
                "tropical_scale" => {
                    if !params.contains_key("scalar") {
                        return Err(TropicalGpuError::InvalidOperation(
                            "Missing 'scalar' parameter for tropical_scale".to_string(),
                        ));
                    }
                    Err(TropicalGpuError::InvalidOperation(
                        "GPU tropical_scale is redesign-pending".to_string(),
                    ))
                }
                _ => Err(TropicalGpuError::InvalidOperation(format!(
                    "Unknown operation: {}",
                    operation
                ))),
            }
        }
    }
}


/// Execution path chosen for tropical matrix multiplication.
#[cfg(feature = "tropical")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TropicalExecutionPath {
    Cpu,
    Gpu,
}

/// High-level GPU tropical algebra operations
#[cfg(feature = "tropical")]
pub struct TropicalGpuOps {
    #[allow(dead_code)]
    context: TropicalGpuContext,
}

#[cfg(feature = "tropical")]
impl TropicalGpuOps {
    /// Create new GPU operations context
    pub async fn new() -> TropicalGpuResult<Self> {
        let context = TropicalGpuContext::new().await?;
        Ok(Self { context })
    }

    /// Conservative heuristic for whether dense tropical matrix multiply should
    /// use the GPU on the current restoration path.
    pub fn should_use_gpu_for_matrix_multiply(
        &self,
        rows_a: usize,
        cols_a: usize,
        cols_b: usize,
    ) -> bool {
        let work = rows_a.saturating_mul(cols_a).saturating_mul(cols_b);
        work >= 64 * 64 * 64
    }

    /// Determine the likely best execution path for dense tropical matrix multiply.
    pub fn matrix_multiply_execution_path(
        &self,
        rows_a: usize,
        cols_a: usize,
        cols_b: usize,
    ) -> TropicalExecutionPath {
        if self.should_use_gpu_for_matrix_multiply(rows_a, cols_a, cols_b) {
            TropicalExecutionPath::Gpu
        } else {
            TropicalExecutionPath::Cpu
        }
    }

    fn cpu_matrix_multiply<T>(
        &self,
        a: &TropicalMatrix<T>,
        b: &TropicalMatrix<T>,
    ) -> TropicalGpuResult<TropicalMatrix<T>>
    where
        T: Float + bytemuck::Pod + Into<f32> + From<f32>,
    {
        if a.cols != b.rows {
            return Err(TropicalGpuError::InvalidOperation(format!(
                "Tropical matrix multiply dimension mismatch: {}x{} cannot multiply {}x{}",
                a.rows, a.cols, b.rows, b.cols
            )));
        }

        let mut out = TropicalMatrix::new(a.rows, b.cols);
        for i in 0..a.rows {
            for j in 0..b.cols {
                let mut max_val = f32::NEG_INFINITY;
                for k in 0..a.cols {
                    let candidate = a.data[i][k].value().into() + b.data[k][j].value().into();
                    max_val = max_val.max(candidate);
                }
                out.data[i][j] = TropicalNumber::new(<T as From<f32>>::from(max_val));
            }
        }

        Ok(out)
    }

    /// Adaptive dense tropical matrix multiply.
    ///
    /// Uses a conservative local heuristic to choose CPU for smaller problems and
    /// GPU for larger ones, while preserving identical max-plus semantics.
    pub async fn matrix_multiply_adaptive<T>(
        &mut self,
        a: &TropicalMatrix<T>,
        b: &TropicalMatrix<T>,
    ) -> TropicalGpuResult<TropicalMatrix<T>>
    where
        T: Float + bytemuck::Pod + Into<f32> + From<f32>,
    {
        match self.matrix_multiply_execution_path(a.rows, a.cols, b.cols) {
            TropicalExecutionPath::Cpu => self.cpu_matrix_multiply(a, b),
            TropicalExecutionPath::Gpu => self.matrix_multiply(a, b).await,
        }
    }

    /// Real shader-backed dense tropical matrix multiplication.
    ///
    /// Computes `(A ⊗ B)[i, j] = max_k(A[i, k] + B[k, j])`.
    pub async fn matrix_multiply<T>(
        &mut self,
        a: &TropicalMatrix<T>,
        b: &TropicalMatrix<T>,
    ) -> TropicalGpuResult<TropicalMatrix<T>>
    where
        T: Float + bytemuck::Pod + Into<f32> + From<f32>,
    {
        if a.cols != b.rows {
            return Err(TropicalGpuError::InvalidOperation(format!(
                "Tropical matrix multiply dimension mismatch: {}x{} cannot multiply {}x{}",
                a.rows, a.cols, b.rows, b.cols
            )));
        }

        let a_data: Vec<GpuTropicalNumber> = a
            .data
            .iter()
            .flat_map(|row| row.iter())
            .map(|value| GpuTropicalNumber {
                value: value.value().into(),
            })
            .collect();

        let b_data: Vec<GpuTropicalNumber> = b
            .data
            .iter()
            .flat_map(|row| row.iter())
            .map(|value| GpuTropicalNumber {
                value: value.value().into(),
            })
            .collect();

        let result_len = a.rows * b.cols;
        let result_init = vec![GpuTropicalNumber { value: f32::NEG_INFINITY }; result_len];
        let params = GpuTropicalMatMulParams {
            rows_a: a.rows as u32,
            cols_a: a.cols as u32,
            cols_b: b.cols as u32,
            _padding: 0,
        };

        let shader = self
            .context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Tropical Matrix Multiply Shader"),
                source: wgpu::ShaderSource::Wgsl(TROPICAL_MATRIX_MULTIPLY_SHADER.into()),
            });

        let pipeline = self
            .context
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Tropical Matrix Multiply Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            });

        let buffer_a = self.context.create_buffer_with_data(
            "Tropical Matrix A",
            &a_data,
            BufferUsages::STORAGE,
        );
        let buffer_b = self.context.create_buffer_with_data(
            "Tropical Matrix B",
            &b_data,
            BufferUsages::STORAGE,
        );
        let result_buffer = self.context.create_buffer_with_data(
            "Tropical Matrix Multiply Result",
            &result_init,
            BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        );
        let params_buffer = self.context.create_buffer_with_data(
            "Tropical Matrix Multiply Params",
            &[params],
            BufferUsages::STORAGE,
        );

        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Tropical Matrix Multiply Bind Group"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer_a.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: buffer_b.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: result_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: params_buffer.as_entire_binding(),
                    },
                ],
            });

        let mut encoder = self
            .context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Tropical Matrix Multiply Encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Tropical Matrix Multiply Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups((a.rows as u32).div_ceil(16), (b.cols as u32).div_ceil(16), 1);
        }

        self.context.queue.submit([encoder.finish()]);
        self.context.device.poll(wgpu::Maintain::Wait);

        let result_data: Vec<GpuTropicalNumber> = self
            .context
            .read_buffer(
                &result_buffer,
                (result_len * std::mem::size_of::<GpuTropicalNumber>()) as u64,
            )
            .await?;

        let mut result = TropicalMatrix::new(a.rows, b.cols);
        for i in 0..a.rows {
            for j in 0..b.cols {
                let idx = i * b.cols + j;
                result.data[i][j] = TropicalNumber::new(<T as From<f32>>::from(result_data[idx].value));
            }
        }

        Ok(result)
    }

    /// Real shader-backed tropical winner-takes-all attention scores.
    ///
    /// For each row of `logits`, writes `1` at maximal entries and `0` elsewhere.
    /// This is a tropical/max-plus analogue of attention score normalization.
    pub async fn attention_scores<T>(
        &mut self,
        logits: &TropicalMatrix<T>,
    ) -> TropicalGpuResult<TropicalMatrix<T>>
    where
        T: Float + bytemuck::Pod + Into<f32> + From<f32>,
    {
        if logits.rows == 0 || logits.cols == 0 {
            return Ok(TropicalMatrix::new(logits.rows, logits.cols));
        }

        let logits_data: Vec<GpuTropicalNumber> = logits
            .data
            .iter()
            .flat_map(|row| row.iter())
            .map(|value| GpuTropicalNumber {
                value: value.value().into(),
            })
            .collect();

        let result_len = logits.rows * logits.cols;
        let result_init = vec![GpuTropicalNumber { value: 0.0 }; result_len];
        let params = GpuTropicalAttentionParams {
            rows: logits.rows as u32,
            cols: logits.cols as u32,
            _padding: [0; 2],
        };

        let shader = self
            .context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Tropical Attention Scores Shader"),
                source: wgpu::ShaderSource::Wgsl(TROPICAL_ATTENTION_SHADER.into()),
            });

        let pipeline = self
            .context
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Tropical Attention Scores Pipeline"),
                layout: None,
                module: &shader,
                entry_point: "main",
            });

        let logits_buffer = self.context.create_buffer_with_data(
            "Tropical Attention Logits",
            &logits_data,
            BufferUsages::STORAGE,
        );
        let result_buffer = self.context.create_buffer_with_data(
            "Tropical Attention Scores Result",
            &result_init,
            BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        );
        let params_buffer = self.context.create_buffer_with_data(
            "Tropical Attention Scores Params",
            &[params],
            BufferUsages::STORAGE,
        );

        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Tropical Attention Scores Bind Group"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: logits_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: result_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params_buffer.as_entire_binding(),
                    },
                ],
            });

        let mut encoder = self
            .context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Tropical Attention Scores Encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Tropical Attention Scores Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(
                (logits.rows as u32).div_ceil(16),
                (logits.cols as u32).div_ceil(16),
                1,
            );
        }

        self.context.queue.submit([encoder.finish()]);
        self.context.device.poll(wgpu::Maintain::Wait);

        let result_data: Vec<GpuTropicalNumber> = self
            .context
            .read_buffer(
                &result_buffer,
                (result_len * std::mem::size_of::<GpuTropicalNumber>()) as u64,
            )
            .await?;

        let mut result = TropicalMatrix::new(logits.rows, logits.cols);
        for i in 0..logits.rows {
            for j in 0..logits.cols {
                let idx = i * logits.cols + j;
                result.data[i][j] = TropicalNumber::new(<T as From<f32>>::from(result_data[idx].value));
            }
        }

        Ok(result)
    }

    /// Redesign-pending neural attention pipeline.
    pub(crate) async fn neural_attention<T>(
        &mut self,
        query: &TropicalMatrix<T>,
        _key: &TropicalMatrix<T>,
        _value: &TropicalMatrix<T>,
    ) -> TropicalGpuResult<TropicalMatrix<T>>
    where
        T: Float + bytemuck::Pod + Into<f32> + From<f32>,
    {
        // TODO: Implement full GPU attention mechanism
        // 1. QK^T tropical matrix multiply (max-plus operations)
        // 2. Apply tropical softmax (max operation)
        // 3. Multiply by V using tropical arithmetic

        // For now, return query as placeholder
        Ok(query.clone())
    }

    /// Redesign-pending batch Viterbi decoding.
    pub(crate) async fn batch_viterbi<T>(
        &mut self,
        transitions: &[TropicalMatrix<T>],
        _emissions: &[TropicalMatrix<T>],
        _initial_probs: &[Vec<T>],
        _sequence_lengths: &[usize],
    ) -> TropicalGpuResult<Vec<Vec<usize>>>
    where
        T: Float + bytemuck::Pod + Into<f32> + From<f32>,
    {
        // TODO: Implement batch GPU Viterbi decoding
        // This would process multiple sequences in parallel on GPU
        // For now, return empty results
        Ok(vec![vec![]; transitions.len()])
    }

    /// Redesign-pending tropical linear algebra solve.
    pub(crate) async fn tropical_solve<T>(
        &mut self,
        _a: &TropicalMatrix<T>,
        b: &TropicalMatrix<T>,
    ) -> TropicalGpuResult<TropicalMatrix<T>>
    where
        T: Float + bytemuck::Pod + Into<f32> + From<f32>,
    {
        // TODO: Implement tropical linear system solver on GPU
        // Uses tropical elimination and max-plus arithmetic
        // For now, return b as placeholder
        Ok(b.clone())
    }
}

/// WGSL shader source for tropical algebra operations
#[cfg(feature = "tropical")]
pub const TROPICAL_MATRIX_MULTIPLY_SHADER: &str = r#"
// Tropical matrix multiplication compute shader
// Tropical: (A ⊗ B)[i,j] = max_k(A[i,k] + B[k,j])

struct TropicalNumber {
    value: f32,
}

struct MatMulParams {
    rows_a: u32,
    cols_a: u32,
    cols_b: u32,
    padding: u32,
}

@group(0) @binding(0)
var<storage, read> matrix_a: array<TropicalNumber>;

@group(0) @binding(1)
var<storage, read> matrix_b: array<TropicalNumber>;

@group(0) @binding(2)
var<storage, read_write> result: array<TropicalNumber>;

@group(0) @binding(3)
var<storage, read> params: MatMulParams;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let i = global_id.x;
    let j = global_id.y;

    let rows_a = params.rows_a;
    let cols_a = params.cols_a;
    let cols_b = params.cols_b;

    if (i >= rows_a || j >= cols_b) {
        return;
    }

    var max_val = -3.402823466e+38;

    for (var k = 0u; k < cols_a; k = k + 1u) {
        let a_val = matrix_a[i * cols_a + k].value;
        let b_val = matrix_b[k * cols_b + j].value;
        let product = a_val + b_val;
        max_val = max(max_val, product);
    }

    result[i * cols_b + j].value = max_val;
}
"#;

/// WGSL shader for tropical attention computation
#[cfg(feature = "tropical")]
pub const TROPICAL_ATTENTION_SHADER: &str = r#"
// Tropical winner-takes-all attention score computation.
// For each row, entries equal to the row maximum receive 1.0; all others receive 0.0.

struct TropicalNumber {
    value: f32,
}

struct AttentionParams {
    rows: u32,
    cols: u32,
    padding: vec2<u32>,
}

@group(0) @binding(0)
var<storage, read> attention_logits: array<TropicalNumber>;

@group(0) @binding(1)
var<storage, read_write> attention_scores: array<TropicalNumber>;

@group(0) @binding(2)
var<storage, read> params: AttentionParams;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let row = global_id.x;
    let col = global_id.y;

    if (row >= params.rows || col >= params.cols) {
        return;
    }

    var max_val = -3.402823466e+38;
    for (var j = 0u; j < params.cols; j = j + 1u) {
        max_val = max(max_val, attention_logits[row * params.cols + j].value);
    }

    let current_val = attention_logits[row * params.cols + col].value;
    attention_scores[row * params.cols + col].value = select(0.0, 1.0, current_val == max_val);
}
"#;

#[cfg(test)]
#[cfg(feature = "tropical")]
mod tests {
    use super::*;
    use crate::tropical::redesign_pending::TropicalGpuAccelerated;
    use amari_tropical::TropicalNumber;
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn test_tropical_gpu_context_creation() {
        // Test should pass even without GPU (graceful fallback)
        let _result = TropicalGpuContext::new().await;
        // Don't assert success since GPU might not be available in CI
    }

    #[tokio::test]
    async fn test_tropical_number_gpu_buffer() {
        let tropical_num = TropicalNumber::new(3.5f32);

        // Test should not fail even if GPU is not available
        if let Ok(context) = TropicalGpuContext::new().await {
            let buffer = tropical_num.to_gpu_buffer(&context).unwrap();
            let reconstructed = TropicalNumber::<f32>::from_gpu_buffer(&buffer, &context).unwrap();

            assert_eq!(tropical_num.value(), reconstructed.value());
        }
    }

    #[tokio::test]
    async fn test_tropical_gpu_ops() {
        // Test initialization
        let result = TropicalGpuOps::new().await;
        // Should not panic even if GPU is not available
        if result.is_ok() {
            // GPU context created successfully
            println!("✅ TropicalGpuOps initialized successfully");
        }
    }

    fn cpu_tropical_matmul(a: &TropicalMatrix<f32>, b: &TropicalMatrix<f32>) -> TropicalMatrix<f32> {
        let mut out = TropicalMatrix::new(a.rows, b.cols);
        for i in 0..a.rows {
            for j in 0..b.cols {
                let mut max_val = f32::NEG_INFINITY;
                for k in 0..a.cols {
                    max_val = max_val.max(a.data[i][k].value() + b.data[k][j].value());
                }
                out.data[i][j] = TropicalNumber::new(max_val);
            }
        }
        out
    }

    fn make_benchmark_matrix(rows: usize, cols: usize, seed: u32) -> TropicalMatrix<f32> {
        let mut matrix = TropicalMatrix::new(rows, cols);
        let mut state = seed;
        for i in 0..rows {
            for j in 0..cols {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                let normalized = ((state >> 8) as f32) / ((u32::MAX >> 8) as f32);
                let value = normalized * 16.0 - 8.0;
                matrix.data[i][j] = TropicalNumber::new(value);
            }
        }
        matrix
    }

    async fn benchmark_gpu_matmul(
        ops: &mut TropicalGpuOps,
        a: &TropicalMatrix<f32>,
        b: &TropicalMatrix<f32>,
        warmups: usize,
        runs: usize,
    ) -> TropicalGpuResult<(TropicalMatrix<f32>, Duration)> {
        for _ in 0..warmups {
            let _ = ops.matrix_multiply(a, b).await?;
        }

        let mut total = Duration::ZERO;
        let mut last = None;
        for _ in 0..runs {
            let start = Instant::now();
            let result = ops.matrix_multiply(a, b).await?;
            total += start.elapsed();
            last = Some(result);
        }

        Ok((last.expect("runs > 0"), total / runs as u32))
    }

    fn benchmark_cpu_matmul(
        a: &TropicalMatrix<f32>,
        b: &TropicalMatrix<f32>,
        warmups: usize,
        runs: usize,
    ) -> (TropicalMatrix<f32>, Duration) {
        for _ in 0..warmups {
            let _ = cpu_tropical_matmul(a, b);
        }

        let mut total = Duration::ZERO;
        let mut last = None;
        for _ in 0..runs {
            let start = Instant::now();
            let result = cpu_tropical_matmul(a, b);
            total += start.elapsed();
            last = Some(result);
        }

        (last.expect("runs > 0"), total / runs as u32)
    }

    fn assert_matrix_close(a: &TropicalMatrix<f32>, b: &TropicalMatrix<f32>, tol: f32) {
        assert_eq!(a.rows, b.rows);
        assert_eq!(a.cols, b.cols);
        for i in 0..a.rows {
            for j in 0..a.cols {
                assert!(
                    (a.data[i][j].value() - b.data[i][j].value()).abs() < tol,
                    "Mismatch at ({}, {}): left={}, right={}",
                    i,
                    j,
                    a.data[i][j].value(),
                    b.data[i][j].value()
                );
            }
        }
    }

    fn cpu_attention_scores(logits: &TropicalMatrix<f32>) -> TropicalMatrix<f32> {
        let mut scores = TropicalMatrix::new(logits.rows, logits.cols);
        for i in 0..logits.rows {
            let mut row_max = f32::NEG_INFINITY;
            for j in 0..logits.cols {
                row_max = row_max.max(logits.data[i][j].value());
            }
            for j in 0..logits.cols {
                let score = if logits.data[i][j].value() == row_max { 1.0 } else { 0.0 };
                scores.data[i][j] = TropicalNumber::new(score);
            }
        }
        scores
    }

    #[tokio::test]
    async fn test_tropical_matrix_multiply_gpu_matches_cpu() {
        let mut ops = match TropicalGpuOps::new().await {
            Ok(ops) => ops,
            Err(_) => return,
        };

        let mut a = TropicalMatrix::new(2, 3);
        a.data[0][0] = TropicalNumber::new(1.0);
        a.data[0][1] = TropicalNumber::new(-2.0);
        a.data[0][2] = TropicalNumber::new(0.5);
        a.data[1][0] = TropicalNumber::new(-1.0);
        a.data[1][1] = TropicalNumber::new(3.0);
        a.data[1][2] = TropicalNumber::new(2.0);

        let mut b = TropicalMatrix::new(3, 2);
        b.data[0][0] = TropicalNumber::new(0.0);
        b.data[0][1] = TropicalNumber::new(2.0);
        b.data[1][0] = TropicalNumber::new(1.5);
        b.data[1][1] = TropicalNumber::new(-3.0);
        b.data[2][0] = TropicalNumber::new(4.0);
        b.data[2][1] = TropicalNumber::new(1.0);

        let cpu = cpu_tropical_matmul(&a, &b);
        let gpu = ops.matrix_multiply(&a, &b).await.unwrap();

        assert_eq!(gpu.rows, cpu.rows);
        assert_eq!(gpu.cols, cpu.cols);
        for i in 0..gpu.rows {
            for j in 0..gpu.cols {
                assert!(
                    (gpu.data[i][j].value() - cpu.data[i][j].value()).abs() < 1e-5,
                    "Mismatch at ({}, {}): gpu={}, cpu={}",
                    i,
                    j,
                    gpu.data[i][j].value(),
                    cpu.data[i][j].value()
                );
            }
        }
    }

    #[tokio::test]
    async fn test_tropical_matrix_multiply_dimension_mismatch() {
        let mut ops = match TropicalGpuOps::new().await {
            Ok(ops) => ops,
            Err(_) => return,
        };

        let a = TropicalMatrix::<f32>::new(2, 3);
        let b = TropicalMatrix::<f32>::new(4, 2);
        let err = ops.matrix_multiply(&a, &b).await.unwrap_err();
        assert!(matches!(err, TropicalGpuError::InvalidOperation(_)));
    }

    #[tokio::test]
    async fn test_tropical_attention_scores_gpu_matches_cpu() {
        let mut ops = match TropicalGpuOps::new().await {
            Ok(ops) => ops,
            Err(_) => return,
        };

        let mut logits = TropicalMatrix::new(3, 4);
        logits.data[0][0] = TropicalNumber::new(1.0);
        logits.data[0][1] = TropicalNumber::new(3.0);
        logits.data[0][2] = TropicalNumber::new(2.0);
        logits.data[0][3] = TropicalNumber::new(3.0);
        logits.data[1][0] = TropicalNumber::new(-1.0);
        logits.data[1][1] = TropicalNumber::new(-2.0);
        logits.data[1][2] = TropicalNumber::new(-3.0);
        logits.data[1][3] = TropicalNumber::new(-4.0);
        logits.data[2][0] = TropicalNumber::new(0.5);
        logits.data[2][1] = TropicalNumber::new(0.25);
        logits.data[2][2] = TropicalNumber::new(0.75);
        logits.data[2][3] = TropicalNumber::new(0.0);

        let cpu = cpu_attention_scores(&logits);
        let gpu = ops.attention_scores(&logits).await.unwrap();
        assert_matrix_close(&cpu, &gpu, 1e-6);
    }

    #[tokio::test]
    async fn test_tropical_matrix_multiply_execution_path_heuristic() {
        let ops = match TropicalGpuOps::new().await {
            Ok(ops) => ops,
            Err(_) => return,
        };

        assert_eq!(
            ops.matrix_multiply_execution_path(16, 16, 16),
            TropicalExecutionPath::Cpu
        );
        assert_eq!(
            ops.matrix_multiply_execution_path(64, 64, 64),
            TropicalExecutionPath::Gpu
        );
    }

    #[tokio::test]
    async fn test_tropical_matrix_multiply_adaptive_matches_cpu() {
        let mut ops = match TropicalGpuOps::new().await {
            Ok(ops) => ops,
            Err(_) => return,
        };

        let a = make_benchmark_matrix(32, 32, 0xCAFE_BABE);
        let b = make_benchmark_matrix(32, 32, 0xDEAD_BEEF);
        let cpu = cpu_tropical_matmul(&a, &b);
        let adaptive = ops.matrix_multiply_adaptive(&a, &b).await.unwrap();
        assert_matrix_close(&cpu, &adaptive, 1e-5);
    }

    #[tokio::test]
    #[ignore = "Manual benchmark harness for tropical GPU crossover work"]
    async fn benchmark_tropical_matrix_multiply_cpu_vs_gpu() {
        let mut ops = match TropicalGpuOps::new().await {
            Ok(ops) => ops,
            Err(err) => {
                eprintln!("Skipping tropical benchmark harness: {err}");
                return;
            }
        };

        let cases = [(16usize, 16usize, 16usize), (32, 32, 32), (64, 64, 64), (128, 128, 128)];
        let warmups = 1;
        let runs = 5;

        println!("\nTropical matrix multiply benchmark (CPU vs GPU)");
        println!("dims\tcpu_avg_ms\tgpu_avg_ms\tspeedup\tcorrect");

        for (m, k, n) in cases {
            let a = make_benchmark_matrix(m, k, 0x1234_5678 ^ m as u32);
            let b = make_benchmark_matrix(k, n, 0x9ABC_DEF0 ^ n as u32);

            let (cpu_result, cpu_avg) = benchmark_cpu_matmul(&a, &b, warmups, runs);
            let (gpu_result, gpu_avg) = match benchmark_gpu_matmul(&mut ops, &a, &b, warmups, runs).await {
                Ok(v) => v,
                Err(err) => {
                    println!("{}x{}x{}\t{:.3}\tERR\t-\tfalse ({})", m, k, n, cpu_avg.as_secs_f64() * 1000.0, err);
                    continue;
                }
            };

            assert_matrix_close(&cpu_result, &gpu_result, 1e-4);
            let cpu_ms = cpu_avg.as_secs_f64() * 1000.0;
            let gpu_ms = gpu_avg.as_secs_f64() * 1000.0;
            let speedup = if gpu_ms > 0.0 { cpu_ms / gpu_ms } else { f64::INFINITY };

            println!(
                "{}x{}x{}\t{:.3}\t{:.3}\t{:.2}x\ttrue",
                m, k, n, cpu_ms, gpu_ms, speedup
            );
        }
    }

    #[test]
    fn test_gpu_tropical_number_conversion() {
        let tropical_num = TropicalNumber::new(-2.5f32);
        let gpu_num: GpuTropicalNumber = tropical_num.into();
        let reconstructed: TropicalNumber<f32> = gpu_num.into();

        assert!((tropical_num.value() - reconstructed.value()).abs() < 1e-6);
    }
}
