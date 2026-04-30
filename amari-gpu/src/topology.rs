//! GPU-backed and GPU-ready computational topology operations.
//!
//! Current public behavior is intentionally explicit:
//! - `GpuTopology::compute_distance_matrix` is GPU-backed pairwise Euclidean distance computation.
//! - `GpuTopology::find_critical_points_2d` is GPU-backed discrete Morse critical point detection.
//! - `GpuTopology::build_rips_filtration` currently builds the Vietoris-Rips filtration on CPU from a supplied distance matrix.
//! - `GpuTopology::compute_betti_numbers` currently uses the `amari-topology` CPU homology baseline.
//! - `AdaptiveTopologyCompute` chooses GPU for distance/Morse paths above thresholds and CPU fallback otherwise.
//!
//! # Example
//!
//! ```ignore
//! use amari_gpu::topology::{GpuTopology, AdaptiveTopologyCompute};
//!
//! // Create GPU topology processor
//! let gpu = GpuTopology::new().await?;
//!
//! // Compute distance matrix for Rips filtration
//! let distances = gpu.compute_distance_matrix(&points).await?;
//!
//! // Find Morse critical points on a grid
//! let critical_points = gpu.find_critical_points_2d(&height_values, width, height).await?;
//!
//! // Or use adaptive dispatch
//! let adaptive = AdaptiveTopologyCompute::new().await;
//! let distances = adaptive.compute_distance_matrix(&points).await?;
//! ```

use crate::GpuError;
use amari_topology::{CriticalType, SimplicialComplex};
use bytemuck::{Pod, Zeroable};
use futures::channel::oneshot;
use std::cmp::Ordering;
use thiserror::Error;
use wgpu::util::DeviceExt;

/// GPU-friendly critical point representation
#[derive(Clone, Debug)]
pub struct GpuCriticalPoint {
    /// Grid position (x, y)
    pub position: (usize, usize),
    /// Function value at the critical point
    pub value: f64,
    /// Type of critical point
    pub critical_type: CriticalType,
    /// Morse index
    pub index: usize,
}

/// Errors specific to GPU topology operations
#[derive(Error, Debug)]
pub enum GpuTopologyError {
    #[error("GPU error: {0}")]
    Gpu(#[from] GpuError),

    #[error("Invalid input size: {0}")]
    InvalidSize(String),

    #[error("Buffer error: {0}")]
    BufferError(String),

    #[error("Shader compilation error: {0}")]
    ShaderError(String),

    #[error("Topology error: {0}")]
    TopologyError(String),
}

pub type GpuTopologyResult<T> = Result<T, GpuTopologyError>;

/// GPU-backed and GPU-ready topology operations.
///
/// Distance matrices and 2D Morse critical point detection are GPU-backed.
/// Rips filtration construction and Betti numbers currently preserve CPU
/// baseline semantics while GPU persistent-homology kernels are pending.
pub struct GpuTopology {
    device: wgpu::Device,
    queue: wgpu::Queue,
    distance_pipeline: wgpu::ComputePipeline,
    morse_pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)] // Reserved for persistent homology GPU acceleration
    boundary_pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)] // Reserved for persistent homology GPU acceleration
    reduction_pipeline: wgpu::ComputePipeline,
}

/// Point data for GPU distance computation
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GpuPoint {
    x: f32,
    y: f32,
    z: f32,
    w: f32, // 4th dimension or padding
}

/// Simplex data for GPU boundary computation
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GpuSimplex {
    vertices: [u32; 8], // Max 7-simplex (8 vertices)
    dimension: u32,
    filtration_time: f32,
    _padding: [u32; 2],
}

/// Critical point data for GPU buffer (internal repr)
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GpuCriticalPointBuffer {
    x: u32,
    y: u32,
    critical_type: u32, // 0=minimum, 1=saddle, 2=maximum
    value: f32,
}

/// Sparse matrix entry for boundary computation
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GpuMatrixEntry {
    row: u32,
    col: u32,
    value: i32,
    _padding: u32,
}

