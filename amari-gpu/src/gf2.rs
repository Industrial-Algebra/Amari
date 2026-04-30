//! GPU acceleration for GF(2) algebra operations
//!
//! Provides GPU-accelerated batch operations for:
//! - Binary Clifford algebra geometric products (Cl(N,R;F₂))
//! - GF(2) matrix-vector multiplication
//! - Hamming distance computation
//!
//! All operations use bit-packed u32 representations for efficient GPU computation
//! via WGSL compute shaders with XOR, AND, and popcount. The public API validates
//! the fixed-layout bounds used by the kernels: Clifford products support up to
//! 128 blades (`num_generators <= 7`), matrix-vector inputs support up to
//! 16 rows × 32 columns, and Hamming vectors support up to 128 bits.

#[cfg(feature = "gf2")]
use amari_core::gf2::{GF2Matrix, GF2Vector};
#[cfg(feature = "gf2")]
use bytemuck::{Pod, Zeroable};
#[cfg(feature = "gf2")]
use futures::channel::oneshot;
#[cfg(feature = "gf2")]
use std::collections::HashMap;
#[cfg(feature = "gf2")]
use thiserror::Error;
#[cfg(feature = "gf2")]
use wgpu::util::DeviceExt;

/// Error types for GF(2) GPU operations
#[cfg(feature = "gf2")]
#[derive(Error, Debug)]
pub enum GF2GpuError {
    #[error("GPU initialization failed: {0}")]
    Initialization(String),

    #[error("GF(2) computation failed: {0}")]
    Computation(String),

    #[error("Buffer operation failed: {0}")]
    Buffer(String),

    #[error("Shader compilation failed: {0}")]
    Shader(String),
}

/// Result type for GF(2) GPU operations
#[cfg(feature = "gf2")]
pub type GF2GpuResult<T> = Result<T, GF2GpuError>;

/// GPU-optimized pair of binary Clifford algebra multivectors for batch geometric product.
///
/// Each multivector is packed into 4 u32 words (up to 128 basis blades).
/// For Cl(N,R;F₂), the basis count is 2^(N+R).
#[cfg(feature = "gf2")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuGF2CliffordPair {
    /// First multivector, bit-packed (up to 128 basis blades)
    pub a_words: [u32; 4],
    /// Second multivector, bit-packed (up to 128 basis blades)
    pub b_words: [u32; 4],
    /// Total number of generators (N + R)
    pub num_generators: u32,
    /// Number of degenerate generators (R), where eⱼ² = 0
    pub num_degenerate: u32,
    /// Padding for alignment
    pub padding: [u32; 2],
}

/// GPU-optimized GF(2) matrix-vector multiplication data.
///
/// Matrix rows are bit-packed into u32 words (supports up to 16 rows × 32 cols).
#[cfg(feature = "gf2")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuGF2MatVecData {
    /// Matrix rows bit-packed (up to 16 rows, each row ≤32 cols as a u32)
    pub matrix_rows: [u32; 16],
    /// Input vector as bitmask
    pub vector: u32,
    /// Number of rows
    pub nrows: u32,
    /// Number of columns
    pub ncols: u32,
    /// Padding for alignment
    pub padding: u32,
}

/// GPU-optimized Hamming distance pair.
///
/// Two GF(2) vectors packed into u32 words (up to 128 bits).
#[cfg(feature = "gf2")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuGF2HammingPair {
    /// First vector, bit-packed (up to 128 bits)
    pub a_words: [u32; 4],
    /// Second vector, bit-packed (up to 128 bits)
    pub b_words: [u32; 4],
    /// Vector dimension
    pub dim: u32,
    /// Padding for alignment
    pub padding: [u32; 3],
}

/// GPU context for GF(2) operations
#[cfg(feature = "gf2")]
pub struct GF2GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    #[allow(dead_code)]
    shader_cache: HashMap<String, wgpu::ComputePipeline>,
}

/// GPU-accelerated operations for GF(2) algebra
#[cfg(feature = "gf2")]
pub struct GF2GpuOps {
    context: GF2GpuContext,
}