impl GpuTopology {
    /// Initialize GPU context for topology operations
    pub async fn new() -> GpuTopologyResult<Self> {
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
                    label: Some("Amari Topology GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::InitializationError(e.to_string()))?;

        let distance_pipeline = Self::create_distance_pipeline(&device)?;
        let morse_pipeline = Self::create_morse_pipeline(&device)?;
        let boundary_pipeline = Self::create_boundary_pipeline(&device)?;
        let reduction_pipeline = Self::create_reduction_pipeline(&device)?;

        Ok(Self {
            device,
            queue,
            distance_pipeline,
            morse_pipeline,
            boundary_pipeline,
            reduction_pipeline,
        })
    }

    /// Compute all pairwise distances for Rips filtration on the GPU.
    ///
    /// This is the key operation for building Vietoris-Rips complexes.
    /// Hardware-specific speedups still require benchmark validation.
    ///
    /// # Arguments
    /// * `points` - Slice of (x, y, z) tuples or (x, y, z, w) for 4D
    ///
    /// # Returns
    /// Flattened n×n distance matrix
    pub async fn compute_distance_matrix(
        &self,
        points: &[(f64, f64, f64)],
    ) -> GpuTopologyResult<Vec<f64>> {
        let num_points = points.len();
        if num_points == 0 {
            return Ok(Vec::new());
        }

        // Convert to GPU format
        let gpu_points: Vec<GpuPoint> = points
            .iter()
            .map(|&(x, y, z)| GpuPoint {
                x: x as f32,
                y: y as f32,
                z: z as f32,
                w: 0.0,
            })
            .collect();

        // Create GPU buffers
        let points_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Points Buffer"),
                contents: bytemuck::cast_slice(&gpu_points),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let output_size = (num_points * num_points * std::mem::size_of::<f32>()) as u64;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Distance Output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Distance Staging"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.distance_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Distance Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: points_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Dispatch compute shader
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Distance Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Distance Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.distance_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            let workgroup_count = num_points.div_ceil(8);
            compute_pass.dispatch_workgroups(workgroup_count as u32, workgroup_count as u32, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);
        self.queue.submit(std::iter::once(encoder.finish()));

        // Read results
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);

        receiver
            .await
            .map_err(|_| {
                GpuTopologyError::BufferError("Failed to receive buffer mapping".to_string())
            })?
            .map_err(|e| {
                GpuTopologyError::BufferError(format!("Buffer mapping failed: {:?}", e))
            })?;

        let data = buffer_slice.get_mapped_range();
        let result_f32: &[f32] = bytemuck::cast_slice(&data);
        let distances: Vec<f64> = result_f32.iter().map(|&d| d as f64).collect();

        drop(data);
        staging_buffer.unmap();

        Ok(distances)
    }

    /// Find Morse critical points on a 2D height function grid on the GPU.
    ///
    /// This operation is embarrassingly parallel. Hardware-specific speedups
    /// still require benchmark validation.
    ///
    /// # Arguments
    /// * `values` - Height values on a width×height grid (row-major)
    /// * `width` - Grid width
    /// * `height` - Grid height
    ///
    /// # Returns
    /// Vector of (x, y, critical_type, value) tuples
    pub async fn find_critical_points_2d(
        &self,
        values: &[f64],
        width: usize,
        height: usize,
    ) -> GpuTopologyResult<Vec<GpuCriticalPoint>> {
        if values.len() != width * height {
            return Err(GpuTopologyError::InvalidSize(format!(
                "Expected {} values for {}x{} grid, got {}",
                width * height,
                width,
                height,
                values.len()
            )));
        }

        if width < 3 || height < 3 {
            return Err(GpuTopologyError::InvalidSize(
                "Grid must be at least 3x3 for critical point detection".to_string(),
            ));
        }

        // Convert to f32 for GPU
        let values_f32: Vec<f32> = values.iter().map(|&v| v as f32).collect();

        // Create dimension buffer
        let dims = [width as u32, height as u32];

        let values_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Height Values"),
                contents: bytemuck::cast_slice(&values_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let dims_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Dimensions"),
                contents: bytemuck::cast_slice(&dims),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        // Max possible critical points is all interior points
        let max_critical = (width - 2) * (height - 2);
        let output_size = (max_critical * std::mem::size_of::<GpuCriticalPointBuffer>()) as u64;

        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Critical Points Output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Counter for number of critical points found
        let counter_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Critical Point Counter"),
                contents: bytemuck::cast_slice(&[0u32]),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            });

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Critical Points Staging"),
            size: output_size + 4, // +4 for counter
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.morse_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Morse Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: values_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: dims_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: counter_buffer.as_entire_binding(),
                },
            ],
        });

        // Dispatch
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Morse Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Morse Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.morse_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            // Interior points only (exclude boundary)
            let workgroup_x = (width - 2).div_ceil(16);
            let workgroup_y = (height - 2).div_ceil(16);
            compute_pass.dispatch_workgroups(workgroup_x as u32, workgroup_y as u32, 1);
        }

        // Copy counter first, then output
        encoder.copy_buffer_to_buffer(&counter_buffer, 0, &staging_buffer, 0, 4);
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 4, output_size);

        self.queue.submit(std::iter::once(encoder.finish()));

        // Read results
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);

        receiver
            .await
            .map_err(|_| {
                GpuTopologyError::BufferError("Failed to receive buffer mapping".to_string())
            })?
            .map_err(|e| {
                GpuTopologyError::BufferError(format!("Buffer mapping failed: {:?}", e))
            })?;

        let data = buffer_slice.get_mapped_range();
        let count = bytemuck::from_bytes::<u32>(&data[0..4]);
        let num_points = (*count as usize).min(max_critical);

        let mut critical_points = Vec::with_capacity(num_points);
        for i in 0..num_points {
            let offset = 4 + i * std::mem::size_of::<GpuCriticalPointBuffer>();
            let gpu_cp: &GpuCriticalPointBuffer = bytemuck::from_bytes(&data[offset..offset + 16]);

            let (critical_type, morse_index) = match gpu_cp.critical_type {
                0 => (CriticalType::Minimum, 0),
                1 => (CriticalType::Saddle(1), 1),
                2 => (CriticalType::Maximum, 2),
                _ => (CriticalType::Saddle(gpu_cp.critical_type as usize), 1),
            };

            critical_points.push(GpuCriticalPoint {
                position: (gpu_cp.x as usize, gpu_cp.y as usize),
                critical_type,
                value: gpu_cp.value as f64,
                index: morse_index,
            });
        }

        drop(data);
        staging_buffer.unmap();

        Ok(critical_points)
    }

    /// Build a Vietoris-Rips filtration from a distance matrix using CPU logic.
    ///
    /// Distance computation may be GPU-backed, but filtration/clique construction
    /// currently runs on CPU for correctness.
    ///
    /// # Arguments
    /// * `distances` - Flattened n×n distance matrix
    /// * `num_points` - Number of points
    /// * `max_distance` - Maximum filtration distance
    /// * `max_dimension` - Maximum simplex dimension to construct
    ///
    /// # Returns
    /// Vector of (simplex_vertices, filtration_time) pairs
    pub async fn build_rips_filtration(
        &self,
        distances: &[f64],
        num_points: usize,
        max_distance: f64,
        max_dimension: usize,
    ) -> GpuTopologyResult<Vec<(Vec<usize>, f64)>> {
        Self::validate_distance_matrix(distances, num_points)?;

        let mut filtration = Vec::new();

        // Add 0-simplices (vertices)
        for i in 0..num_points {
            filtration.push((vec![i], 0.0));
        }

        // Add 1-simplices (edges) - this is where GPU helps most
        for i in 0..num_points {
            for j in (i + 1)..num_points {
                let d = distances[i * num_points + j];
                if d <= max_distance {
                    filtration.push((vec![i, j], d));
                }
            }
        }

        // Higher-dimensional simplices (clique detection)
        if max_dimension >= 2 {
            // TODO(0.20.x+): replace clique detection with validated GPU kernels.
            for dim in 2..=max_dimension {
                let mut new_simplices = Vec::new();

                // Find (dim+1)-cliques by extending dim-cliques
                let current_dim_simplices: Vec<_> =
                    filtration.iter().filter(|(v, _)| v.len() == dim).collect();

                for (simplex, birth_time) in &current_dim_simplices {
                    // Try to extend with each vertex
                    'vertex: for v in 0..num_points {
                        if simplex.contains(&v) {
                            continue;
                        }

                        // Check if v forms edges with all vertices in simplex
                        let mut max_edge_dist = *birth_time;
                        for &u in simplex.iter() {
                            let edge_idx = if u < v {
                                u * num_points + v
                            } else {
                                v * num_points + u
                            };
                            let d = distances[edge_idx];
                            if d > max_distance {
                                continue 'vertex;
                            }
                            max_edge_dist = max_edge_dist.max(d);
                        }

                        // Found a valid (dim+1)-simplex
                        let mut new_simplex = simplex.clone();
                        new_simplex.push(v);
                        new_simplex.sort();
                        new_simplices.push((new_simplex, max_edge_dist));
                    }
                }

                // Deduplicate (sort by vertices first, then by time)
                new_simplices.sort_by(|a, b| {
                    a.0.cmp(&b.0)
                        .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
                });
                new_simplices.dedup();
                filtration.extend(new_simplices);
            }
        }

        // Sort by filtration time
        filtration.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        Ok(filtration)
    }

    /// Compute Betti numbers using the CPU homology baseline.
    ///
    /// Boundary/reduction shader scaffolding exists, but validated GPU persistent
    /// homology is not yet exposed as public behavior.
    ///
    /// # Arguments
    /// * `complex` - The simplicial complex
    ///
    /// # Returns
    /// Vector of Betti numbers [β₀, β₁, β₂, ...]
    pub async fn compute_betti_numbers(
        &self,
        complex: &SimplicialComplex,
    ) -> GpuTopologyResult<Vec<usize>> {
        let _total_simplices = complex.total_simplices();
        // TODO(0.20.x+): replace with validated sparse boundary-matrix GPU reduction.
        Ok(complex.betti_numbers())
    }

    /// Determine if GPU should be used based on problem size
    pub fn should_use_gpu_distance(num_points: usize) -> bool {
        num_points >= 100
    }

    /// Threshold reserved for future GPU Betti number computation.
    pub fn gpu_threshold_betti() -> usize {
        500
    }

    /// Threshold for GPU critical point detection
    pub fn should_use_gpu_morse(grid_size: usize) -> bool {
        grid_size >= 10000 // 100x100 or larger
    }

    fn validate_distance_matrix(distances: &[f64], num_points: usize) -> GpuTopologyResult<()> {
        let expected = num_points.checked_mul(num_points).ok_or_else(|| {
            GpuTopologyError::InvalidSize("Distance matrix dimensions overflow".to_string())
        })?;
        if distances.len() != expected {
            return Err(GpuTopologyError::InvalidSize(format!(
                "Expected flattened {num_points}x{num_points} distance matrix with {expected} entries, got {}",
                distances.len()
            )));
        }
        Ok(())
    }

    // Pipeline creation methods

    fn create_distance_pipeline(device: &wgpu::Device) -> Result<wgpu::ComputePipeline, GpuError> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Distance Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(DISTANCE_MATRIX_SHADER.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Distance Pipeline"),
            layout: None,
            module: &shader,
            entry_point: "main",
        });

        Ok(pipeline)
    }

    fn create_morse_pipeline(device: &wgpu::Device) -> Result<wgpu::ComputePipeline, GpuError> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Morse Critical Point Shader"),
            source: wgpu::ShaderSource::Wgsl(MORSE_CRITICAL_SHADER.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Morse Pipeline"),
            layout: None,
            module: &shader,
            entry_point: "main",
        });

        Ok(pipeline)
    }

    fn create_boundary_pipeline(device: &wgpu::Device) -> Result<wgpu::ComputePipeline, GpuError> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Boundary Matrix Shader"),
            source: wgpu::ShaderSource::Wgsl(BOUNDARY_MATRIX_SHADER.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Boundary Pipeline"),
            layout: None,
            module: &shader,
            entry_point: "main",
        });

        Ok(pipeline)
    }

    fn create_reduction_pipeline(device: &wgpu::Device) -> Result<wgpu::ComputePipeline, GpuError> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Matrix Reduction Shader"),
            source: wgpu::ShaderSource::Wgsl(MATRIX_REDUCTION_SHADER.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Reduction Pipeline"),
            layout: None,
            module: &shader,
            entry_point: "main",
        });

        Ok(pipeline)
    }
}