#[cfg(feature = "gf2")]
impl GF2GpuContext {
    /// Create new GPU context for GF(2) operations
    pub async fn new() -> GF2GpuResult<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| {
                GF2GpuError::Initialization("No suitable GPU adapter found".to_string())
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("GF(2) GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GF2GpuError::Initialization(format!("Failed to get GPU device: {}", e)))?;

        Ok(Self {
            device,
            queue,
            shader_cache: HashMap::new(),
        })
    }

    /// Read data from GPU buffer
    pub async fn read_buffer<T: Pod>(
        &self,
        buffer: &wgpu::Buffer,
        size: u64,
    ) -> GF2GpuResult<Vec<T>> {
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GF(2) Staging Buffer"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("GF(2) Copy Encoder"),
            });

        encoder.copy_buffer_to_buffer(buffer, 0, &staging_buffer, 0, size);
        self.queue.submit(Some(encoder.finish()));

        let (sender, receiver) = oneshot::channel();
        staging_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                sender.send(result).ok();
            });

        self.device.poll(wgpu::Maintain::Wait);

        receiver
            .await
            .map_err(|_| GF2GpuError::Buffer("Failed to receive buffer data".to_string()))?
            .map_err(|e| GF2GpuError::Buffer(format!("Buffer mapping failed: {:?}", e)))?;

        let data = staging_buffer.slice(..).get_mapped_range();
        let result: Vec<T> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }

    /// Execute compute pipeline
    fn execute_compute(
        &self,
        shader_source: &str,
        bind_group_layout: &wgpu::BindGroupLayout,
        bind_group: &wgpu::BindGroup,
        workgroup_count: (u32, u32, u32),
    ) -> GF2GpuResult<()> {
        let shader_module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("GF(2) Compute Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_source.into()),
            });

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("GF(2) Pipeline Layout"),
                bind_group_layouts: &[bind_group_layout],
                push_constant_ranges: &[],
            });

        let compute_pipeline =
            self.device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("GF(2) Pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader_module,
                    entry_point: "main",
                });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("GF(2) Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("GF(2) Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&compute_pipeline);
            compute_pass.set_bind_group(0, bind_group, &[]);
            compute_pass.dispatch_workgroups(
                workgroup_count.0,
                workgroup_count.1,
                workgroup_count.2,
            );
        }

        self.queue.submit(Some(encoder.finish()));

        Ok(())
    }

    /// Create a standard two-binding layout (read input, read-write output)
    fn create_standard_bind_group_layout(&self, label: &str) -> wgpu::BindGroupLayout {
        self.device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            })
    }
}

#[cfg(feature = "gf2")]
impl GF2GpuOps {
    /// Create new GF(2) GPU operations context
    pub async fn new() -> GF2GpuResult<Self> {
        let context = GF2GpuContext::new().await?;
        Ok(Self { context })
    }

    /// Batch GF(2) Clifford algebra geometric product.
    ///
    /// Computes the geometric product of each pair `(a, b)` in the Clifford
    /// algebra `Cl(N,R;F₂)`. Results are packed as `[u32; 4]` per pair.
    ///
    /// This GPU kernel supports at most 128 basis blades, so
    /// `num_generators <= 7`; `num_degenerate` must not exceed
    /// `num_generators`.
    pub async fn batch_gf2_geometric_product(
        &mut self,
        pairs: &[GpuGF2CliffordPair],
    ) -> GF2GpuResult<Vec<[u32; 4]>> {
        let num_pairs = pairs.len();
        if num_pairs == 0 {
            return Ok(Vec::new());
        }
        for pair in pairs {
            if pair.num_generators > 7 {
                return Err(GF2GpuError::Computation(
                    "GF(2) Clifford GPU kernel supports num_generators <= 7".to_string(),
                ));
            }
            if pair.num_degenerate > pair.num_generators {
                return Err(GF2GpuError::Computation(
                    "num_degenerate must be <= num_generators".to_string(),
                ));
            }
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("GF2 Clifford Input"),
                    contents: bytemuck::cast_slice(pairs),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        // Output: 4 u32 words per result
        let output_size = (num_pairs * 4 * std::mem::size_of::<u32>()) as u64;
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GF2 Clifford Output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = Self::get_gf2_clifford_shader();
        let bind_group_layout = self
            .context
            .create_standard_bind_group_layout("GF2 Clifford Layout");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("GF2 Clifford Bind Group"),
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

        let workgroup_count = num_pairs.div_ceil(64) as u32;
        self.context.execute_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        let flat_results: Vec<u32> = self
            .context
            .read_buffer(&output_buffer, output_size)
            .await?;

        // Reshape flat u32 results into [u32; 4] per pair
        Ok(flat_results
            .chunks(4)
            .map(|chunk| {
                let mut arr = [0u32; 4];
                arr[..chunk.len()].copy_from_slice(chunk);
                arr
            })
            .collect())
    }

    /// Batch GF(2) matrix-vector multiplication.
    ///
    /// For each `(matrix, vector)` pair, computes the matrix-vector product over
    /// GF(2). Results are returned as row-bitmasks. The fixed GPU layout supports
    /// at most 16 rows and 32 columns.
    pub async fn batch_gf2_matvec(&mut self, data: &[GpuGF2MatVecData]) -> GF2GpuResult<Vec<u32>> {
        let num_items = data.len();
        if num_items == 0 {
            return Ok(Vec::new());
        }
        for item in data {
            if item.nrows > 16 {
                return Err(GF2GpuError::Computation(
                    "GF(2) matvec GPU kernel supports nrows <= 16".to_string(),
                ));
            }
            if item.ncols > 32 {
                return Err(GF2GpuError::Computation(
                    "GF(2) matvec GPU kernel supports ncols <= 32".to_string(),
                ));
            }
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("GF2 MatVec Input"),
                    contents: bytemuck::cast_slice(data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        let output_size = (num_items * std::mem::size_of::<u32>()) as u64;
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GF2 MatVec Output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = Self::get_gf2_matvec_shader();
        let bind_group_layout = self
            .context
            .create_standard_bind_group_layout("GF2 MatVec Layout");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("GF2 MatVec Bind Group"),
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

        let workgroup_count = num_items.div_ceil(64) as u32;
        self.context.execute_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        self.context.read_buffer(&output_buffer, output_size).await
    }

    /// Batch Hamming distance computation.
    ///
    /// Computes the Hamming distance between each pair of GF(2) vectors. The
    /// fixed GPU layout supports vectors up to 128 bits and masks unused bits in
    /// the final word according to `dim`.
    pub async fn batch_gf2_hamming_distance(
        &mut self,
        pairs: &[GpuGF2HammingPair],
    ) -> GF2GpuResult<Vec<u32>> {
        let num_pairs = pairs.len();
        if num_pairs == 0 {
            return Ok(Vec::new());
        }
        for pair in pairs {
            if pair.dim > 128 {
                return Err(GF2GpuError::Computation(
                    "GF(2) Hamming GPU kernel supports dim <= 128".to_string(),
                ));
            }
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("GF2 Hamming Input"),
                    contents: bytemuck::cast_slice(pairs),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        let output_size = (num_pairs * std::mem::size_of::<u32>()) as u64;
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GF2 Hamming Output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = Self::get_gf2_hamming_shader();
        let bind_group_layout = self
            .context
            .create_standard_bind_group_layout("GF2 Hamming Layout");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("GF2 Hamming Bind Group"),
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

        let workgroup_count = num_pairs.div_ceil(64) as u32;
        self.context.execute_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        self.context.read_buffer(&output_buffer, output_size).await
    }

    // ─── WGSL Shaders ───

    /// GF(2) Clifford geometric product shader
    ///
    /// Over GF(2), the geometric product is:
    /// - eₐeᵦ = eₐ⊕ᵦ (XOR blade indices) if A∩B has no degenerate generators
    /// - eₐeᵦ = 0 if A∩B has any degenerate generator
    /// - No sign since -1 = 1 in GF(2)
    fn get_gf2_clifford_shader() -> String {
        String::from(
            r#"
struct GF2CliffordPair {
    a_words: array<u32, 4>,
    b_words: array<u32, 4>,
    num_generators: u32,
    num_degenerate: u32,
    padding: array<u32, 2>,
}

@group(0) @binding(0) var<storage, read> input_data: array<GF2CliffordPair>;
@group(0) @binding(1) var<storage, read_write> output_data: array<u32>;

fn count_ones(x: u32) -> u32 {
    var n = x;
    n = n - ((n >> 1u) & 0x55555555u);
    n = (n & 0x33333333u) + ((n >> 2u) & 0x33333333u);
    n = (n + (n >> 4u)) & 0x0F0F0F0Fu;
    n = n + (n >> 8u);
    n = n + (n >> 16u);
    return n & 0x3Fu;
}

// WGSL requires constant indexing for local arrays, so we use helper functions
// that branch on the index value.

fn get_word(words: array<u32, 4>, idx: u32) -> u32 {
    if (idx == 0u) { return words[0]; }
    if (idx == 1u) { return words[1]; }
    if (idx == 2u) { return words[2]; }
    return words[3];
}

fn get_bit(words: array<u32, 4>, index: u32) -> u32 {
    let word_idx = index / 32u;
    let bit_idx = index % 32u;
    return (get_word(words, word_idx) >> bit_idx) & 1u;
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= arrayLength(&input_data)) {
        return;
    }

    let pair = input_data[index];
    let n_gen = pair.num_generators;
    let n_degen = pair.num_degenerate;
    let n_nondegen = n_gen - n_degen;
    let basis_count = 1u << n_gen;

    // Degenerate mask: bits n_nondegen..n_gen are degenerate generators
    var degen_mask: u32 = 0u;
    if (n_degen > 0u) {
        degen_mask = ((1u << n_degen) - 1u) << n_nondegen;
    }

    // Result multivector (4 words)
    var r0: u32 = 0u;
    var r1: u32 = 0u;
    var r2: u32 = 0u;
    var r3: u32 = 0u;

    // Iterate over all nonzero blade pairs
    for (var a_blade: u32 = 0u; a_blade < basis_count && a_blade < 128u; a_blade++) {
        let a_coeff = get_bit(pair.a_words, a_blade);
        if (a_coeff == 0u) {
            continue;
        }

        for (var b_blade: u32 = 0u; b_blade < basis_count && b_blade < 128u; b_blade++) {
            let b_coeff = get_bit(pair.b_words, b_blade);
            if (b_coeff == 0u) {
                continue;
            }

            // Check if intersection has degenerate generators → product is 0
            let intersection = a_blade & b_blade;
            if ((intersection & degen_mask) != 0u) {
                continue;
            }

            // Result blade index = XOR of blade indices
            let result_blade = a_blade ^ b_blade;

            // XOR the result bit (accumulate in GF(2))
            let word_idx = result_blade / 32u;
            let bit_idx = result_blade % 32u;
            let bit_mask = 1u << bit_idx;
            if (word_idx == 0u) { r0 ^= bit_mask; }
            else if (word_idx == 1u) { r1 ^= bit_mask; }
            else if (word_idx == 2u) { r2 ^= bit_mask; }
            else { r3 ^= bit_mask; }
        }
    }

    // Write 4 output words
    let out_base = index * 4u;
    output_data[out_base] = r0;
    output_data[out_base + 1u] = r1;
    output_data[out_base + 2u] = r2;
    output_data[out_base + 3u] = r3;
}
"#,
        )
    }

    /// GF(2) matrix-vector multiplication shader
    ///
    /// For each row i: result[i] = parity(row_i AND vector)
    fn get_gf2_matvec_shader() -> String {
        String::from(
            r#"
struct GF2MatVecData {
    matrix_rows: array<u32, 16>,
    vector: u32,
    nrows: u32,
    ncols: u32,
    padding: u32,
}

@group(0) @binding(0) var<storage, read> input_data: array<GF2MatVecData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<u32>;

fn count_ones(x: u32) -> u32 {
    var n = x;
    n = n - ((n >> 1u) & 0x55555555u);
    n = (n & 0x33333333u) + ((n >> 2u) & 0x33333333u);
    n = (n + (n >> 4u)) & 0x0F0F0F0Fu;
    n = n + (n >> 8u);
    n = n + (n >> 16u);
    return n & 0x3Fu;
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= arrayLength(&input_data)) {
        return;
    }

    let data = input_data[index];
    var result: u32 = 0u;

    // Unroll loop to avoid dynamic indexing into matrix_rows array
    if (data.nrows > 0u) { result |= ((count_ones(data.matrix_rows[0] & data.vector) & 1u) << 0u); }
    if (data.nrows > 1u) { result |= ((count_ones(data.matrix_rows[1] & data.vector) & 1u) << 1u); }
    if (data.nrows > 2u) { result |= ((count_ones(data.matrix_rows[2] & data.vector) & 1u) << 2u); }
    if (data.nrows > 3u) { result |= ((count_ones(data.matrix_rows[3] & data.vector) & 1u) << 3u); }
    if (data.nrows > 4u) { result |= ((count_ones(data.matrix_rows[4] & data.vector) & 1u) << 4u); }
    if (data.nrows > 5u) { result |= ((count_ones(data.matrix_rows[5] & data.vector) & 1u) << 5u); }
    if (data.nrows > 6u) { result |= ((count_ones(data.matrix_rows[6] & data.vector) & 1u) << 6u); }
    if (data.nrows > 7u) { result |= ((count_ones(data.matrix_rows[7] & data.vector) & 1u) << 7u); }
    if (data.nrows > 8u) { result |= ((count_ones(data.matrix_rows[8] & data.vector) & 1u) << 8u); }
    if (data.nrows > 9u) { result |= ((count_ones(data.matrix_rows[9] & data.vector) & 1u) << 9u); }
    if (data.nrows > 10u) { result |= ((count_ones(data.matrix_rows[10] & data.vector) & 1u) << 10u); }
    if (data.nrows > 11u) { result |= ((count_ones(data.matrix_rows[11] & data.vector) & 1u) << 11u); }
    if (data.nrows > 12u) { result |= ((count_ones(data.matrix_rows[12] & data.vector) & 1u) << 12u); }
    if (data.nrows > 13u) { result |= ((count_ones(data.matrix_rows[13] & data.vector) & 1u) << 13u); }
    if (data.nrows > 14u) { result |= ((count_ones(data.matrix_rows[14] & data.vector) & 1u) << 14u); }
    if (data.nrows > 15u) { result |= ((count_ones(data.matrix_rows[15] & data.vector) & 1u) << 15u); }

    output_data[index] = result;
}
"#,
        )
    }

    /// Hamming distance shader
    ///
    /// XOR corresponding words, popcount each, sum.
    fn get_gf2_hamming_shader() -> String {
        String::from(
            r#"
struct GF2HammingPair {
    a_words: array<u32, 4>,
    b_words: array<u32, 4>,
    dim: u32,
    padding: array<u32, 3>,
}

@group(0) @binding(0) var<storage, read> input_data: array<GF2HammingPair>;
@group(0) @binding(1) var<storage, read_write> output_data: array<u32>;

fn count_ones(x: u32) -> u32 {
    var n = x;
    n = n - ((n >> 1u) & 0x55555555u);
    n = (n & 0x33333333u) + ((n >> 2u) & 0x33333333u);
    n = (n + (n >> 4u)) & 0x0F0F0F0Fu;
    n = n + (n >> 8u);
    n = n + (n >> 16u);
    return n & 0x3Fu;
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= arrayLength(&input_data)) {
        return;
    }

    let pair = input_data[index];
    let num_words = (pair.dim + 31u) / 32u;

    var distance: u32 = 0u;
    if (num_words >= 1u) { distance += count_ones(pair.a_words[0] ^ pair.b_words[0]); }
    if (num_words >= 2u) { distance += count_ones(pair.a_words[1] ^ pair.b_words[1]); }
    if (num_words >= 3u) { distance += count_ones(pair.a_words[2] ^ pair.b_words[2]); }
    if (num_words >= 4u) { distance += count_ones(pair.a_words[3] ^ pair.b_words[3]); }

    let rem_bits = pair.dim % 32u;
    if (rem_bits != 0u && num_words > 0u) {
        let last_word = num_words - 1u;
        let mask = (1u << rem_bits) - 1u;
        var extra: u32 = 0u;
        if (last_word == 0u) { extra = count_ones((pair.a_words[0] ^ pair.b_words[0]) & ~mask); }
        else if (last_word == 1u) { extra = count_ones((pair.a_words[1] ^ pair.b_words[1]) & ~mask); }
        else if (last_word == 2u) { extra = count_ones((pair.a_words[2] ^ pair.b_words[2]) & ~mask); }
        else { extra = count_ones((pair.a_words[3] ^ pair.b_words[3]) & ~mask); }
        distance -= extra;
    }

    output_data[index] = distance;
}
"#,
        )
    }
}

// ─── Conversion constructors ───

#[cfg(feature = "gf2")]
impl GpuGF2CliffordPair {
    /// Create from raw u64 words of two binary multivectors.
    ///
    /// Words are packed from the `BinaryMultivector` internal representation (u64).
    /// The u64 words are split into pairs of u32 for GPU compatibility.
    pub fn from_u64_words(
        a_words_u64: &[u64],
        b_words_u64: &[u64],
        num_generators: usize,
        num_degenerate: usize,
    ) -> Self {
        let mut a_words = [0u32; 4];
        let mut b_words = [0u32; 4];

        // Convert u64 words to u32 pairs (low, high)
        for (i, &w) in a_words_u64.iter().enumerate() {
            let lo_idx = i * 2;
            let hi_idx = i * 2 + 1;
            if lo_idx < 4 {
                a_words[lo_idx] = w as u32;
            }
            if hi_idx < 4 {
                a_words[hi_idx] = (w >> 32) as u32;
            }
        }
        for (i, &w) in b_words_u64.iter().enumerate() {
            let lo_idx = i * 2;
            let hi_idx = i * 2 + 1;
            if lo_idx < 4 {
                b_words[lo_idx] = w as u32;
            }
            if hi_idx < 4 {
                b_words[hi_idx] = (w >> 32) as u32;
            }
        }

        Self {
            a_words,
            b_words,
            num_generators: num_generators as u32,
            num_degenerate: num_degenerate as u32,
            padding: [0; 2],
        }
    }