/// Adaptive CPU/GPU dispatcher for topology operations.
///
/// Uses GPU distance/Morse kernels when available and above conservative
/// thresholds. Rips filtration construction and Betti numbers currently use CPU
/// topology baselines after optional GPU distance computation.
pub struct AdaptiveTopologyCompute {
    gpu: Option<GpuTopology>,
}

impl AdaptiveTopologyCompute {
    /// Create with optional GPU acceleration
    pub async fn new() -> Self {
        let gpu = {
            let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                pollster::block_on(async { GpuTopology::new().await.ok() })
            }));
            panic_result.unwrap_or_default()
        };

        Self { gpu }
    }

    /// Check if GPU is available
    pub fn has_gpu(&self) -> bool {
        self.gpu.is_some()
    }

    /// Compute distance matrix with adaptive dispatch
    pub async fn compute_distance_matrix(
        &self,
        points: &[(f64, f64, f64)],
    ) -> GpuTopologyResult<Vec<f64>> {
        let num_points = points.len();

        if let Some(gpu) = &self.gpu {
            if GpuTopology::should_use_gpu_distance(num_points) {
                return gpu.compute_distance_matrix(points).await;
            }
        }

        // CPU fallback
        Ok(Self::compute_distance_matrix_cpu(points))
    }

    /// Find critical points with adaptive dispatch
    pub async fn find_critical_points_2d(
        &self,
        values: &[f64],
        width: usize,
        height: usize,
    ) -> GpuTopologyResult<Vec<GpuCriticalPoint>> {
        if values.len() != width * height {
            return Err(GpuTopologyError::InvalidSize(format!(
                "Expected {} values for {}x{} grid, got {}",
                width * height,
                width,
                height,
                values.len()
            )));
        }

        if width < 3 || height < 3 {
            return Err(GpuTopologyError::InvalidSize(
                "Grid must be at least 3x3 for critical point detection".to_string(),
            ));
        }

        let grid_size = width * height;

        if let Some(gpu) = &self.gpu {
            if GpuTopology::should_use_gpu_morse(grid_size) {
                return gpu.find_critical_points_2d(values, width, height).await;
            }
        }

        // CPU fallback
        Ok(Self::find_critical_points_cpu(values, width, height))
    }

    /// Build Rips filtration with adaptive distance computation.
    ///
    /// Distance matrix computation may use GPU for sufficiently large point sets;
    /// filtration construction currently uses CPU logic.
    pub async fn build_rips_filtration(
        &self,
        points: &[(f64, f64, f64)],
        max_distance: f64,
        max_dimension: usize,
    ) -> GpuTopologyResult<Vec<(Vec<usize>, f64)>> {
        let num_points = points.len();

        // Always use GPU for distance matrix if available and beneficial
        let distances = self.compute_distance_matrix(points).await?;

        if let Some(gpu) = &self.gpu {
            if GpuTopology::should_use_gpu_distance(num_points) {
                return gpu
                    .build_rips_filtration(&distances, num_points, max_distance, max_dimension)
                    .await;
            }
        }

        // CPU fallback for filtration construction.
        Self::build_rips_filtration_cpu(&distances, num_points, max_distance, max_dimension)
    }

    // CPU fallback implementations

    fn compute_distance_matrix_cpu(points: &[(f64, f64, f64)]) -> Vec<f64> {
        let n = points.len();
        let mut distances = vec![0.0; n * n];

        for i in 0..n {
            for j in 0..n {
                if i != j {
                    let dx = points[i].0 - points[j].0;
                    let dy = points[i].1 - points[j].1;
                    let dz = points[i].2 - points[j].2;
                    distances[i * n + j] = (dx * dx + dy * dy + dz * dz).sqrt();
                }
            }
        }

        distances
    }

    fn find_critical_points_cpu(
        values: &[f64],
        width: usize,
        height: usize,
    ) -> Vec<GpuCriticalPoint> {
        let mut critical_points = Vec::new();

        for y in 1..(height - 1) {
            for x in 1..(width - 1) {
                let v = values[y * width + x];

                // Get 8-neighbors
                let neighbors = [
                    values[(y - 1) * width + (x - 1)],
                    values[(y - 1) * width + x],
                    values[(y - 1) * width + (x + 1)],
                    values[y * width + (x - 1)],
                    values[y * width + (x + 1)],
                    values[(y + 1) * width + (x - 1)],
                    values[(y + 1) * width + x],
                    values[(y + 1) * width + (x + 1)],
                ];

                let lower_count = neighbors.iter().filter(|&&n| n < v).count();
                let upper_count = neighbors.iter().filter(|&&n| n > v).count();

                let critical_info: Option<(CriticalType, usize)> = if lower_count == 8 {
                    Some((CriticalType::Maximum, 2))
                } else if upper_count == 8 {
                    Some((CriticalType::Minimum, 0))
                } else if lower_count > 0 && upper_count > 0 {
                    // Count sign changes around the boundary
                    let signs: Vec<bool> = neighbors.iter().map(|&n| n > v).collect();
                    let mut changes = 0;
                    for i in 0..8 {
                        if signs[i] != signs[(i + 1) % 8] {
                            changes += 1;
                        }
                    }
                    if changes >= 4 {
                        Some((CriticalType::Saddle(changes / 2 - 1), 1))
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some((ct, morse_index)) = critical_info {
                    critical_points.push(GpuCriticalPoint {
                        position: (x, y),
                        critical_type: ct,
                        value: v,
                        index: morse_index,
                    });
                }
            }
        }

        critical_points
    }

    fn build_rips_filtration_cpu(
        distances: &[f64],
        num_points: usize,
        max_distance: f64,
        max_dimension: usize,
    ) -> GpuTopologyResult<Vec<(Vec<usize>, f64)>> {
        GpuTopology::validate_distance_matrix(distances, num_points)?;

        let mut filtration = Vec::new();

        // Add 0-simplices
        for i in 0..num_points {
            filtration.push((vec![i], 0.0));
        }

        // Add 1-simplices
        for i in 0..num_points {
            for j in (i + 1)..num_points {
                let d = distances[i * num_points + j];
                if d <= max_distance {
                    filtration.push((vec![i, j], d));
                }
            }
        }

        // Higher dimensions
        for dim in 2..=max_dimension {
            let mut new_simplices = Vec::new();
            let current: Vec<_> = filtration
                .iter()
                .filter(|(v, _)| v.len() == dim)
                .cloned()
                .collect();

            for (simplex, birth) in current {
                'vertex: for v in 0..num_points {
                    if simplex.contains(&v) {
                        continue;
                    }

                    let mut max_dist = birth;
                    for &u in &simplex {
                        let idx = u.min(v) * num_points + u.max(v);
                        let d = distances[idx];
                        if d > max_distance {
                            continue 'vertex;
                        }
                        max_dist = max_dist.max(d);
                    }

                    let mut new_simplex = simplex.clone();
                    new_simplex.push(v);
                    new_simplex.sort();
                    new_simplices.push((new_simplex, max_dist));
                }
            }

            new_simplices.sort_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
            });
            new_simplices.dedup();
            filtration.extend(new_simplices);
        }

        filtration.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        Ok(filtration)
    }
}