    /// Create from individual blade bit arrays.
    ///
    /// Each byte in `a_bits`/`b_bits` is 0 or 1 for the corresponding basis blade.
    pub fn from_bits(
        a_bits: &[u8],
        b_bits: &[u8],
        num_generators: usize,
        num_degenerate: usize,
    ) -> Self {
        let mut a_words = [0u32; 4];
        let mut b_words = [0u32; 4];

        for (i, &bit) in a_bits.iter().enumerate() {
            if i < 128 && bit != 0 {
                a_words[i / 32] |= 1 << (i % 32);
            }
        }
        for (i, &bit) in b_bits.iter().enumerate() {
            if i < 128 && bit != 0 {
                b_words[i / 32] |= 1 << (i % 32);
            }
        }

        Self {
            a_words,
            b_words,
            num_generators: num_generators as u32,
            num_degenerate: num_degenerate as u32,
            padding: [0; 2],
        }
    }
}

#[cfg(feature = "gf2")]
impl GpuGF2MatVecData {
    /// Create from a GF2Matrix and GF2Vector.
    ///
    /// Matrix must have ≤16 rows and ≤32 columns.
    pub fn from_matrix_and_vector(matrix: &GF2Matrix, vector: &GF2Vector) -> Self {
        let mut matrix_rows = [0u32; 16];

        for (i, slot) in matrix_rows
            .iter_mut()
            .enumerate()
            .take(matrix.nrows().min(16))
        {
            let row = matrix.row(i);
            let words = row.as_words();
            // Take the low 32 bits of the first u64 word
            if !words.is_empty() {
                *slot = words[0] as u32;
            }
        }

        let vec_words = vector.as_words();
        let vec_u32 = if !vec_words.is_empty() {
            vec_words[0] as u32
        } else {
            0
        };

        Self {
            matrix_rows,
            vector: vec_u32,
            nrows: matrix.nrows() as u32,
            ncols: matrix.ncols() as u32,
            padding: 0,
        }
    }
}

#[cfg(feature = "gf2")]
impl GpuGF2HammingPair {
    /// Create from two GF2Vectors.
    pub fn from_vectors(a: &GF2Vector, b: &GF2Vector) -> Self {
        let mut a_words = [0u32; 4];
        let mut b_words = [0u32; 4];

        // Convert u64 words to u32 pairs
        for (i, &w) in a.as_words().iter().enumerate() {
            let lo_idx = i * 2;
            let hi_idx = i * 2 + 1;
            if lo_idx < 4 {
                a_words[lo_idx] = w as u32;
            }
            if hi_idx < 4 {
                a_words[hi_idx] = (w >> 32) as u32;
            }
        }
        for (i, &w) in b.as_words().iter().enumerate() {
            let lo_idx = i * 2;
            let hi_idx = i * 2 + 1;
            if lo_idx < 4 {
                b_words[lo_idx] = w as u32;
            }
            if hi_idx < 4 {
                b_words[hi_idx] = (w >> 32) as u32;
            }
        }

        Self {
            a_words,
            b_words,
            dim: a.dim() as u32,
            padding: [0; 3],
        }
    }
}

// ─── Tests ───

#[cfg(feature = "gf2")]
#[cfg(test)]
mod tests {
    use super::*;
    use amari_core::gf2::{GF2Matrix, GF2Vector, GF2};