// =====================================================================
// WGSL COMPUTE SHADERS
// =====================================================================

/// Distance matrix computation shader
const DISTANCE_MATRIX_SHADER: &str = r#"
struct Point {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

@group(0) @binding(0)
var<storage, read> points: array<Point>;

@group(0) @binding(1)
var<storage, read_write> distances: array<f32>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let i = global_id.x;
    let j = global_id.y;
    let num_points = arrayLength(&points);

    if (i >= num_points || j >= num_points) {
        return;
    }

    let idx = i * num_points + j;

    if (i == j) {
        distances[idx] = 0.0;
        return;
    }

    let pi = points[i];
    let pj = points[j];

    let dx = pi.x - pj.x;
    let dy = pi.y - pj.y;
    let dz = pi.z - pj.z;
    let dw = pi.w - pj.w;

    // Euclidean distance (supports up to 4D)
    distances[idx] = sqrt(dx * dx + dy * dy + dz * dz + dw * dw);
}
"#;

/// Morse critical point detection shader
const MORSE_CRITICAL_SHADER: &str = r#"
struct CriticalPoint {
    x: u32,
    y: u32,
    critical_type: u32,  // 0=min, 1=saddle, 2=max
    value: f32,
}

@group(0) @binding(0)
var<storage, read> values: array<f32>;