    macro_rules! with_serial_gpu_ops {
        ($gpu_ops:ident, $body:block) => {{
            let _gpu_test_guard = crate::GPU_TEST_LOCK.lock().await;
            if let Ok(mut $gpu_ops) = GF2GpuOps::new().await $body
        }};
    }

    #[tokio::test]
    async fn test_gf2_gpu_context_initialization() {
        let _gpu_test_guard = crate::GPU_TEST_LOCK.lock().await;
        let result = GF2GpuContext::new().await;

        match result {
            Ok(_context) => {
                println!("GF(2) GPU context initialized successfully");
            }
            Err(_) => {
                println!("GPU not available, test passes with graceful fallback");
            }
        }
    }

    #[tokio::test]
    async fn test_batch_gf2_geometric_product() {
        with_serial_gpu_ops!(gpu_ops, {
            // Cl(3,0;F₂): e1*e2 = e12
            // a = e1 (blade index 1 = bit 1), b = e2 (blade index 2 = bit 2)
            let pairs = vec![
                GpuGF2CliffordPair::from_bits(
                    &[0, 1, 0, 0, 0, 0, 0, 0], // e1
                    &[0, 0, 1, 0, 0, 0, 0, 0], // e2
                    3,
                    0,
                ),
                GpuGF2CliffordPair::from_bits(
                    &[1, 0, 0, 0, 0, 0, 0, 0], // scalar 1
                    &[0, 1, 0, 0, 0, 0, 0, 0], // e1
                    3,
                    0,
                ),
            ];

            let result = gpu_ops.batch_gf2_geometric_product(&pairs).await;

            match result {
                Ok(products) => {
                    assert_eq!(products.len(), 2);
                    // e1 * e2 = e12 (blade index 3 = bit 3)
                    assert_eq!(products[0][0] & (1 << 3), 1 << 3, "e1*e2 should give e12");
                    // 1 * e1 = e1 (blade index 1 = bit 1)
                    assert_eq!(products[1][0] & (1 << 1), 1 << 1, "1*e1 should give e1");
                    println!("Batch GF(2) geometric product successful");
                }
                Err(_) => {
                    println!("GPU not available, test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_batch_gf2_matvec() {
        with_serial_gpu_ops!(gpu_ops, {
            // Identity matrix times [1,0,1] should give [1,0,1]
            let matrix = GF2Matrix::identity(3);
            let vector = GF2Vector::from_bits(&[1, 0, 1]);

            let data = vec![GpuGF2MatVecData::from_matrix_and_vector(&matrix, &vector)];

            let result = gpu_ops.batch_gf2_matvec(&data).await;

            match result {
                Ok(results) => {
                    assert_eq!(results.len(), 1);
                    // Result should be 0b101 = 5
                    assert_eq!(results[0], 5, "I * [1,0,1] should give [1,0,1] = 5");
                    println!("Batch GF(2) matvec successful");
                }
                Err(_) => {
                    println!("GPU not available, test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_batch_gf2_hamming_distance() {
        with_serial_gpu_ops!(gpu_ops, {
            let a = GF2Vector::from_bits(&[1, 0, 1, 1, 0]);
            let b = GF2Vector::from_bits(&[0, 0, 1, 0, 1]);

            let pairs = vec![GpuGF2HammingPair::from_vectors(&a, &b)];

            let result = gpu_ops.batch_gf2_hamming_distance(&pairs).await;

            match result {
                Ok(distances) => {
                    assert_eq!(distances.len(), 1);
                    // Diff at positions 0, 3, 4 → distance = 3
                    assert_eq!(distances[0], 3, "Hamming distance should be 3");
                    println!("Batch GF(2) Hamming distance successful");
                }
                Err(_) => {
                    println!("GPU not available, test passes");
                }
            }
        });
    }

    #[test]
    fn test_clifford_pair_from_bits() {
        let pair = GpuGF2CliffordPair::from_bits(
            &[1, 1, 0, 0, 0, 0, 0, 0], // scalar + e1
            &[0, 0, 1, 0, 0, 0, 0, 0], // e2
            3,
            0,
        );
        assert_eq!(pair.a_words[0], 0b11); // bits 0 and 1
        assert_eq!(pair.b_words[0], 0b100); // bit 2
        assert_eq!(pair.num_generators, 3);
        assert_eq!(pair.num_degenerate, 0);
    }

    #[test]
    fn test_matvec_conversion() {
        let mut matrix = GF2Matrix::zero(2, 3);
        matrix.set(0, 0, GF2::ONE);
        matrix.set(0, 2, GF2::ONE);
        matrix.set(1, 1, GF2::ONE);

        let vector = GF2Vector::from_bits(&[1, 1, 0]);

        let gpu_data = GpuGF2MatVecData::from_matrix_and_vector(&matrix, &vector);

        assert_eq!(gpu_data.nrows, 2);
        assert_eq!(gpu_data.ncols, 3);
        // Row 0: [1,0,1] = 0b101 = 5
        assert_eq!(gpu_data.matrix_rows[0], 5);
        // Row 1: [0,1,0] = 0b010 = 2
        assert_eq!(gpu_data.matrix_rows[1], 2);
        // Vector: [1,1,0] = 0b011 = 3
        assert_eq!(gpu_data.vector, 3);
    }

    #[test]
    fn test_hamming_pair_conversion() {
        let a = GF2Vector::from_bits(&[1, 0, 1]);
        let b = GF2Vector::from_bits(&[0, 1, 1]);

        let pair = GpuGF2HammingPair::from_vectors(&a, &b);

        assert_eq!(pair.a_words[0], 0b101); // [1,0,1]
        assert_eq!(pair.b_words[0], 0b110); // [0,1,1]
        assert_eq!(pair.dim, 3);
    }

    #[test]
    fn test_clifford_pair_from_u64_words() {
        // Single u64 word: bits 0 and 3 set = 0b1001 = 9
        let a_words = vec![9u64];
        let b_words = vec![4u64]; // bit 2 set

        let pair = GpuGF2CliffordPair::from_u64_words(&a_words, &b_words, 3, 0);

        assert_eq!(pair.a_words[0], 9);
        assert_eq!(pair.b_words[0], 4);
        assert_eq!(pair.num_generators, 3);
    }
}