@group(0) @binding(1)
var<uniform> dims: vec2<u32>;  // width, height

@group(0) @binding(2)
var<storage, read_write> critical_points: array<CriticalPoint>;

@group(0) @binding(3)
var<storage, read_write> counter: atomic<u32>;

fn get_value(x: u32, y: u32) -> f32 {
    return values[y * dims.x + x];
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Interior points only (offset by 1)
    let x = global_id.x + 1u;
    let y = global_id.y + 1u;

    if (x >= dims.x - 1u || y >= dims.y - 1u) {
        return;
    }

    let v = get_value(x, y);

    // Get 8-neighbors
    let n0 = get_value(x - 1u, y - 1u);
    let n1 = get_value(x, y - 1u);
    let n2 = get_value(x + 1u, y - 1u);
    let n3 = get_value(x - 1u, y);
    let n4 = get_value(x + 1u, y);
    let n5 = get_value(x - 1u, y + 1u);
    let n6 = get_value(x, y + 1u);
    let n7 = get_value(x + 1u, y + 1u);

    // Count neighbors lower/higher than center
    var lower_count = 0u;
    var upper_count = 0u;

    if (n0 < v) { lower_count += 1u; } else if (n0 > v) { upper_count += 1u; }
    if (n1 < v) { lower_count += 1u; } else if (n1 > v) { upper_count += 1u; }
    if (n2 < v) { lower_count += 1u; } else if (n2 > v) { upper_count += 1u; }
    if (n3 < v) { lower_count += 1u; } else if (n3 > v) { upper_count += 1u; }
    if (n4 < v) { lower_count += 1u; } else if (n4 > v) { upper_count += 1u; }
    if (n5 < v) { lower_count += 1u; } else if (n5 > v) { upper_count += 1u; }
    if (n6 < v) { lower_count += 1u; } else if (n6 > v) { upper_count += 1u; }
    if (n7 < v) { lower_count += 1u; } else if (n7 > v) { upper_count += 1u; }

    var critical_type = 3u;  // 3 = not critical

    if (lower_count == 8u) {
        critical_type = 2u;  // Maximum
    } else if (upper_count == 8u) {
        critical_type = 0u;  // Minimum
    } else if (lower_count > 0u && upper_count > 0u) {
        // Check for saddle by counting sign changes
        var signs = array<bool, 8>(
            n0 > v, n1 > v, n2 > v, n3 > v, n4 > v, n5 > v, n6 > v, n7 > v
        );

        // Reorder to circular (corners then edges)
        // Actually use 4-connected ordering for change detection
        var changes = 0u;
        if (signs[0] != signs[1]) { changes += 1u; }
        if (signs[1] != signs[2]) { changes += 1u; }
        if (signs[2] != signs[4]) { changes += 1u; }
        if (signs[4] != signs[7]) { changes += 1u; }
        if (signs[7] != signs[6]) { changes += 1u; }
        if (signs[6] != signs[5]) { changes += 1u; }
        if (signs[5] != signs[3]) { changes += 1u; }
        if (signs[3] != signs[0]) { changes += 1u; }

        if (changes >= 4u) {
            critical_type = 1u;  // Saddle
        }
    }

    if (critical_type < 3u) {
        let idx = atomicAdd(&counter, 1u);
        critical_points[idx] = CriticalPoint(x, y, critical_type, v);
    }
}
"#;

/// Reserved boundary matrix construction shader.
///
/// Public Betti computation currently uses the CPU topology baseline, so this
/// pipeline is intentionally a validation-safe no-op placeholder until the full
/// sparse boundary kernel is restored and tested.
const BOUNDARY_MATRIX_SHADER: &str = r#"
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) _global_id: vec3<u32>) {
    return;
}
"#;

/// Parallel matrix reduction shader (for homology computation)
const MATRIX_REDUCTION_SHADER: &str = r#"
// Parallel column reduction using GPU
// This is a simplified version - full implementation would need
// multiple passes for complete reduction

@group(0) @binding(0)
var<storage, read_write> matrix: array<i32>;

@group(0) @binding(1)
var<uniform> dims: vec2<u32>;  // rows, cols

@group(0) @binding(2)
var<storage, read_write> pivots: array<u32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let col = global_id.x;
    let rows = dims.x;
    let cols = dims.y;

    if (col >= cols) {
        return;
    }

    // Find lowest 1 in column (pivot row)
    var pivot_row = rows;
    for (var row = 0u; row < rows; row++) {
        let idx = row * cols + col;
        if (matrix[idx] != 0) {
            pivot_row = row;
        }
    }

    pivots[col] = pivot_row;

    // Reduction would happen across multiple passes
    // This shader just identifies pivots
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_use_gpu() {
        assert!(!GpuTopology::should_use_gpu_distance(10));
        assert!(GpuTopology::should_use_gpu_distance(1000));
        assert!(!GpuTopology::should_use_gpu_morse(100));
        assert!(GpuTopology::should_use_gpu_morse(100000));
    }

    #[tokio::test]
    async fn test_adaptive_topology_creation() {
        let adaptive = AdaptiveTopologyCompute::new().await;

        match &adaptive.gpu {
            Some(_) => println!("GPU topology acceleration available"),
            None => println!("Using CPU fallback for topology operations"),
        }
    }

    #[tokio::test]
    async fn test_distance_matrix_cpu_fallback() {
        let adaptive = AdaptiveTopologyCompute::new().await;

        let points = vec![(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)];

        let distances = adaptive.compute_distance_matrix(&points).await.unwrap();
        assert_eq!(distances.len(), 9);

        // Check diagonal is zero
        assert_eq!(distances[0], 0.0);
        assert_eq!(distances[4], 0.0);
        assert_eq!(distances[8], 0.0);

        // Check d(0,1) = 1.0
        assert!((distances[1] - 1.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_critical_points_cpu_fallback() {
        let adaptive = AdaptiveTopologyCompute::new().await;

        // Simple 5x5 grid with a maximum in the center
        let width = 5;
        let height = 5;
        let mut values = vec![0.0; width * height];

        // Create a peak at center
        for y in 0..height {
            for x in 0..width {
                let dx = x as f64 - 2.0;
                let dy = y as f64 - 2.0;
                values[y * width + x] = -(dx * dx + dy * dy);
            }
        }

        let critical_points = adaptive
            .find_critical_points_2d(&values, width, height)
            .await
            .unwrap();

        // Should find at least the maximum at center
        assert!(!critical_points.is_empty());

        let has_max = critical_points
            .iter()
            .any(|cp| matches!(cp.critical_type, CriticalType::Maximum));
        assert!(has_max, "Should find maximum at center");
    }
}
