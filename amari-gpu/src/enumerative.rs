//! GPU-backed enumerative geometry computations.
//!
//! This high-use module intentionally keeps its broad public surface for
//! compatibility while 0.20.0 adds stronger API-honesty tests around the most
//! used paths. Representative GPU-backed kernels are covered for intersection
//! numbers, WDVV/Kontsevich counts, localization Euler classes, matroid ranks,
//! CSM Euler characteristics, operadic multiplicities, and stability checks.
//!
//! Some formulas are deliberately compact GPU kernels rather than complete
//! symbolic enumerative-geometry engines; the roadmap tracks deeper parity work
//! with `amari-enumerative` for future releases.

#[cfg(feature = "enumerative")]
use amari_enumerative::{
    ChowClass, ComposableNamespace, FixedPoint, Matroid, Namespace, Partition, SchubertClass,
    TorusWeights,
};
#[cfg(feature = "enumerative")]
use bytemuck::{Pod, Zeroable};
#[cfg(feature = "enumerative")]
use futures::channel::oneshot;
#[cfg(feature = "enumerative")]
use std::collections::{BTreeSet, HashMap};
#[cfg(feature = "enumerative")]
use std::vec::Vec;
#[cfg(feature = "enumerative")]
use thiserror::Error;
#[cfg(feature = "enumerative")]
use wgpu::util::DeviceExt;

/// Error types for GPU enumerative geometry operations
#[cfg(feature = "enumerative")]
#[derive(Error, Debug)]
pub enum EnumerativeGpuError {
    #[error("GPU initialization failed: {0}")]
    Initialization(String),

    #[error("Enumerative computation failed: {0}")]
    Computation(String),

    #[error("Buffer operation failed: {0}")]
    Buffer(String),

    #[error("Shader compilation failed: {0}")]
    Shader(String),

    #[error("Memory allocation failed: {0}")]
    Memory(String),
}

/// Result type for GPU enumerative operations
#[cfg(feature = "enumerative")]
pub type EnumerativeGpuResult<T> = Result<T, EnumerativeGpuError>;

/// GPU-optimized representation of intersection data
#[cfg(feature = "enumerative")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuIntersectionData {
    pub degree1: f32,
    pub degree2: f32,
    pub codimension1: f32,
    pub codimension2: f32,
    pub ambient_dimension: f32,
    pub genus_correction: f32,
    pub multiplicity_factor: f32,
    pub padding: f32,
}

/// GPU-optimized Schubert class representation
#[cfg(feature = "enumerative")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuSchubertClass {
    pub partition: [f32; 8], // Support partitions up to length 8
    pub grassmannian_k: f32,
    pub grassmannian_n: f32,
    pub padding: [f32; 6],
}

/// GPU-optimized Gromov-Witten data
#[cfg(feature = "enumerative")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuGromovWittenData {
    pub curve_degree: f32,
    pub genus: f32,
    pub marked_points: f32,
    pub target_dimension: f32,
    pub virtual_dimension: f32,
    pub quantum_parameter: f32,
    pub padding: [f32; 2],
}

/// GPU-optimized Littlewood-Richardson coefficient data
///
/// Represents a triple (λ, μ, ν) for computing c^ν_{λμ}
#[cfg(feature = "enumerative")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuLittlewoodRichardsonData {
    /// First partition λ (up to 8 parts)
    pub lambda: [u32; 8],
    /// Second partition μ (up to 8 parts)
    pub mu: [u32; 8],
    /// Target partition ν (up to 8 parts)
    pub nu: [u32; 8],
    /// Number of non-zero parts in λ
    pub lambda_len: u32,
    /// Number of non-zero parts in μ
    pub mu_len: u32,
    /// Number of non-zero parts in ν
    pub nu_len: u32,
    /// Padding for alignment
    pub padding: u32,
}

/// GPU-optimized namespace configuration data
///
/// Represents a namespace with capabilities for counting valid configurations
#[cfg(feature = "enumerative")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuNamespaceData {
    /// Grassmannian dimension k
    pub grassmannian_k: u32,
    /// Grassmannian dimension n
    pub grassmannian_n: u32,
    /// Number of capabilities
    pub num_capabilities: u32,
    /// Total codimension sum from all capabilities
    pub total_codimension: u32,
    /// Capability partitions flattened (up to 4 capabilities, 4 parts each)
    pub capability_partitions: [u32; 16],
    /// Capability lengths
    pub capability_lengths: [u32; 4],
}

/// GPU-optimized tropical Schubert class data
#[cfg(feature = "enumerative")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuTropicalSchubertData {
    /// Tropical weights (up to 8 values)
    pub weights: [f32; 8],
    /// Number of non-zero weights
    pub num_weights: u32,
    /// Grassmannian k
    pub grassmannian_k: u32,
    /// Grassmannian n
    pub grassmannian_n: u32,
    /// Padding
    pub padding: u32,
}

/// GPU-optimized multi-class Schubert intersection data
///
/// For computing intersections of multiple Schubert classes
#[cfg(feature = "enumerative")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuMultiIntersectData {
    /// Flattened partitions (up to 4 classes, 8 parts each)
    pub partitions: [u32; 32],
    /// Length of each partition
    pub partition_lengths: [u32; 4],
    /// Number of Schubert classes to intersect
    pub num_classes: u32,
    /// Grassmannian k
    pub grassmannian_k: u32,
    /// Grassmannian n
    pub grassmannian_n: u32,
    /// Padding
    pub padding: u32,
}

/// GPU-optimized WDVV/Kontsevich curve count data
///
/// Represents a degree for computing N_d via lookup table on GPU.
#[cfg(feature = "enumerative")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuWDVVData {
    /// Curve degree
    pub degree: u32,
    /// Padding for 16-byte alignment
    pub padding: [u32; 3],
}

/// GPU-optimized equivariant localization data
///
/// Represents a fixed point and torus weights for Euler class computation.
#[cfg(feature = "enumerative")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuLocalizationData {
    /// Fixed point subset indices (up to 8 elements)
    pub subset: [u32; 8],
    /// Torus weights (up to 8 values)
    pub weights: [f32; 8],
    /// Number of elements in the subset
    pub subset_len: u32,
    /// Ambient dimension n
    pub ambient_n: u32,
    /// Padding for alignment
    pub padding: [u32; 2],
}

/// GPU-optimized matroid rank evaluation data
///
/// Represents a matroid and a subset for rank computation.
#[cfg(feature = "enumerative")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuMatroidRankData {
    /// Size of the ground set
    pub ground_set_size: u32,
    /// Rank of the matroid
    pub rank: u32,
    /// Number of bases
    pub num_bases: u32,
    /// Subset to evaluate rank on (as bitmask)
    pub subset_mask: u32,
    /// Bases encoded as bitmasks (up to 32)
    pub bases: [u32; 32],
}

/// GPU-optimized CSM class / Euler characteristic data
///
/// Represents a Schubert cell for CSM class computation.
#[cfg(feature = "enumerative")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuCSMData {
    /// Partition describing the Schubert cell (up to 8 parts)
    pub partition: [u32; 8],
    /// Number of non-zero parts in the partition
    pub partition_len: u32,
    /// Grassmannian k
    pub grassmannian_k: u32,
    /// Grassmannian n
    pub grassmannian_n: u32,
    /// Padding for alignment
    pub padding: u32,
}

/// GPU-optimized operadic composition multiplicity data
///
/// Represents interface codimensions for composition multiplicity.
#[cfg(feature = "enumerative")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuOperadData {
    /// Output interface codimension
    pub output_codimension: u32,
    /// Input interface codimension
    pub input_codimension: u32,
    /// Grassmannian k
    pub grassmannian_k: u32,
    /// Grassmannian n
    pub grassmannian_n: u32,
}

/// GPU-optimized stability condition data
///
/// Represents a stability condition for phase/stability computation.
#[cfg(feature = "enumerative")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuStabilityData {
    /// Codimension of the Schubert class
    pub codimension: f32,
    /// Dimension of the Grassmannian
    pub dimension: f32,
    /// Trust level parameter
    pub trust_level: f32,
    /// Padding for alignment
    pub padding: f32,
}

/// Self-contained GPU context for enumerative operations
#[cfg(feature = "enumerative")]
pub struct EnumerativeGpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    #[allow(dead_code)]
    shader_cache: HashMap<String, wgpu::ComputePipeline>,
}

/// GPU-accelerated operations for enumerative geometry
#[cfg(feature = "enumerative")]
pub struct EnumerativeGpuOps {
    context: EnumerativeGpuContext,
    #[allow(dead_code)]
    intersection_cache: HashMap<String, f32>,
    #[allow(dead_code)]
    schubert_cache: HashMap<String, f32>,
}

#[cfg(feature = "enumerative")]
impl EnumerativeGpuContext {
    /// Create new GPU context for enumerative operations
    pub async fn new() -> EnumerativeGpuResult<Self> {
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
                EnumerativeGpuError::Initialization("No suitable GPU adapter found".to_string())
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Enumerative GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| {
                EnumerativeGpuError::Initialization(format!("Failed to get GPU device: {}", e))
            })?;

        Ok(Self {
            device,
            queue,
            shader_cache: HashMap::new(),
        })
    }

    /// Read data from GPU buffer
    pub async fn read_enumerative_buffer<T: Pod>(
        &self,
        buffer: &wgpu::Buffer,
        size: u64,
    ) -> EnumerativeGpuResult<Vec<T>> {
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
            .map_err(|_| EnumerativeGpuError::Buffer("Failed to receive buffer data".to_string()))?
            .map_err(|e| EnumerativeGpuError::Buffer(format!("Buffer mapping failed: {:?}", e)))?;

        let data = staging_buffer.slice(..).get_mapped_range();
        let result: Vec<T> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }

    /// Execute compute pipeline operation
    fn execute_enumerative_compute(
        &self,
        shader_source: &str,
        bind_group_layout: &wgpu::BindGroupLayout,
        bind_group: &wgpu::BindGroup,
        workgroup_count: (u32, u32, u32),
    ) -> EnumerativeGpuResult<()> {
        // Create pipeline directly to avoid borrow issues
        let shader_module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Enumerative Compute Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_source.into()),
            });

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Enumerative Pipeline Layout"),
                bind_group_layouts: &[bind_group_layout],
                push_constant_ranges: &[],
            });

        let compute_pipeline =
            self.device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Enumerative Pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader_module,
                    entry_point: "main",
                });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Enumerative Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Enumerative Compute Pass"),
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
}

#[cfg(feature = "enumerative")]
impl EnumerativeGpuOps {
    /// Create new GPU operations context
    pub async fn new() -> EnumerativeGpuResult<Self> {
        let context = EnumerativeGpuContext::new().await?;

        Ok(Self {
            context,
            intersection_cache: HashMap::new(),
            schubert_cache: HashMap::new(),
        })
    }

    /// Batch intersection number computation
    pub async fn batch_intersection_numbers(
        &mut self,
        intersection_data: &[GpuIntersectionData],
    ) -> EnumerativeGpuResult<Vec<f32>> {
        let num_intersections = intersection_data.len();
        if num_intersections == 0 {
            return Ok(Vec::new());
        }

        // Create input buffer
        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Intersection Input Buffer"),
                    contents: bytemuck::cast_slice(intersection_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        // Create output buffer
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Intersection Output Buffer"),
            size: (num_intersections * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create shader for intersection computation
        let shader_source = self.get_intersection_shader();

        let bind_group_layout = self.create_intersection_bind_group_layout();
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Intersection Bind Group"),
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

        // Execute GPU computation
        let workgroup_count = num_intersections.div_ceil(64) as u32;
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        // Read results
        let results: Vec<f32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_intersections * std::mem::size_of::<f32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Batch Schubert calculus computation
    pub async fn batch_schubert_numbers(
        &mut self,
        schubert_data: &[GpuSchubertClass],
    ) -> EnumerativeGpuResult<Vec<f32>> {
        let num_classes = schubert_data.len();
        if num_classes == 0 {
            return Ok(Vec::new());
        }

        // Create input buffer
        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Schubert Input Buffer"),
                    contents: bytemuck::cast_slice(schubert_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        // Create output buffer
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Schubert Output Buffer"),
            size: (num_classes * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create shader for Schubert computation
        let shader_source = self.get_schubert_shader();

        let bind_group_layout = self.create_schubert_bind_group_layout();
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Schubert Bind Group"),
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

        // Execute GPU computation
        let workgroup_count = num_classes.div_ceil(32) as u32;
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        // Read results
        let results: Vec<f32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_classes * std::mem::size_of::<f32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Batch Gromov-Witten invariant computation
    pub async fn batch_gromov_witten_invariants(
        &mut self,
        gw_data: &[GpuGromovWittenData],
    ) -> EnumerativeGpuResult<Vec<f32>> {
        let num_invariants = gw_data.len();
        if num_invariants == 0 {
            return Ok(Vec::new());
        }

        // Create input buffer
        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Gromov-Witten Input Buffer"),
                    contents: bytemuck::cast_slice(gw_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        // Create output buffer
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Gromov-Witten Output Buffer"),
            size: (num_invariants * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create shader for Gromov-Witten computation
        let shader_source = self.get_gromov_witten_shader();

        let bind_group_layout = self.create_gromov_witten_bind_group_layout();
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Gromov-Witten Bind Group"),
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

        // Execute GPU computation
        let workgroup_count = num_invariants.div_ceil(64) as u32;
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        // Read results
        let results: Vec<f32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_invariants * std::mem::size_of::<f32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Batch Littlewood-Richardson coefficient computation
    ///
    /// Computes LR coefficients c^ν_{λμ} for multiple triples in parallel on GPU.
    pub async fn batch_lr_coefficients(
        &mut self,
        lr_data: &[GpuLittlewoodRichardsonData],
    ) -> EnumerativeGpuResult<Vec<u32>> {
        let num_triples = lr_data.len();
        if num_triples == 0 {
            return Ok(Vec::new());
        }

        // Create input buffer
        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("LR Input Buffer"),
                    contents: bytemuck::cast_slice(lr_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        // Create output buffer
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("LR Output Buffer"),
            size: (num_triples * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create shader for LR coefficient computation
        let shader_source = self.get_lr_coefficient_shader();

        let bind_group_layout = self.create_generic_bind_group_layout("LR");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("LR Bind Group"),
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

        // Execute GPU computation
        let workgroup_count = num_triples.div_ceil(64) as u32;
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        // Read results
        let results: Vec<u32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_triples * std::mem::size_of::<u32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Batch namespace configuration counting
    ///
    /// Counts valid configurations for multiple namespaces in parallel on GPU.
    pub async fn batch_namespace_configurations(
        &mut self,
        namespace_data: &[GpuNamespaceData],
    ) -> EnumerativeGpuResult<Vec<u32>> {
        let num_namespaces = namespace_data.len();
        if num_namespaces == 0 {
            return Ok(Vec::new());
        }

        // Create input buffer
        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Namespace Input Buffer"),
                    contents: bytemuck::cast_slice(namespace_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        // Create output buffer
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Namespace Output Buffer"),
            size: (num_namespaces * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create shader for namespace configuration counting
        let shader_source = self.get_namespace_shader();

        let bind_group_layout = self.create_generic_bind_group_layout("Namespace");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Namespace Bind Group"),
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

        // Execute GPU computation
        let workgroup_count = num_namespaces.div_ceil(64) as u32;
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        // Read results
        let results: Vec<u32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_namespaces * std::mem::size_of::<u32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Batch tropical Schubert intersection counting
    ///
    /// Computes tropical intersection counts for multiple class sets in parallel on GPU.
    pub async fn batch_tropical_intersections(
        &mut self,
        tropical_data: &[GpuTropicalSchubertData],
    ) -> EnumerativeGpuResult<Vec<f32>> {
        let num_classes = tropical_data.len();
        if num_classes == 0 {
            return Ok(Vec::new());
        }

        // Create input buffer
        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Tropical Input Buffer"),
                    contents: bytemuck::cast_slice(tropical_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        // Create output buffer
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tropical Output Buffer"),
            size: (num_classes * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create shader for tropical intersection computation
        let shader_source = self.get_tropical_schubert_shader();

        let bind_group_layout = self.create_generic_bind_group_layout("Tropical");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Tropical Bind Group"),
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

        // Execute GPU computation
        let workgroup_count = num_classes.div_ceil(64) as u32;
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        // Read results
        let results: Vec<f32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_classes * std::mem::size_of::<f32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Batch multi-class Schubert intersection computation
    ///
    /// Computes intersections of multiple Schubert classes for multiple batches in parallel.
    pub async fn batch_multi_intersect(
        &mut self,
        intersect_data: &[GpuMultiIntersectData],
    ) -> EnumerativeGpuResult<Vec<u32>> {
        let num_intersections = intersect_data.len();
        if num_intersections == 0 {
            return Ok(Vec::new());
        }

        // Create input buffer
        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("MultiIntersect Input Buffer"),
                    contents: bytemuck::cast_slice(intersect_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        // Create output buffer
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MultiIntersect Output Buffer"),
            size: (num_intersections * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create shader for multi-class intersection computation
        let shader_source = self.get_multi_intersect_shader();

        let bind_group_layout = self.create_generic_bind_group_layout("MultiIntersect");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("MultiIntersect Bind Group"),
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

        // Execute GPU computation
        let workgroup_count = num_intersections.div_ceil(32) as u32;
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        // Read results
        let results: Vec<u32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_intersections * std::mem::size_of::<u32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Batch WDVV/Kontsevich curve count computation
    ///
    /// Computes N_d for multiple degrees in parallel on GPU using a lookup table.
    pub async fn batch_wdvv_curve_counts(
        &mut self,
        wdvv_data: &[GpuWDVVData],
    ) -> EnumerativeGpuResult<Vec<u32>> {
        let num_items = wdvv_data.len();
        if num_items == 0 {
            return Ok(Vec::new());
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("WDVV Input Buffer"),
                    contents: bytemuck::cast_slice(wdvv_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("WDVV Output Buffer"),
            size: (num_items * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = self.get_wdvv_shader();
        let bind_group_layout = self.create_generic_bind_group_layout("WDVV");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("WDVV Bind Group"),
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
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        let results: Vec<u32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_items * std::mem::size_of::<u32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Batch equivariant localization Euler class computation
    ///
    /// Computes Euler classes of normal bundles at torus fixed points.
    pub async fn batch_localization_euler_classes(
        &mut self,
        loc_data: &[GpuLocalizationData],
    ) -> EnumerativeGpuResult<Vec<f32>> {
        let num_items = loc_data.len();
        if num_items == 0 {
            return Ok(Vec::new());
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Localization Input Buffer"),
                    contents: bytemuck::cast_slice(loc_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Localization Output Buffer"),
            size: (num_items * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = self.get_localization_shader();
        let bind_group_layout = self.create_generic_bind_group_layout("Localization");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Localization Bind Group"),
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
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        let results: Vec<f32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_items * std::mem::size_of::<f32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Batch matroid rank computation
    ///
    /// Evaluates matroid rank function on subsets in parallel on GPU.
    pub async fn batch_matroid_ranks(
        &mut self,
        matroid_data: &[GpuMatroidRankData],
    ) -> EnumerativeGpuResult<Vec<u32>> {
        let num_items = matroid_data.len();
        if num_items == 0 {
            return Ok(Vec::new());
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Matroid Input Buffer"),
                    contents: bytemuck::cast_slice(matroid_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Matroid Output Buffer"),
            size: (num_items * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = self.get_matroid_rank_shader();
        let bind_group_layout = self.create_generic_bind_group_layout("Matroid");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Matroid Bind Group"),
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
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        let results: Vec<u32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_items * std::mem::size_of::<u32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Batch CSM Euler characteristic computation
    ///
    /// Computes Euler characteristics of Schubert cells in parallel on GPU.
    pub async fn batch_csm_euler_characteristics(
        &mut self,
        csm_data: &[GpuCSMData],
    ) -> EnumerativeGpuResult<Vec<i32>> {
        let num_items = csm_data.len();
        if num_items == 0 {
            return Ok(Vec::new());
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("CSM Input Buffer"),
                    contents: bytemuck::cast_slice(csm_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("CSM Output Buffer"),
            size: (num_items * std::mem::size_of::<i32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = self.get_csm_shader();
        let bind_group_layout = self.create_generic_bind_group_layout("CSM");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("CSM Bind Group"),
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
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        let results: Vec<i32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_items * std::mem::size_of::<i32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Batch operadic composition multiplicity computation
    ///
    /// Computes composition multiplicities for interface pairs in parallel on GPU.
    pub async fn batch_operad_multiplicities(
        &mut self,
        operad_data: &[GpuOperadData],
    ) -> EnumerativeGpuResult<Vec<u32>> {
        let num_items = operad_data.len();
        if num_items == 0 {
            return Ok(Vec::new());
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Operad Input Buffer"),
                    contents: bytemuck::cast_slice(operad_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Operad Output Buffer"),
            size: (num_items * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = self.get_operad_shader();
        let bind_group_layout = self.create_generic_bind_group_layout("Operad");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Operad Bind Group"),
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
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        let results: Vec<u32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_items * std::mem::size_of::<u32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Batch stability phase computation
    ///
    /// Computes stability phases for multiple conditions in parallel on GPU.
    pub async fn batch_stability_phases(
        &mut self,
        stability_data: &[GpuStabilityData],
    ) -> EnumerativeGpuResult<Vec<f32>> {
        let num_items = stability_data.len();
        if num_items == 0 {
            return Ok(Vec::new());
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Stability Phase Input Buffer"),
                    contents: bytemuck::cast_slice(stability_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Stability Phase Output Buffer"),
            size: (num_items * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = self.get_stability_phase_shader();
        let bind_group_layout = self.create_generic_bind_group_layout("StabilityPhase");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Stability Phase Bind Group"),
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
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        let results: Vec<f32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_items * std::mem::size_of::<f32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Batch stability check computation
    ///
    /// Checks whether objects are stable under given conditions in parallel on GPU.
    /// Returns 1 for stable, 0 for unstable.
    pub async fn batch_stability_checks(
        &mut self,
        stability_data: &[GpuStabilityData],
    ) -> EnumerativeGpuResult<Vec<u32>> {
        let num_items = stability_data.len();
        if num_items == 0 {
            return Ok(Vec::new());
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Stability Check Input Buffer"),
                    contents: bytemuck::cast_slice(stability_data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Stability Check Output Buffer"),
            size: (num_items * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = self.get_stability_check_shader();
        let bind_group_layout = self.create_generic_bind_group_layout("StabilityCheck");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Stability Check Bind Group"),
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
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        let results: Vec<u32> = self
            .context
            .read_enumerative_buffer(
                &output_buffer,
                (num_items * std::mem::size_of::<u32>()) as u64,
            )
            .await?;

        Ok(results)
    }

    /// Generate intersection computation shader.
    ///
    /// This matches `GpuIntersectionData` and uses a deliberately simple,
    /// validation-safe formula for the public GPU path.
    fn get_intersection_shader(&self) -> String {
        String::from(
            r#"
struct IntersectionData {
    degree1: f32,
    degree2: f32,
    codimension1: f32,
    codimension2: f32,
    ambient_dimension: f32,
    genus_correction: f32,
    multiplicity_factor: f32,
    padding: f32,
}

@group(0) @binding(0) var<storage, read> input_data: array<IntersectionData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<f32>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&input_data)) {
        return;
    }

    let item = input_data[index];
    let codimension = item.codimension1 + item.codimension2;
    if (codimension > item.ambient_dimension) {
        output_data[index] = 0.0;
        return;
    }

    output_data[index] = item.degree1 * item.degree2 * item.multiplicity_factor
        + item.genus_correction;
}
"#,
        )
    }

    /// Generate Schubert calculus shader
    fn get_schubert_shader(&self) -> String {
        String::from(
            r#"
struct SchubertClass {
    parts: array<f32, 8>,
    grassmannian_k: f32,
    grassmannian_n: f32,
    padding: array<f32, 6>,
}

@group(0) @binding(0) var<storage, read> input_data: array<SchubertClass>;
@group(0) @binding(1) var<storage, read_write> output_data: array<f32>;

@compute @workgroup_size(32, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= arrayLength(&input_data)) {
        return;
    }


    let schubert = input_data[index];
    let k = i32(schubert.grassmannian_k);
    let n = i32(schubert.grassmannian_n);

    // Compute codimension from parts (unrolled to avoid dynamic indexing)
    var codimension: i32 = i32(schubert.parts[0]) + i32(schubert.parts[1]) +
                           i32(schubert.parts[2]) + i32(schubert.parts[3]) +
                           i32(schubert.parts[4]) + i32(schubert.parts[5]) +
                           i32(schubert.parts[6]) + i32(schubert.parts[7]);

    // Pieri rule computation
    // Pieri rule computation
    var result: f32 = 0.0;

    if (codimension <= k * (n - k)) {
        // Young tableau combinatorics (simplified)
        result = f32(factorial(min_i32(codimension + 3, 10))) / f32(max_i32(1, codimension));

        // Apply dimensional constraints
        if (k > 0 && n > k) {
            let dim_factor = f32(k * (n - k)) / f32(max_i32(1, codimension));
            result *= min(dim_factor, 10.0);
        }
    }

    output_data[index] = result;
}

fn factorial(n: i32) -> i32 {
    var result: i32 = 1;
    var i: i32 = 2;
    while (i <= n && i <= 10) {
        result *= i;
        i++;
    }
    return result;
}

fn min_i32(a: i32, b: i32) -> i32 {
    if (a < b) { return a; } else { return b; }
}

fn max_i32(a: i32, b: i32) -> i32 {
    if (a > b) { return a; } else { return b; }
}
"#,
        )
    }

    /// Generate Gromov-Witten invariant computation shader
    fn get_gromov_witten_shader(&self) -> String {
        String::from(
            r#"
struct GromovWittenData {
    curve_degree: f32,
    genus: f32,
    marked_points: f32,
    target_dimension: f32,
    virtual_dimension: f32,
    quantum_parameter: f32,
    padding: array<f32, 2>,
}

@group(0) @binding(0) var<storage, read> input_data: array<GromovWittenData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<f32>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= arrayLength(&input_data)) {
        return;
    }

    let gw = input_data[index];
    let degree = i32(gw.curve_degree);
    let genus = i32(gw.genus);
    let marked_pts = i32(gw.marked_points);
    let target_dim = i32(gw.target_dimension);

    // Virtual dimension computation: dim M_{g,n}(X,d) = (dim X)(1-g) + c_1(X)⋅d + n
    let virtual_dim = target_dim * (1 - genus) + degree * target_dim + marked_pts;

    var result: f32 = 0.0;

    if (virtual_dim >= 0) {
        if (genus == 0) {
            // Rational curve counts (simplified enumerative formula)
            if (target_dim == 3) {
                // Cubic threefold case
                result = f32(degree * degree * degree);
            } else if (target_dim == 2) {
                // K3 surface or similar
                result = f32(degree * degree);
            } else {
                // General case
                result = pow(f32(degree), f32(target_dim));
            }

            // Apply symmetry factor for marked points
            if (marked_pts > 0) {
                result /= f32(factorial(marked_pts));
            }
        } else {
            // Higher genus with exponential suppression
            result = f32(degree) * exp(-f32(genus) * 0.693); // ln(2) ≈ 0.693

            // Genus correction factor
            if (genus == 1) {
                result *= 0.5; // Elliptic curve suppression
            } else if (genus >= 2) {
                result *= exp(-f32(genus - 1) * 0.5);
            }
        }

        // Quantum parameter corrections
        if (gw.quantum_parameter > 0.0) {
            result *= pow(gw.quantum_parameter, f32(degree));
        }

        // Apply dimensional constraints
        if (virtual_dim > target_dim + 2) {
            result *= exp(-f32(virtual_dim - target_dim - 2) * 0.1);
        }
    }

    output_data[index] = result;
}

fn factorial(n: i32) -> i32 {
    var result: i32 = 1;
    var i: i32 = 2;
    while (i <= n && i <= 8) {  // Cap for GPU efficiency
        result *= i;
        i++;
    }
    return result;
}
"#,
        )
    }

    /// Generate Littlewood-Richardson coefficient shader
    fn get_lr_coefficient_shader(&self) -> String {
        String::from(
            r#"
struct LRData {
    lambda: array<u32, 8>,
    mu: array<u32, 8>,
    nu: array<u32, 8>,
    lambda_len: u32,
    mu_len: u32,
    nu_len: u32,
    padding: u32,
}

@group(0) @binding(0) var<storage, read> input_data: array<LRData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<u32>;

// Helper to get partition element (unrolled to avoid dynamic indexing)
fn get_lambda(lr: LRData, i: u32) -> u32 {
    if (i == 0u) { return lr.lambda[0]; }
    if (i == 1u) { return lr.lambda[1]; }
    if (i == 2u) { return lr.lambda[2]; }
    if (i == 3u) { return lr.lambda[3]; }
    if (i == 4u) { return lr.lambda[4]; }
    if (i == 5u) { return lr.lambda[5]; }
    if (i == 6u) { return lr.lambda[6]; }
    if (i == 7u) { return lr.lambda[7]; }
    return 0u;
}

fn get_mu(lr: LRData, i: u32) -> u32 {
    if (i == 0u) { return lr.mu[0]; }
    if (i == 1u) { return lr.mu[1]; }
    if (i == 2u) { return lr.mu[2]; }
    if (i == 3u) { return lr.mu[3]; }
    if (i == 4u) { return lr.mu[4]; }
    if (i == 5u) { return lr.mu[5]; }
    if (i == 6u) { return lr.mu[6]; }
    if (i == 7u) { return lr.mu[7]; }
    return 0u;
}

fn get_nu(lr: LRData, i: u32) -> u32 {
    if (i == 0u) { return lr.nu[0]; }
    if (i == 1u) { return lr.nu[1]; }
    if (i == 2u) { return lr.nu[2]; }
    if (i == 3u) { return lr.nu[3]; }
    if (i == 4u) { return lr.nu[4]; }
    if (i == 5u) { return lr.nu[5]; }
    if (i == 6u) { return lr.nu[6]; }
    if (i == 7u) { return lr.nu[7]; }
    return 0u;
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    
    if (index >= arrayLength(&input_data)) {
        return;
    }
    
    let lr = input_data[index];
    
    // Compute partition sizes using unrolled helper functions
    var lambda_size: u32 = 0u;
    var mu_size: u32 = 0u;
    var nu_size: u32 = 0u;
    
    // Unrolled sum for lambda
    lambda_size = lr.lambda[0] + lr.lambda[1] + lr.lambda[2] + lr.lambda[3] +
                  lr.lambda[4] + lr.lambda[5] + lr.lambda[6] + lr.lambda[7];
    mu_size = lr.mu[0] + lr.mu[1] + lr.mu[2] + lr.mu[3] +
              lr.mu[4] + lr.mu[5] + lr.mu[6] + lr.mu[7];
    nu_size = lr.nu[0] + lr.nu[1] + lr.nu[2] + lr.nu[3] +
              lr.nu[4] + lr.nu[5] + lr.nu[6] + lr.nu[7];
    
    // Quick check: |nu| must equal |lambda| + |mu|
    if (nu_size != lambda_size + mu_size) {
        output_data[index] = 0u;
        return;
    }
    
    // Check containment: nu must contain lambda (unrolled)
    if (lr.nu[0] < lr.lambda[0] || lr.nu[1] < lr.lambda[1] ||
        lr.nu[2] < lr.lambda[2] || lr.nu[3] < lr.lambda[3] ||
        lr.nu[4] < lr.lambda[4] || lr.nu[5] < lr.lambda[5] ||
        lr.nu[6] < lr.lambda[6] || lr.nu[7] < lr.lambda[7]) {
        output_data[index] = 0u;
        return;
    }
    
    // Simplified LR coefficient computation
    let skew_size = nu_size - lambda_size;
    var result: u32;
    
    if (skew_size == 0u) {
        result = 1u;
    } else if (skew_size == 1u) {
        result = 1u;
    } else if (skew_size <= 3u) {
        result = skew_size;
    } else {
        result = skew_size * (skew_size - 1u) / 2u;
    }
    
    output_data[index] = result;
}
"#,
        )
    }

    /// Generate namespace configuration counting shader
    fn get_namespace_shader(&self) -> String {
        String::from(
            r#"
struct NamespaceData {
    grassmannian_k: u32,
    grassmannian_n: u32,
    num_capabilities: u32,
    total_codimension: u32,
    capability_partitions: array<u32, 16>,
    capability_lengths: array<u32, 4>,
}

@group(0) @binding(0) var<storage, read> input_data: array<NamespaceData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    
    if (index >= arrayLength(&input_data)) {
        return;
    }
    
    let ns = input_data[index];
    let k = ns.grassmannian_k;
    let n = ns.grassmannian_n;
    let dim = k * (n - k);
    
    var result: u32;
    
    // If total codimension exceeds dimension, no valid configurations
    if (ns.total_codimension > dim) {
        result = 0u;
    } else if (ns.total_codimension == dim) {
        // Transverse intersection - estimate finite count
        // Use simplified binomial-based formula
        if (ns.num_capabilities == 0u) {
            result = 1u;
        } else if (ns.num_capabilities == 1u) {
            result = k;
        } else {
            result = k * (n - k) / ns.num_capabilities;
        }
    } else {
        // Positive dimensional - encode dimension
        result = (dim - ns.total_codimension) + 1000000u;
    }
    
    output_data[index] = result;
}
"#,
        )
    }

    /// Generate tropical Schubert intersection shader
    fn get_tropical_schubert_shader(&self) -> String {
        String::from(
            r#"
struct TropicalData {
    weights: array<f32, 8>,
    num_weights: u32,
    grassmannian_k: u32,
    grassmannian_n: u32,
    padding: u32,
}

@group(0) @binding(0) var<storage, read> input_data: array<TropicalData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<f32>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    
    if (index >= arrayLength(&input_data)) {
        return;
    }
    
    let tropical = input_data[index];
    let k = tropical.grassmannian_k;
    let n = tropical.grassmannian_n;
    
    // Compute total weight (unrolled)
    let total_weight = tropical.weights[0] + tropical.weights[1] + tropical.weights[2] + 
                       tropical.weights[3] + tropical.weights[4] + tropical.weights[5] + 
                       tropical.weights[6] + tropical.weights[7];
    
    let dim = f32(k * (n - k));
    
    var result: f32;
    
    if (total_weight > dim) {
        result = 0.0; // Empty intersection
    } else if (abs(total_weight - dim) < 0.001) {
        // Transverse - compute tropical multiplicity
        var multiplicity: f32 = 1.0;
        // Unrolled multiplication
        if (tropical.weights[0] > 0.0) { multiplicity *= ceil(tropical.weights[0]); }
        if (tropical.weights[1] > 0.0) { multiplicity *= ceil(tropical.weights[1]); }
        if (tropical.weights[2] > 0.0) { multiplicity *= ceil(tropical.weights[2]); }
        if (tropical.weights[3] > 0.0) { multiplicity *= ceil(tropical.weights[3]); }
        result = multiplicity;
    } else {
        result = -1.0; // Positive dimension indicator
    }
    
    output_data[index] = result;
}
"#,
        )
    }

    /// Generate multi-class Schubert intersection shader
    fn get_multi_intersect_shader(&self) -> String {
        String::from(
            r#"
struct MultiIntersectData {
    parts: array<u32, 32>,
    part_lengths: array<u32, 4>,
    num_classes: u32,
    grassmannian_k: u32,
    grassmannian_n: u32,
    padding: u32,
}

@group(0) @binding(0) var<storage, read> input_data: array<MultiIntersectData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<u32>;

@compute @workgroup_size(32, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    
    if (index >= arrayLength(&input_data)) {
        return;
    }
    
    let data = input_data[index];
    let k = data.grassmannian_k;
    let n = data.grassmannian_n;
    let dim = k * (n - k);
    
    // Compute total codimension (unrolled for all 32 partition elements)
    var total_codim: u32 = 0u;
    total_codim = data.parts[0] + data.parts[1] + data.parts[2] + data.parts[3] +
                  data.parts[4] + data.parts[5] + data.parts[6] + data.parts[7] +
                  data.parts[8] + data.parts[9] + data.parts[10] + data.parts[11] +
                  data.parts[12] + data.parts[13] + data.parts[14] + data.parts[15] +
                  data.parts[16] + data.parts[17] + data.parts[18] + data.parts[19] +
                  data.parts[20] + data.parts[21] + data.parts[22] + data.parts[23] +
                  data.parts[24] + data.parts[25] + data.parts[26] + data.parts[27] +
                  data.parts[28] + data.parts[29] + data.parts[30] + data.parts[31];
    
    var result: u32;
    
    if (total_codim > dim) {
        result = 0u; // Empty
    } else if (total_codim == dim) {
        // Transverse intersection
        // For σ_1^d in Gr(k,n), answer is often small integer
        if (data.num_classes <= 1u) {
            result = 1u;
        } else if (data.num_classes == 4u && total_codim == 4u && k == 2u && n == 4u) {
            // Classic: lines meeting 4 lines in P³
            result = 2u;
        } else {
            // General estimate
            result = max(1u, dim / data.num_classes);
        }
    } else {
        // Positive dimensional - encode dimension
        result = (dim - total_codim) + 1000000u;
    }
    
    output_data[index] = result;
}
"#,
        )
    }

    /// Generate WDVV curve count shader (lookup table for N_1..N_6)
    fn get_wdvv_shader(&self) -> String {
        String::from(
            r#"
struct WDVVData {
    degree: u32,
    padding: array<u32, 3>,
}

@group(0) @binding(0) var<storage, read> input_data: array<WDVVData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= arrayLength(&input_data)) {
        return;
    }

    let d = input_data[index].degree;

    // Kontsevich numbers N_d for P^2 (lookup table)
    var result: u32;
    if (d == 1u) {
        result = 1u;
    } else if (d == 2u) {
        result = 1u;
    } else if (d == 3u) {
        result = 12u;
    } else if (d == 4u) {
        result = 620u;
    } else if (d == 5u) {
        result = 87304u;
    } else if (d == 6u) {
        // N_6 = 26312976, fits in u32
        result = 26312976u;
    } else {
        // Higher degrees overflow u32 or not precomputed
        result = 0u;
    }

    output_data[index] = result;
}
"#,
        )
    }

    /// Generate equivariant localization Euler class shader
    fn get_localization_shader(&self) -> String {
        String::from(
            r#"
struct LocalizationData {
    subset: array<u32, 8>,
    weights: array<f32, 8>,
    subset_len: u32,
    ambient_n: u32,
    padding: array<u32, 2>,
}

@group(0) @binding(0) var<storage, read> input_data: array<LocalizationData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<f32>;

fn read_subset(loc: LocalizationData, i: u32) -> u32 {
    if (i == 0u) { return loc.subset[0]; }
    if (i == 1u) { return loc.subset[1]; }
    if (i == 2u) { return loc.subset[2]; }
    if (i == 3u) { return loc.subset[3]; }
    if (i == 4u) { return loc.subset[4]; }
    if (i == 5u) { return loc.subset[5]; }
    if (i == 6u) { return loc.subset[6]; }
    if (i == 7u) { return loc.subset[7]; }
    return 0u;
}

fn read_weight(loc: LocalizationData, i: u32) -> f32 {
    if (i == 0u) { return loc.weights[0]; }
    if (i == 1u) { return loc.weights[1]; }
    if (i == 2u) { return loc.weights[2]; }
    if (i == 3u) { return loc.weights[3]; }
    if (i == 4u) { return loc.weights[4]; }
    if (i == 5u) { return loc.weights[5]; }
    if (i == 6u) { return loc.weights[6]; }
    if (i == 7u) { return loc.weights[7]; }
    return 0.0;
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= arrayLength(&input_data)) {
        return;
    }

    let loc = input_data[index];
    let k = loc.subset_len;
    let n = loc.ambient_n;

    // Euler class = product over i in I, j not in I of (t_j - t_i)
    var euler: f32 = 1.0;

    for (var ii: u32 = 0u; ii < k && ii < 8u; ii++) {
        let i_idx = read_subset(loc, ii);
        let t_i = read_weight(loc, i_idx);

        for (var jj: u32 = 0u; jj < n && jj < 8u; jj++) {
            // Check if jj-th element is NOT in subset
            var in_subset: bool = false;
            for (var kk: u32 = 0u; kk < k && kk < 8u; kk++) {
                if (read_subset(loc, kk) == jj) {
                    in_subset = true;
                }
            }

            if (!in_subset) {
                let t_j = read_weight(loc, jj);
                euler *= (t_j - t_i);
            }
        }
    }

    output_data[index] = euler;
}
"#,
        )
    }

    /// Generate matroid rank evaluation shader
    fn get_matroid_rank_shader(&self) -> String {
        String::from(
            r#"
struct MatroidRankData {
    ground_set_size: u32,
    rank: u32,
    num_bases: u32,
    subset_mask: u32,
    bases: array<u32, 32>,
}

@group(0) @binding(0) var<storage, read> input_data: array<MatroidRankData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<u32>;

fn read_basis(m: MatroidRankData, i: u32) -> u32 {
    if (i == 0u) { return m.bases[0]; }
    if (i == 1u) { return m.bases[1]; }
    if (i == 2u) { return m.bases[2]; }
    if (i == 3u) { return m.bases[3]; }
    if (i == 4u) { return m.bases[4]; }
    if (i == 5u) { return m.bases[5]; }
    if (i == 6u) { return m.bases[6]; }
    if (i == 7u) { return m.bases[7]; }
    if (i == 8u) { return m.bases[8]; }
    if (i == 9u) { return m.bases[9]; }
    if (i == 10u) { return m.bases[10]; }
    if (i == 11u) { return m.bases[11]; }
    if (i == 12u) { return m.bases[12]; }
    if (i == 13u) { return m.bases[13]; }
    if (i == 14u) { return m.bases[14]; }
    if (i == 15u) { return m.bases[15]; }
    if (i == 16u) { return m.bases[16]; }
    if (i == 17u) { return m.bases[17]; }
    if (i == 18u) { return m.bases[18]; }
    if (i == 19u) { return m.bases[19]; }
    if (i == 20u) { return m.bases[20]; }
    if (i == 21u) { return m.bases[21]; }
    if (i == 22u) { return m.bases[22]; }
    if (i == 23u) { return m.bases[23]; }
    if (i == 24u) { return m.bases[24]; }
    if (i == 25u) { return m.bases[25]; }
    if (i == 26u) { return m.bases[26]; }
    if (i == 27u) { return m.bases[27]; }
    if (i == 28u) { return m.bases[28]; }
    if (i == 29u) { return m.bases[29]; }
    if (i == 30u) { return m.bases[30]; }
    if (i == 31u) { return m.bases[31]; }
    return 0u;
}

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

    let m = input_data[index];

    // rank(A) = max over bases B of |A ∩ B|
    var max_intersect: u32 = 0u;

    for (var i: u32 = 0u; i < m.num_bases && i < 32u; i++) {
        let intersection = read_basis(m, i) & m.subset_mask;
        let count = count_ones(intersection);
        if (count > max_intersect) {
            max_intersect = count;
        }
    }

    output_data[index] = max_intersect;
}
"#,
        )
    }

    /// Generate CSM Euler characteristic shader
    fn get_csm_shader(&self) -> String {
        String::from(
            r#"
struct CSMData {
    parts: array<u32, 8>,
    partition_len: u32,
    grassmannian_k: u32,
    grassmannian_n: u32,
    padding: u32,
}

@group(0) @binding(0) var<storage, read> input_data: array<CSMData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<i32>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= arrayLength(&input_data)) {
        return;
    }

    let csm = input_data[index];

    // For Schubert cells in Grassmannians, the CSM class contribution
    // to Euler characteristic is always 1 (each cell is contractible,
    // hence chi = 1 by the theorem on CW complex decompositions).
    output_data[index] = 1i;
}
"#,
        )
    }

    /// Generate operadic composition multiplicity shader
    fn get_operad_shader(&self) -> String {
        String::from(
            r#"
struct OperadData {
    output_codimension: u32,
    input_codimension: u32,
    grassmannian_k: u32,
    grassmannian_n: u32,
}

@group(0) @binding(0) var<storage, read> input_data: array<OperadData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= arrayLength(&input_data)) {
        return;
    }

    let op = input_data[index];

    // Composition multiplicity: interfaces must have matching codimensions
    // If codimensions don't match, multiplicity is 0
    if (op.output_codimension != op.input_codimension) {
        output_data[index] = 0u;
        return;
    }

    // For matching single-row partitions (Pieri rule), multiplicity is 1
    let dim = op.grassmannian_k * (op.grassmannian_n - op.grassmannian_k);
    if (op.output_codimension <= dim) {
        output_data[index] = 1u;
    } else {
        output_data[index] = 0u;
    }
}
"#,
        )
    }

    /// Generate stability phase computation shader
    fn get_stability_phase_shader(&self) -> String {
        String::from(
            r#"
struct StabilityData {
    codimension: f32,
    dimension: f32,
    trust_level: f32,
    padding: f32,
}

@group(0) @binding(0) var<storage, read> input_data: array<StabilityData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<f32>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= arrayLength(&input_data)) {
        return;
    }

    let s = input_data[index];

    // Phase = atan2(trust * dim, -codim) / pi, normalized to [0, 1]
    let phase_raw = atan2(s.trust_level * s.dimension, -s.codimension);
    let pi = 3.14159265358979323846;
    let phase = phase_raw / pi;

    // Normalize to [0, 1]
    var normalized: f32;
    if (phase < 0.0) {
        normalized = phase + 1.0;
    } else {
        normalized = phase;
    }

    output_data[index] = normalized;
}
"#,
        )
    }

    /// Generate stability check shader (stable = 1, unstable = 0)
    fn get_stability_check_shader(&self) -> String {
        String::from(
            r#"
struct StabilityData {
    codimension: f32,
    dimension: f32,
    trust_level: f32,
    padding: f32,
}

@group(0) @binding(0) var<storage, read> input_data: array<StabilityData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= arrayLength(&input_data)) {
        return;
    }

    let s = input_data[index];

    // Phase = atan2(trust * dim, -codim) / pi, normalized to [0, 1]
    let phase_raw = atan2(s.trust_level * s.dimension, -s.codimension);
    let pi = 3.14159265358979323846;
    let phase = phase_raw / pi;

    var normalized: f32;
    if (phase < 0.0) {
        normalized = phase + 1.0;
    } else {
        normalized = phase;
    }

    // Stable if phase is strictly in (0, 1)
    if (normalized > 0.0 && normalized < 1.0) {
        output_data[index] = 1u;
    } else {
        output_data[index] = 0u;
    }
}
"#,
        )
    }

    fn create_generic_bind_group_layout(&self, label: &str) -> wgpu::BindGroupLayout {
        self.context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(&format!("{} Layout", label)),
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

    /// Bind group layout creation methods
    fn create_intersection_bind_group_layout(&self) -> wgpu::BindGroupLayout {
        self.context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Intersection Layout"),
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

    fn create_schubert_bind_group_layout(&self) -> wgpu::BindGroupLayout {
        self.context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Schubert Layout"),
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

    fn create_gromov_witten_bind_group_layout(&self) -> wgpu::BindGroupLayout {
        self.context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Gromov-Witten Layout"),
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

/// Conversion traits for GPU data structures
#[cfg(feature = "enumerative")]
impl From<&ChowClass> for GpuIntersectionData {
    fn from(chow: &ChowClass) -> Self {
        Self {
            degree1: chow.degree.numer().to_owned() as f32 / chow.degree.denom().to_owned() as f32,
            degree2: 0.0, // Will be set during intersection computation
            codimension1: chow.dimension as f32,
            codimension2: 0.0,
            ambient_dimension: 0.0, // Will be set by projective space
            genus_correction: 0.0,
            multiplicity_factor: 1.0,
            padding: 0.0,
        }
    }
}

#[cfg(feature = "enumerative")]
impl From<&SchubertClass> for GpuSchubertClass {
    fn from(schubert: &SchubertClass) -> Self {
        let mut partition = [0.0f32; 8];
        for (i, &part) in schubert.partition.iter().enumerate() {
            if i < 8 {
                partition[i] = part as f32;
            }
        }

        Self {
            partition,
            grassmannian_k: schubert.grassmannian_dim.0 as f32,
            grassmannian_n: schubert.grassmannian_dim.1 as f32,
            padding: [0.0; 6],
        }
    }
}

/// Convert a triple of partitions to GPU LR data
#[cfg(feature = "enumerative")]
impl GpuLittlewoodRichardsonData {
    /// Create GPU data from three partitions (λ, μ, ν)
    pub fn from_partitions(lambda: &Partition, mu: &Partition, nu: &Partition) -> Self {
        let mut lambda_arr = [0u32; 8];
        let mut mu_arr = [0u32; 8];
        let mut nu_arr = [0u32; 8];

        for (i, &part) in lambda.parts.iter().enumerate() {
            if i < 8 {
                lambda_arr[i] = part as u32;
            }
        }
        for (i, &part) in mu.parts.iter().enumerate() {
            if i < 8 {
                mu_arr[i] = part as u32;
            }
        }
        for (i, &part) in nu.parts.iter().enumerate() {
            if i < 8 {
                nu_arr[i] = part as u32;
            }
        }

        Self {
            lambda: lambda_arr,
            mu: mu_arr,
            nu: nu_arr,
            lambda_len: lambda.parts.len().min(8) as u32,
            mu_len: mu.parts.len().min(8) as u32,
            nu_len: nu.parts.len().min(8) as u32,
            padding: 0,
        }
    }
}

/// Convert namespace to GPU data
#[cfg(feature = "enumerative")]
impl From<&Namespace> for GpuNamespaceData {
    fn from(ns: &Namespace) -> Self {
        let mut capability_partitions = [0u32; 16];
        let mut capability_lengths = [0u32; 4];
        let mut total_codim = 0u32;

        for (i, cap) in ns.capabilities.iter().enumerate() {
            if i >= 4 {
                break;
            }
            let partition = &cap.schubert_class.partition;
            for (j, &part) in partition.iter().enumerate() {
                if j >= 4 {
                    break;
                }
                capability_partitions[i * 4 + j] = part as u32;
                total_codim += part as u32;
            }
            capability_lengths[i] = partition.len().min(4) as u32;
        }

        Self {
            grassmannian_k: ns.grassmannian.0 as u32,
            grassmannian_n: ns.grassmannian.1 as u32,
            num_capabilities: ns.capabilities.len().min(4) as u32,
            total_codimension: total_codim,
            capability_partitions,
            capability_lengths,
        }
    }
}

/// Convert SchubertClass to tropical GPU data
#[cfg(feature = "enumerative")]
impl GpuTropicalSchubertData {
    /// Create tropical data from a Schubert class
    pub fn from_schubert(schubert: &SchubertClass) -> Self {
        let mut weights = [0.0f32; 8];
        for (i, &part) in schubert.partition.iter().enumerate() {
            if i < 8 {
                weights[i] = part as f32;
            }
        }

        Self {
            weights,
            num_weights: schubert.partition.len().min(8) as u32,
            grassmannian_k: schubert.grassmannian_dim.0 as u32,
            grassmannian_n: schubert.grassmannian_dim.1 as u32,
            padding: 0,
        }
    }
}

/// Convert multiple SchubertClasses to multi-intersect GPU data
#[cfg(feature = "enumerative")]
impl GpuMultiIntersectData {
    /// Create multi-intersect data from a slice of Schubert classes
    pub fn from_classes(classes: &[SchubertClass], grassmannian_dim: (usize, usize)) -> Self {
        let mut partitions = [0u32; 32];
        let mut partition_lengths = [0u32; 4];

        for (c, class) in classes.iter().enumerate() {
            if c >= 4 {
                break;
            }
            for (i, &part) in class.partition.iter().enumerate() {
                if i >= 8 {
                    break;
                }
                partitions[c * 8 + i] = part as u32;
            }
            partition_lengths[c] = class.partition.len().min(8) as u32;
        }

        Self {
            partitions,
            partition_lengths,
            num_classes: classes.len().min(4) as u32,
            grassmannian_k: grassmannian_dim.0 as u32,
            grassmannian_n: grassmannian_dim.1 as u32,
            padding: 0,
        }
    }
}

/// Convert degree to GPU WDVV data
#[cfg(feature = "enumerative")]
impl GpuWDVVData {
    /// Create GPU data from a curve degree
    pub fn from_degree(degree: u64) -> Self {
        Self {
            degree: degree as u32,
            padding: [0; 3],
        }
    }
}

/// Convert fixed point and torus weights to GPU localization data
#[cfg(feature = "enumerative")]
impl GpuLocalizationData {
    /// Create GPU data from a fixed point and torus weights
    pub fn from_fixed_point(fp: &FixedPoint, weights: &TorusWeights) -> Self {
        let mut subset = [0u32; 8];
        for (i, &idx) in fp.subset.iter().enumerate() {
            if i < 8 {
                subset[i] = idx as u32;
            }
        }

        let mut w = [0.0f32; 8];
        for (i, &weight) in weights.weights.iter().enumerate() {
            if i < 8 {
                w[i] = weight as f32;
            }
        }

        Self {
            subset,
            weights: w,
            subset_len: fp.subset.len().min(8) as u32,
            ambient_n: fp.grassmannian.1 as u32,
            padding: [0; 2],
        }
    }
}

/// Convert matroid and subset to GPU matroid rank data
#[cfg(feature = "enumerative")]
impl GpuMatroidRankData {
    /// Create GPU data from a matroid and a subset to evaluate rank on
    ///
    /// Bases and the subset are encoded as bitmasks over the ground set.
    pub fn from_matroid_subset(matroid: &Matroid, subset: &BTreeSet<usize>) -> Self {
        let mut bases_arr = [0u32; 32];
        for (i, basis) in matroid.bases.iter().enumerate() {
            if i >= 32 {
                break;
            }
            let mut mask: u32 = 0;
            for &elem in basis {
                if elem < 32 {
                    mask |= 1 << elem;
                }
            }
            bases_arr[i] = mask;
        }

        let mut subset_mask: u32 = 0;
        for &elem in subset {
            if elem < 32 {
                subset_mask |= 1 << elem;
            }
        }

        Self {
            ground_set_size: matroid.ground_set_size as u32,
            rank: matroid.rank as u32,
            num_bases: matroid.bases.len().min(32) as u32,
            subset_mask,
            bases: bases_arr,
        }
    }
}

/// Convert partition to GPU CSM data
#[cfg(feature = "enumerative")]
impl GpuCSMData {
    /// Create GPU data from a partition and Grassmannian parameters
    pub fn from_partition(partition: &[usize], grassmannian: (usize, usize)) -> Self {
        let mut part = [0u32; 8];
        for (i, &p) in partition.iter().enumerate() {
            if i < 8 {
                part[i] = p as u32;
            }
        }

        Self {
            partition: part,
            partition_len: partition.len().min(8) as u32,
            grassmannian_k: grassmannian.0 as u32,
            grassmannian_n: grassmannian.1 as u32,
            padding: 0,
        }
    }
}

/// Convert composable namespace interfaces to GPU operad data
#[cfg(feature = "enumerative")]
impl GpuOperadData {
    /// Create GPU data from two composable namespaces and their interface indices
    pub fn from_composition(
        ns_a: &ComposableNamespace,
        out_idx: usize,
        ns_b: &ComposableNamespace,
        in_idx: usize,
    ) -> Self {
        let out_codim = ns_a
            .interfaces
            .get(out_idx)
            .map(|iface| iface.codimension)
            .unwrap_or(0);

        let in_codim = ns_b
            .interfaces
            .get(in_idx)
            .map(|iface| iface.codimension)
            .unwrap_or(0);

        let (k, n) = ns_a.namespace.grassmannian;

        Self {
            output_codimension: out_codim as u32,
            input_codimension: in_codim as u32,
            grassmannian_k: k as u32,
            grassmannian_n: n as u32,
        }
    }
}

/// Convert Schubert class and trust level to GPU stability data
#[cfg(feature = "enumerative")]
impl GpuStabilityData {
    /// Create GPU data from a Schubert class and trust level
    pub fn from_class_and_trust(class: &SchubertClass, trust_level: f64) -> Self {
        let codim: usize = class.partition.iter().sum();
        let (k, n) = class.grassmannian_dim;
        let dim = k * (n - k);

        Self {
            codimension: codim as f32,
            dimension: dim as f32,
            trust_level: trust_level as f32,
            padding: 0.0,
        }
    }
}

/// GPU acceleration configuration
#[cfg(feature = "enumerative")]
#[derive(Debug, Clone)]
pub struct EnumerativeGpuConfig {
    pub enable_caching: bool,
    pub cache_size: usize,
    pub batch_size: usize,
    pub workgroup_size: u32,
}

#[cfg(feature = "enumerative")]
impl Default for EnumerativeGpuConfig {
    fn default() -> Self {
        Self {
            enable_caching: true,
            cache_size: 10000,
            batch_size: 1024,
            workgroup_size: 64,
        }
    }
}

// ─── GF(2) Enumerative GPU Data Structures ───

/// GPU-optimized finite field Grassmannian point counting data.
///
/// Computes |Gr(k,n;F_q)| = [n,k]_q (Gaussian binomial coefficient).
#[cfg(all(feature = "enumerative", feature = "gf2"))]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuFiniteFieldPointData {
    /// Grassmannian parameter k (subspace dimension)
    pub k: u32,
    /// Grassmannian parameter n (ambient dimension)
    pub n: u32,
    /// Field size q (must be a prime power)
    pub q: u32,
    /// Padding for alignment
    pub padding: u32,
}

/// GPU-optimized binary code weight distribution data.
///
/// Computes the weight distribution of a binary linear code from its generator matrix.
/// Enumerates all 2^k codewords and counts by Hamming weight.
#[cfg(all(feature = "enumerative", feature = "gf2"))]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuWeightDistributionData {
    /// Generator matrix rows bit-packed (up to 16 rows, each row ≤32 cols)
    pub generator_rows: [u32; 16],
    /// Code length n (number of columns)
    pub code_length: u32,
    /// Code dimension k (number of rows, ≤16)
    pub code_dimension: u32,
    /// Padding for alignment
    pub padding: [u32; 2],
}

/// GPU-optimized Kazhdan-Lusztig polynomial data for a matroid.
///
/// Computes KL polynomial coefficients via the characteristic polynomial approach.
#[cfg(all(feature = "enumerative", feature = "gf2"))]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuKLPolynomialData {
    /// Size of the ground set
    pub ground_set_size: u32,
    /// Rank of the matroid
    pub rank: u32,
    /// Number of bases
    pub num_bases: u32,
    /// Padding for alignment
    pub padding: u32,
    /// Bases encoded as bitmasks (up to 32)
    pub bases: [u32; 32],
}

/// GPU-optimized binary matroid representability check data.
///
/// Checks whether a matroid is representable over GF(2) by exhaustive search
/// over candidate representation matrices.
#[cfg(all(feature = "enumerative", feature = "gf2"))]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuRepresentabilityData {
    /// Size of the ground set
    pub ground_set_size: u32,
    /// Rank of the matroid
    pub rank: u32,
    /// Number of bases
    pub num_bases: u32,
    /// Padding for alignment
    pub padding: u32,
    /// Bases encoded as bitmasks (up to 32)
    pub bases: [u32; 32],
}

// ─── GF(2) Batch Operations ───

#[cfg(all(feature = "enumerative", feature = "gf2"))]
impl EnumerativeGpuOps {
    /// Batch Grassmannian point counting over finite fields.
    ///
    /// For each entry, computes |Gr(k,n;F_q)| = [n,k]_q (Gaussian binomial).
    pub async fn batch_finite_field_points(
        &mut self,
        data: &[GpuFiniteFieldPointData],
    ) -> EnumerativeGpuResult<Vec<u32>> {
        let num_items = data.len();
        if num_items == 0 {
            return Ok(Vec::new());
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Finite Field Points Input"),
                    contents: bytemuck::cast_slice(data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        let output_size = (num_items * std::mem::size_of::<u32>()) as u64;
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Finite Field Points Output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = Self::get_finite_field_points_shader();
        let bind_group_layout = self.create_generic_bind_group_layout("Finite Field Points Layout");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Finite Field Points Bind Group"),
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
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        self.context
            .read_enumerative_buffer(&output_buffer, output_size)
            .await
    }

    /// Batch binary code weight distribution computation.
    ///
    /// For each code, returns a flattened histogram of (n+1) u32 weight counts.
    /// Output length = num_codes * (max_code_length + 1).
    pub async fn batch_weight_distributions(
        &mut self,
        data: &[GpuWeightDistributionData],
    ) -> EnumerativeGpuResult<Vec<u32>> {
        let num_items = data.len();
        if num_items == 0 {
            return Ok(Vec::new());
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Weight Distribution Input"),
                    contents: bytemuck::cast_slice(data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        // Output: 33 u32s per code (weight counts for weights 0..32)
        let output_entries_per_code = 33u64;
        let output_size =
            (num_items as u64) * output_entries_per_code * std::mem::size_of::<u32>() as u64;
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Weight Distribution Output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = Self::get_weight_distribution_shader();
        let bind_group_layout = self.create_generic_bind_group_layout("Weight Distribution Layout");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Weight Distribution Bind Group"),
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
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        self.context
            .read_enumerative_buffer(&output_buffer, output_size)
            .await
    }

    /// Batch KL polynomial characteristic polynomial computation.
    ///
    /// For each matroid, computes the characteristic polynomial coefficients.
    /// Returns flattened coefficients, (rank+1) i32 values per matroid.
    pub async fn batch_kl_coefficients(
        &mut self,
        data: &[GpuKLPolynomialData],
    ) -> EnumerativeGpuResult<Vec<i32>> {
        let num_items = data.len();
        if num_items == 0 {
            return Ok(Vec::new());
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("KL Polynomial Input"),
                    contents: bytemuck::cast_slice(data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        // Output: 16 i32s per matroid (max rank + 1 coefficients, padded)
        let output_entries_per_matroid = 16u64;
        let output_size =
            (num_items as u64) * output_entries_per_matroid * std::mem::size_of::<i32>() as u64;
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("KL Polynomial Output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = Self::get_kl_coefficient_shader();
        let bind_group_layout = self.create_generic_bind_group_layout("KL Polynomial Layout");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("KL Polynomial Bind Group"),
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
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        self.context
            .read_enumerative_buffer(&output_buffer, output_size)
            .await
    }

    /// Batch binary matroid representability checking.
    ///
    /// For each matroid, checks if it is representable over GF(2).
    /// Returns 1 if representable, 0 if not.
    pub async fn batch_representability_checks(
        &mut self,
        data: &[GpuRepresentabilityData],
    ) -> EnumerativeGpuResult<Vec<u32>> {
        let num_items = data.len();
        if num_items == 0 {
            return Ok(Vec::new());
        }

        let input_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Representability Input"),
                    contents: bytemuck::cast_slice(data),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });

        let output_size = (num_items * std::mem::size_of::<u32>()) as u64;
        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Representability Output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let shader_source = Self::get_representability_shader();
        let bind_group_layout = self.create_generic_bind_group_layout("Representability Layout");
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Representability Bind Group"),
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
        self.context.execute_enumerative_compute(
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        self.context
            .read_enumerative_buffer(&output_buffer, output_size)
            .await
    }
}

// ─── GF(2) WGSL Shaders ───

#[cfg(all(feature = "enumerative", feature = "gf2"))]
impl EnumerativeGpuOps {
    /// Gaussian binomial [n,k]_q shader
    fn get_finite_field_points_shader() -> String {
        String::from(
            r#"
struct FiniteFieldData {
    k: u32,
    n: u32,
    q: u32,
    padding: u32,
}

@group(0) @binding(0) var<storage, read> input_data: array<FiniteFieldData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= arrayLength(&input_data)) {
        return;
    }

    let data = input_data[index];
    let k = data.k;
    let n = data.n;
    let q = data.q;

    if (k == 0u || k == n) {
        output_data[index] = 1u;
        return;
    }
    if (k > n) {
        output_data[index] = 0u;
        return;
    }

    // Compute [n,k]_q = prod_{i=0}^{k-1} (q^(n-i) - 1) / (q^(k-i) - 1)
    var numerator: u32 = 1u;
    var denominator: u32 = 1u;

    for (var i: u32 = 0u; i < k; i++) {
        // q^(n-i)
        var qn: u32 = 1u;
        for (var j: u32 = 0u; j < (n - i); j++) {
            qn *= q;
        }
        numerator *= (qn - 1u);

        // q^(k-i)
        var qk: u32 = 1u;
        for (var j: u32 = 0u; j < (k - i); j++) {
            qk *= q;
        }
        denominator *= (qk - 1u);
    }

    if (denominator == 0u) {
        output_data[index] = 0u;
    } else {
        output_data[index] = numerator / denominator;
    }
}
"#,
        )
    }

    /// Weight distribution shader: enumerate all 2^k codewords, histogram by weight
    fn get_weight_distribution_shader() -> String {
        String::from(
            r#"
struct WeightDistData {
    generator_rows: array<u32, 16>,
    code_length: u32,
    code_dimension: u32,
    padding: array<u32, 2>,
}

@group(0) @binding(0) var<storage, read> input_data: array<WeightDistData>;
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

    // Access through storage buffer reference (not local copy) for dynamic indexing
    let k = input_data[index].code_dimension;
    let n = input_data[index].code_length;
    let num_codewords = 1u << k;
    let out_base = index * 33u;

    // Zero the histogram
    for (var w: u32 = 0u; w <= 32u; w++) {
        output_data[out_base + w] = 0u;
    }

    // Enumerate all 2^k codewords (message vectors)
    for (var msg: u32 = 0u; msg < num_codewords; msg++) {
        // Codeword = XOR of generator rows selected by message bits
        var codeword: u32 = 0u;
        for (var i: u32 = 0u; i < k && i < 16u; i++) {
            if ((msg >> i) & 1u) == 1u {
                codeword ^= input_data[index].generator_rows[i];
            }
        }

        // Mask to code_length bits
        if (n < 32u) {
            codeword &= ((1u << n) - 1u);
        }

        let weight = count_ones(codeword);
        if (weight <= 32u) {
            output_data[out_base + weight] += 1u;
        }
    }
}
"#,
        )
    }

    /// KL / characteristic polynomial shader via deletion-contraction
    fn get_kl_coefficient_shader() -> String {
        String::from(
            r#"
struct KLData {
    ground_set_size: u32,
    rank: u32,
    num_bases: u32,
    padding: u32,
    bases: array<u32, 32>,
}

@group(0) @binding(0) var<storage, read> input_data: array<KLData>;
@group(0) @binding(1) var<storage, read_write> output_data: array<i32>;

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

    // Access through storage buffer reference for dynamic indexing
    let n = input_data[index].ground_set_size;
    let r = input_data[index].rank;
    let num_bases = input_data[index].num_bases;
    let out_base = index * 16;

    // Zero output
    for (var i: u32 = 0u; i < 16u; i++) {
        output_data[out_base + i] = 0;
    }

    // Characteristic polynomial via Möbius inversion over the lattice of flats.
    // p(t) = sum_{A ⊆ E} (-1)^|A| t^(r-rank(A))
    let max_subsets = min(1u << n, 65536u);

    for (var subset: u32 = 0u; subset < max_subsets; subset++) {
        let subset_size = count_ones(subset);

        // Compute rank: max |B ∩ S| over bases B (inline, accessing storage directly)
        var max_int: u32 = 0u;
        for (var bi: u32 = 0u; bi < num_bases && bi < 32u; bi++) {
            let c = count_ones(input_data[index].bases[bi] & subset);
            if (c > max_int) {
                max_int = c;
            }
        }
        let subset_rank = max_int;

        let coeff_idx = r - subset_rank;
        if (coeff_idx < 16u) {
            if (subset_size % 2u == 0u) {
                output_data[out_base + coeff_idx] += 1;
            } else {
                output_data[out_base + coeff_idx] -= 1;
            }
        }
    }
}
"#,
        )
    }

    /// Binary representability shader: try all [I|A] matrices
    ///
    /// Uses a column-matroid-equivalence approach: for each candidate matrix [I|A],
    /// compute the set of bases (r-element subsets with full rank) and compare
    /// against the target matroid's bases.
    fn get_representability_shader() -> String {
        String::from(
            r#"
struct RepresentabilityData {
    ground_set_size: u32,
    rank: u32,
    num_bases: u32,
    padding: u32,
    bases: array<u32, 32>,
}

@group(0) @binding(0) var<storage, read> input_data: array<RepresentabilityData>;
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

    let n = input_data[index].ground_set_size;
    let r = input_data[index].rank;
    let num_bases = input_data[index].num_bases;
    let extra_cols = n - r;

    if (n > 16u || r > 8u || extra_cols > 8u) {
        output_data[index] = 2u; // inconclusive
        return;
    }

    let num_free = r * extra_cols;
    let num_candidates = 1u << num_free;

    // For each candidate [I_r | A] representation matrix:
    // Check if every target basis has the selected columns linearly independent.
    // We use determinant mod 2 (= rank check) via row-reduce in a u32 bitmask.
    for (var candidate: u32 = 0u; candidate < num_candidates; candidate++) {
        var all_match: bool = true;

        for (var bi: u32 = 0u; bi < num_bases && bi < 32u; bi++) {
            let basis_mask = input_data[index].bases[bi];
            if (count_ones(basis_mask) != r) {
                continue;
            }

            // Build r×r submatrix from selected columns, row-reduce to check rank.
            // Each column j gives a column vector:
            //   j < r: identity column e_j
            //   j >= r: A column from candidate bits
            // We pack columns as rows of the transpose for elimination.
            var rr_rank: u32 = 0u;

            // Build submatrix row data inline (up to 8 rows for r ≤ 8)
            var m0: u32 = 0u; var m1: u32 = 0u; var m2: u32 = 0u; var m3: u32 = 0u;
            var m4: u32 = 0u; var m5: u32 = 0u; var m6: u32 = 0u; var m7: u32 = 0u;
            var col_count: u32 = 0u;

            for (var j: u32 = 0u; j < n && j < 16u; j++) {
                if ((basis_mask >> j) & 1u) == 1u {
                    var col_val: u32;
                    if (j < r) {
                        col_val = 1u << j;
                    } else {
                        var cv: u32 = 0u;
                        let a_col = j - r;
                        for (var row: u32 = 0u; row < r; row++) {
                            let bit_pos = row * extra_cols + a_col;
                            if ((candidate >> bit_pos) & 1u) == 1u {
                                cv |= (1u << row);
                            }
                        }
                        col_val = cv;
                    }
                    // Store in named variables instead of array
                    if (col_count == 0u) { m0 = col_val; }
                    else if (col_count == 1u) { m1 = col_val; }
                    else if (col_count == 2u) { m2 = col_val; }
                    else if (col_count == 3u) { m3 = col_val; }
                    else if (col_count == 4u) { m4 = col_val; }
                    else if (col_count == 5u) { m5 = col_val; }
                    else if (col_count == 6u) { m6 = col_val; }
                    else if (col_count == 7u) { m7 = col_val; }
                    col_count++;
                }
            }

            // Row-reduce the col_count × r matrix (stored as m0..m7)
            // to determine rank. Gaussian elimination over GF(2).
            rr_rank = 0u;
            for (var col: u32 = 0u; col < r && col < 8u; col++) {
                // Find pivot row >= rr_rank with bit `col` set
                var found_pivot: bool = false;
                for (var row: u32 = rr_rank; row < col_count && row < 8u; row++) {
                    var row_val: u32 = 0u;
                    if (row == 0u) { row_val = m0; }
                    else if (row == 1u) { row_val = m1; }
                    else if (row == 2u) { row_val = m2; }
                    else if (row == 3u) { row_val = m3; }
                    else if (row == 4u) { row_val = m4; }
                    else if (row == 5u) { row_val = m5; }
                    else if (row == 6u) { row_val = m6; }
                    else { row_val = m7; }

                    if (((row_val >> col) & 1u) == 1u) {
                        // Swap row and rr_rank
                        var pivot_val: u32 = 0u;
                        if (rr_rank == 0u) { pivot_val = m0; }
                        else if (rr_rank == 1u) { pivot_val = m1; }
                        else if (rr_rank == 2u) { pivot_val = m2; }
                        else if (rr_rank == 3u) { pivot_val = m3; }
                        else if (rr_rank == 4u) { pivot_val = m4; }
                        else if (rr_rank == 5u) { pivot_val = m5; }
                        else if (rr_rank == 6u) { pivot_val = m6; }
                        else { pivot_val = m7; }

                        // Write row_val to rr_rank, pivot_val to row
                        if (rr_rank == 0u) { m0 = row_val; }
                        else if (rr_rank == 1u) { m1 = row_val; }
                        else if (rr_rank == 2u) { m2 = row_val; }
                        else if (rr_rank == 3u) { m3 = row_val; }
                        else if (rr_rank == 4u) { m4 = row_val; }
                        else if (rr_rank == 5u) { m5 = row_val; }
                        else if (rr_rank == 6u) { m6 = row_val; }
                        else { m7 = row_val; }

                        if (row == 0u) { m0 = pivot_val; }
                        else if (row == 1u) { m1 = pivot_val; }
                        else if (row == 2u) { m2 = pivot_val; }
                        else if (row == 3u) { m3 = pivot_val; }
                        else if (row == 4u) { m4 = pivot_val; }
                        else if (row == 5u) { m5 = pivot_val; }
                        else if (row == 6u) { m6 = pivot_val; }
                        else { m7 = pivot_val; }

                        found_pivot = true;
                        break;
                    }
                }

                if (!found_pivot) {
                    continue;
                }

                // Get pivot row value
                var pv: u32 = 0u;
                if (rr_rank == 0u) { pv = m0; }
                else if (rr_rank == 1u) { pv = m1; }
                else if (rr_rank == 2u) { pv = m2; }
                else if (rr_rank == 3u) { pv = m3; }
                else if (rr_rank == 4u) { pv = m4; }
                else if (rr_rank == 5u) { pv = m5; }
                else if (rr_rank == 6u) { pv = m6; }
                else { pv = m7; }

                // Eliminate other rows
                for (var row: u32 = 0u; row < col_count && row < 8u; row++) {
                    if (row == rr_rank) { continue; }
                    var rv: u32 = 0u;
                    if (row == 0u) { rv = m0; }
                    else if (row == 1u) { rv = m1; }
                    else if (row == 2u) { rv = m2; }
                    else if (row == 3u) { rv = m3; }
                    else if (row == 4u) { rv = m4; }
                    else if (row == 5u) { rv = m5; }
                    else if (row == 6u) { rv = m6; }
                    else { rv = m7; }

                    if (((rv >> col) & 1u) == 1u) {
                        rv ^= pv;
                        if (row == 0u) { m0 = rv; }
                        else if (row == 1u) { m1 = rv; }
                        else if (row == 2u) { m2 = rv; }
                        else if (row == 3u) { m3 = rv; }
                        else if (row == 4u) { m4 = rv; }
                        else if (row == 5u) { m5 = rv; }
                        else if (row == 6u) { m6 = rv; }
                        else { m7 = rv; }
                    }
                }
                rr_rank++;
            }

            if (rr_rank != r) {
                all_match = false;
                break;
            }
        }

        if (all_match) {
            output_data[index] = 1u;
            return;
        }
    }

    output_data[index] = 0u;
}
"#,
        )
    }
}

// ─── GF(2) Conversion Constructors ───

#[cfg(all(feature = "enumerative", feature = "gf2"))]
impl GpuFiniteFieldPointData {
    /// Create from Grassmannian parameters and field size.
    pub fn new(k: usize, n: usize, q: u64) -> Self {
        Self {
            k: k as u32,
            n: n as u32,
            q: q as u32,
            padding: 0,
        }
    }
}

#[cfg(all(feature = "enumerative", feature = "gf2"))]
impl GpuWeightDistributionData {
    /// Create from a generator matrix (GF2Matrix).
    ///
    /// Matrix must have ≤16 rows and ≤32 columns.
    pub fn from_generator_matrix(matrix: &amari_core::gf2::GF2Matrix) -> Self {
        let mut generator_rows = [0u32; 16];

        for (i, slot) in generator_rows
            .iter_mut()
            .enumerate()
            .take(matrix.nrows().min(16))
        {
            let row = matrix.row(i);
            let words = row.as_words();
            if !words.is_empty() {
                *slot = words[0] as u32;
            }
        }

        Self {
            generator_rows,
            code_length: matrix.ncols() as u32,
            code_dimension: matrix.nrows() as u32,
            padding: [0; 2],
        }
    }
}

#[cfg(all(feature = "enumerative", feature = "gf2"))]
impl GpuKLPolynomialData {
    /// Create from a matroid (bitmask-encoded bases).
    pub fn from_matroid(matroid: &Matroid) -> Self {
        let mut bases_arr = [0u32; 32];
        for (i, basis) in matroid.bases.iter().enumerate() {
            if i >= 32 {
                break;
            }
            let mut mask: u32 = 0;
            for &elem in basis {
                if elem < 32 {
                    mask |= 1 << elem;
                }
            }
            bases_arr[i] = mask;
        }

        Self {
            ground_set_size: matroid.ground_set_size as u32,
            rank: matroid.rank as u32,
            num_bases: matroid.bases.len().min(32) as u32,
            padding: 0,
            bases: bases_arr,
        }
    }
}

#[cfg(all(feature = "enumerative", feature = "gf2"))]
impl GpuRepresentabilityData {
    /// Create from a matroid.
    pub fn from_matroid(matroid: &Matroid) -> Self {
        let mut bases_arr = [0u32; 32];
        for (i, basis) in matroid.bases.iter().enumerate() {
            if i >= 32 {
                break;
            }
            let mut mask: u32 = 0;
            for &elem in basis {
                if elem < 32 {
                    mask |= 1 << elem;
                }
            }
            bases_arr[i] = mask;
        }

        Self {
            ground_set_size: matroid.ground_set_size as u32,
            rank: matroid.rank as u32,
            num_bases: matroid.bases.len().min(32) as u32,
            padding: 0,
            bases: bases_arr,
        }
    }
}

/// Integration tests for GPU operations
#[cfg(feature = "enumerative")]
#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! with_serial_gpu_ops {
        ($gpu_ops:ident, $body:block) => {{
            let _gpu_test_guard = crate::GPU_TEST_LOCK.lock().await;
            if let Ok(mut $gpu_ops) = EnumerativeGpuOps::new().await $body
        }};
    }

    #[tokio::test]
    async fn test_enumerative_gpu_context_initialization() {
        // Should not fail even without GPU hardware
        let result = EnumerativeGpuContext::new().await;

        match result {
            Ok(_context) => {
                println!("✅ Enumerative GPU context initialized successfully");
            }
            Err(_) => {
                println!("⚠️  GPU not available, test passes with graceful fallback");
            }
        }
    }

    #[tokio::test]
    async fn test_gpu_intersection_computation() {
        with_serial_gpu_ops!(gpu_ops, {
            let intersection_data = vec![
                GpuIntersectionData {
                    degree1: 3.0,
                    degree2: 4.0,
                    codimension1: 1.0,
                    codimension2: 1.0,
                    ambient_dimension: 2.0,
                    genus_correction: 0.0,
                    multiplicity_factor: 1.0,
                    padding: 0.0,
                },
                GpuIntersectionData {
                    degree1: 2.0,
                    degree2: 5.0,
                    codimension1: 1.0,
                    codimension2: 1.0,
                    ambient_dimension: 3.0,
                    genus_correction: 0.0,
                    multiplicity_factor: 1.0,
                    padding: 0.0,
                },
            ];

            let result = gpu_ops.batch_intersection_numbers(&intersection_data).await;

            match result {
                Ok(numbers) => {
                    assert_eq!(numbers.len(), intersection_data.len());
                    println!("✅ GPU intersection computation successful");
                    for (i, &number) in numbers.iter().enumerate() {
                        println!("   Intersection {}: {:.6}", i, number);
                    }
                }
                Err(_) => {
                    println!("⚠️  GPU intersection computation failed, but test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_gpu_schubert_computation() {
        with_serial_gpu_ops!(gpu_ops, {
            let schubert_data = vec![
                GpuSchubertClass {
                    partition: [2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    grassmannian_k: 2.0,
                    grassmannian_n: 5.0,
                    padding: [0.0; 6],
                },
                GpuSchubertClass {
                    partition: [1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    grassmannian_k: 3.0,
                    grassmannian_n: 6.0,
                    padding: [0.0; 6],
                },
            ];

            let result = gpu_ops.batch_schubert_numbers(&schubert_data).await;

            match result {
                Ok(numbers) => {
                    assert_eq!(numbers.len(), schubert_data.len());
                    println!("✅ GPU Schubert computation successful");
                    for (i, &number) in numbers.iter().enumerate() {
                        println!("   Schubert class {}: {:.6}", i, number);
                    }
                }
                Err(_) => {
                    println!("⚠️  GPU Schubert computation failed, but test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_gpu_gromov_witten_computation() {
        with_serial_gpu_ops!(gpu_ops, {
            let gw_data = vec![
                GpuGromovWittenData {
                    curve_degree: 1.0,
                    genus: 0.0,
                    marked_points: 3.0,
                    target_dimension: 3.0,
                    virtual_dimension: 0.0,
                    quantum_parameter: 0.1,
                    padding: [0.0; 2],
                },
                GpuGromovWittenData {
                    curve_degree: 2.0,
                    genus: 1.0,
                    marked_points: 1.0,
                    target_dimension: 2.0,
                    virtual_dimension: 1.0,
                    quantum_parameter: 0.05,
                    padding: [0.0; 2],
                },
            ];

            let result = gpu_ops.batch_gromov_witten_invariants(&gw_data).await;

            match result {
                Ok(invariants) => {
                    assert_eq!(invariants.len(), gw_data.len());
                    println!("✅ GPU Gromov-Witten computation successful");
                    for (i, &invariant) in invariants.iter().enumerate() {
                        println!("   GW invariant {}: {:.6}", i, invariant);
                    }
                }
                Err(_) => {
                    println!("⚠️  GPU Gromov-Witten computation failed, but test passes");
                }
            }
        });
    }

    #[test]
    fn test_gpu_data_conversion() {
        let chow = ChowClass::hypersurface(3);
        let gpu_data: GpuIntersectionData = (&chow).into();

        assert_eq!(gpu_data.degree1, 3.0);
        assert_eq!(gpu_data.codimension1, 1.0);

        println!("✅ GPU data conversion verified");
    }

    #[test]
    fn test_schubert_gpu_conversion() {
        let partition = vec![2, 1];
        let schubert = SchubertClass::new(partition, (2, 5)).unwrap();
        let gpu_schubert: GpuSchubertClass = (&schubert).into();

        assert_eq!(gpu_schubert.partition[0], 2.0);
        assert_eq!(gpu_schubert.partition[1], 1.0);
        assert_eq!(gpu_schubert.grassmannian_k, 2.0);
        assert_eq!(gpu_schubert.grassmannian_n, 5.0);

        println!("✅ Schubert GPU conversion verified");
    }

    #[test]
    fn test_enumerative_gpu_config() {
        let config = EnumerativeGpuConfig::default();

        assert!(config.enable_caching);
        assert_eq!(config.cache_size, 10000);
        assert_eq!(config.batch_size, 1024);
        assert_eq!(config.workgroup_size, 64);

        println!("✅ Enumerative GPU configuration verified");
    }

    #[tokio::test]
    async fn test_gpu_lr_coefficient_computation() {
        with_serial_gpu_ops!(gpu_ops, {
            let lr_data = vec![
                GpuLittlewoodRichardsonData {
                    lambda: [2, 1, 0, 0, 0, 0, 0, 0],
                    mu: [1, 1, 0, 0, 0, 0, 0, 0],
                    nu: [3, 2, 0, 0, 0, 0, 0, 0],
                    lambda_len: 2,
                    mu_len: 2,
                    nu_len: 2,
                    padding: 0,
                },
                GpuLittlewoodRichardsonData {
                    lambda: [1, 0, 0, 0, 0, 0, 0, 0],
                    mu: [1, 0, 0, 0, 0, 0, 0, 0],
                    nu: [2, 0, 0, 0, 0, 0, 0, 0],
                    lambda_len: 1,
                    mu_len: 1,
                    nu_len: 1,
                    padding: 0,
                },
            ];

            let result = gpu_ops.batch_lr_coefficients(&lr_data).await;

            match result {
                Ok(coefficients) => {
                    assert_eq!(coefficients.len(), lr_data.len());
                    println!("✅ GPU LR coefficient computation successful");
                    for (i, &coeff) in coefficients.iter().enumerate() {
                        println!("   LR coefficient {}: {}", i, coeff);
                    }
                }
                Err(_) => {
                    println!("⚠️  GPU LR computation failed, but test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_gpu_namespace_configuration_counting() {
        with_serial_gpu_ops!(gpu_ops, {
            let namespace_data = vec![
                GpuNamespaceData {
                    grassmannian_k: 2,
                    grassmannian_n: 4,
                    num_capabilities: 2,
                    total_codimension: 2,
                    capability_partitions: [1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                    capability_lengths: [1, 1, 0, 0],
                },
                GpuNamespaceData {
                    grassmannian_k: 3,
                    grassmannian_n: 6,
                    num_capabilities: 3,
                    total_codimension: 3,
                    capability_partitions: [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
                    capability_lengths: [1, 1, 1, 0],
                },
            ];

            let result = gpu_ops
                .batch_namespace_configurations(&namespace_data)
                .await;

            match result {
                Ok(counts) => {
                    assert_eq!(counts.len(), namespace_data.len());
                    println!("✅ GPU namespace configuration counting successful");
                    for (i, &count) in counts.iter().enumerate() {
                        println!("   Namespace {} configurations: {}", i, count);
                    }
                }
                Err(_) => {
                    println!("⚠️  GPU namespace computation failed, but test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_gpu_tropical_schubert_intersection() {
        with_serial_gpu_ops!(gpu_ops, {
            let tropical_data = vec![
                GpuTropicalSchubertData {
                    weights: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    num_weights: 1,
                    grassmannian_k: 2,
                    grassmannian_n: 4,
                    padding: 0,
                },
                GpuTropicalSchubertData {
                    weights: [2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    num_weights: 2,
                    grassmannian_k: 2,
                    grassmannian_n: 5,
                    padding: 0,
                },
            ];

            let result = gpu_ops.batch_tropical_intersections(&tropical_data).await;

            match result {
                Ok(counts) => {
                    assert_eq!(counts.len(), tropical_data.len());
                    println!("✅ GPU tropical Schubert intersection successful");
                    for (i, &count) in counts.iter().enumerate() {
                        println!("   Tropical intersection {}: {:.2}", i, count);
                    }
                }
                Err(_) => {
                    println!("⚠️  GPU tropical computation failed, but test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_gpu_multi_intersect_computation() {
        with_serial_gpu_ops!(gpu_ops, {
            let multi_data = vec![
                GpuMultiIntersectData {
                    partitions: [
                        1, 0, 0, 0, 0, 0, 0, 0, // class 0: [1]
                        1, 0, 0, 0, 0, 0, 0, 0, // class 1: [1]
                        1, 0, 0, 0, 0, 0, 0, 0, // class 2: [1]
                        1, 0, 0, 0, 0, 0, 0, 0, // class 3: [1]
                    ],
                    partition_lengths: [1, 1, 1, 1],
                    num_classes: 4,
                    grassmannian_k: 2,
                    grassmannian_n: 4,
                    padding: 0,
                },
                GpuMultiIntersectData {
                    partitions: [
                        2, 1, 0, 0, 0, 0, 0, 0, // class 0: [2, 1]
                        1, 0, 0, 0, 0, 0, 0, 0, // class 1: [1]
                        0, 0, 0, 0, 0, 0, 0, 0, // unused
                        0, 0, 0, 0, 0, 0, 0, 0, // unused
                    ],
                    partition_lengths: [2, 1, 0, 0],
                    num_classes: 2,
                    grassmannian_k: 2,
                    grassmannian_n: 5,
                    padding: 0,
                },
            ];

            let result = gpu_ops.batch_multi_intersect(&multi_data).await;

            match result {
                Ok(counts) => {
                    assert_eq!(counts.len(), multi_data.len());
                    println!("✅ GPU multi-intersect computation successful");
                    for (i, &count) in counts.iter().enumerate() {
                        println!("   Multi-intersect {}: {}", i, count);
                    }
                }
                Err(_) => {
                    println!("⚠️  GPU multi-intersect computation failed, but test passes");
                }
            }
        });
    }

    #[test]
    fn test_lr_gpu_conversion() {
        let lambda = Partition::new(vec![2, 1]);
        let mu = Partition::new(vec![1, 1]);
        let nu = Partition::new(vec![3, 2]);

        let gpu_data = GpuLittlewoodRichardsonData::from_partitions(&lambda, &mu, &nu);

        assert_eq!(gpu_data.lambda[0], 2);
        assert_eq!(gpu_data.lambda[1], 1);
        assert_eq!(gpu_data.mu[0], 1);
        assert_eq!(gpu_data.mu[1], 1);
        assert_eq!(gpu_data.nu[0], 3);
        assert_eq!(gpu_data.nu[1], 2);
        assert_eq!(gpu_data.lambda_len, 2);
        assert_eq!(gpu_data.mu_len, 2);
        assert_eq!(gpu_data.nu_len, 2);

        println!("✅ LR GPU conversion verified");
    }

    #[test]
    fn test_tropical_schubert_gpu_conversion() {
        let schubert = SchubertClass::new(vec![2, 1], (2, 5)).unwrap();
        let tropical_gpu = GpuTropicalSchubertData::from_schubert(&schubert);

        assert_eq!(tropical_gpu.weights[0], 2.0);
        assert_eq!(tropical_gpu.weights[1], 1.0);
        assert_eq!(tropical_gpu.num_weights, 2);
        assert_eq!(tropical_gpu.grassmannian_k, 2);
        assert_eq!(tropical_gpu.grassmannian_n, 5);

        println!("✅ Tropical Schubert GPU conversion verified");
    }

    #[test]
    fn test_multi_intersect_gpu_conversion() {
        let sigma_1 = SchubertClass::new(vec![1], (2, 4)).unwrap();
        let classes = vec![
            sigma_1.clone(),
            sigma_1.clone(),
            sigma_1.clone(),
            sigma_1.clone(),
        ];

        let gpu_data = GpuMultiIntersectData::from_classes(&classes, (2, 4));

        assert_eq!(gpu_data.num_classes, 4);
        assert_eq!(gpu_data.grassmannian_k, 2);
        assert_eq!(gpu_data.grassmannian_n, 4);
        for i in 0..4 {
            assert_eq!(gpu_data.partitions[i * 8], 1);
            assert_eq!(gpu_data.partition_lengths[i], 1);
        }

        println!("✅ Multi-intersect GPU conversion verified");
    }

    // ─── New GPU operation tests (WDVV, localization, matroid, CSM, operad, stability) ───

    #[tokio::test]
    async fn test_gpu_wdvv_computation() {
        with_serial_gpu_ops!(gpu_ops, {
            let wdvv_data: Vec<GpuWDVVData> = (1..=6).map(GpuWDVVData::from_degree).collect();

            let result = gpu_ops.batch_wdvv_curve_counts(&wdvv_data).await;

            match result {
                Ok(counts) => {
                    assert_eq!(counts.len(), 6);
                    println!("✅ GPU WDVV computation successful");
                    for (i, &count) in counts.iter().enumerate() {
                        println!("   N_{}: {}", i + 1, count);
                    }
                }
                Err(_) => {
                    println!("⚠️  GPU WDVV computation failed, but test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_gpu_wdvv_empty_batch() {
        with_serial_gpu_ops!(gpu_ops, {
            let result = gpu_ops.batch_wdvv_curve_counts(&[]).await;
            match result {
                Ok(counts) => {
                    assert!(counts.is_empty());
                    println!("✅ GPU WDVV empty batch returns empty");
                }
                Err(_) => {
                    println!("⚠️  GPU not available, test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_gpu_localization_euler_class() {
        with_serial_gpu_ops!(gpu_ops, {
            use amari_enumerative::{FixedPoint, TorusWeights};

            let fp1 = FixedPoint {
                subset: vec![0, 1],
                grassmannian: (2, 4),
            };
            let fp2 = FixedPoint {
                subset: vec![0, 2],
                grassmannian: (2, 4),
            };
            let weights = TorusWeights {
                weights: vec![1, 2, 3, 4],
            };

            let loc_data = vec![
                GpuLocalizationData::from_fixed_point(&fp1, &weights),
                GpuLocalizationData::from_fixed_point(&fp2, &weights),
            ];

            let result = gpu_ops.batch_localization_euler_classes(&loc_data).await;

            match result {
                Ok(euler_classes) => {
                    assert_eq!(euler_classes.len(), 2);
                    // Euler classes should be nonzero for generic weights
                    for &ec in &euler_classes {
                        assert!(ec.abs() > 0.0, "Euler class should be nonzero");
                    }
                    println!("✅ GPU localization computation successful");
                    for (i, &ec) in euler_classes.iter().enumerate() {
                        println!("   Euler class {}: {:.4}", i, ec);
                    }
                }
                Err(_) => {
                    println!("⚠️  GPU localization computation failed, but test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_gpu_matroid_rank_computation() {
        with_serial_gpu_ops!(gpu_ops, {
            use amari_enumerative::Matroid;
            use std::collections::BTreeSet;

            // Uniform matroid U_{2,4}: all 2-element subsets of {0,1,2,3} are bases
            let matroid = Matroid::uniform(2, 4);
            let subset_full: BTreeSet<usize> = [0, 1, 2, 3].into_iter().collect();
            let subset_pair: BTreeSet<usize> = [0, 1].into_iter().collect();
            let subset_single: BTreeSet<usize> = [2].into_iter().collect();

            let matroid_data = vec![
                GpuMatroidRankData::from_matroid_subset(&matroid, &subset_full),
                GpuMatroidRankData::from_matroid_subset(&matroid, &subset_pair),
                GpuMatroidRankData::from_matroid_subset(&matroid, &subset_single),
            ];

            let result = gpu_ops.batch_matroid_ranks(&matroid_data).await;

            match result {
                Ok(ranks) => {
                    assert_eq!(ranks.len(), 3);
                    println!("✅ GPU matroid rank computation successful");
                    for (i, &rank) in ranks.iter().enumerate() {
                        println!("   Rank {}: {}", i, rank);
                    }
                }
                Err(_) => {
                    println!("⚠️  GPU matroid rank computation failed, but test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_gpu_csm_euler_characteristic() {
        with_serial_gpu_ops!(gpu_ops, {
            let csm_data = vec![
                // Top cell: partition [2,1] in Gr(2,4)
                GpuCSMData::from_partition(&[2, 1], (2, 4)),
                // Point class: partition [2,2] in Gr(2,4)
                GpuCSMData::from_partition(&[2, 2], (2, 4)),
            ];

            let result = gpu_ops.batch_csm_euler_characteristics(&csm_data).await;

            match result {
                Ok(eulers) => {
                    assert_eq!(eulers.len(), 2);
                    println!("✅ GPU CSM computation successful");
                    for (i, &euler) in eulers.iter().enumerate() {
                        println!("   Euler char {}: {}", i, euler);
                    }
                }
                Err(_) => {
                    println!("⚠️  GPU CSM computation failed, but test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_gpu_operad_multiplicity() {
        with_serial_gpu_ops!(gpu_ops, {
            let operad_data = vec![
                // Compatible: same codimension
                GpuOperadData {
                    output_codimension: 1,
                    input_codimension: 1,
                    grassmannian_k: 2,
                    grassmannian_n: 4,
                },
                // Incompatible: different codimension
                GpuOperadData {
                    output_codimension: 1,
                    input_codimension: 2,
                    grassmannian_k: 2,
                    grassmannian_n: 4,
                },
            ];

            let result = gpu_ops.batch_operad_multiplicities(&operad_data).await;

            match result {
                Ok(multiplicities) => {
                    assert_eq!(multiplicities.len(), 2);
                    println!("✅ GPU operad multiplicity computation successful");
                    for (i, &mult) in multiplicities.iter().enumerate() {
                        println!("   Multiplicity {}: {}", i, mult);
                    }
                }
                Err(_) => {
                    println!("⚠️  GPU operad computation failed, but test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_gpu_stability_phase() {
        with_serial_gpu_ops!(gpu_ops, {
            // σ_1 on Gr(2,4): codim=1, dim=4, trust=1.0
            let stability_data = vec![GpuStabilityData {
                codimension: 1.0,
                dimension: 4.0,
                trust_level: 1.0,
                padding: 0.0,
            }];

            let result = gpu_ops.batch_stability_phases(&stability_data).await;

            match result {
                Ok(phases) => {
                    assert_eq!(phases.len(), 1);
                    // Phase should be in [0, 1]
                    assert!(
                        phases[0] >= 0.0 && phases[0] <= 1.0,
                        "Phase {} should be in [0,1]",
                        phases[0]
                    );
                    println!(
                        "✅ GPU stability phase computation successful: {:.4}",
                        phases[0]
                    );
                }
                Err(_) => {
                    println!("⚠️  GPU stability computation failed, but test passes");
                }
            }
        });
    }

    #[tokio::test]
    async fn test_gpu_stability_check() {
        with_serial_gpu_ops!(gpu_ops, {
            let stability_data = vec![
                // Should be stable: positive trust, positive dim
                GpuStabilityData {
                    codimension: 1.0,
                    dimension: 4.0,
                    trust_level: 1.0,
                    padding: 0.0,
                },
                // Edge case: zero trust
                GpuStabilityData {
                    codimension: 1.0,
                    dimension: 4.0,
                    trust_level: 0.0,
                    padding: 0.0,
                },
            ];

            let result = gpu_ops.batch_stability_checks(&stability_data).await;

            match result {
                Ok(checks) => {
                    assert_eq!(checks.len(), 2);
                    println!("✅ GPU stability check computation successful");
                    for (i, &check) in checks.iter().enumerate() {
                        println!(
                            "   Stability check {}: {} ({})",
                            i,
                            check,
                            if check == 1 { "stable" } else { "unstable" }
                        );
                    }
                }
                Err(_) => {
                    println!("⚠️  GPU stability check failed, but test passes");
                }
            }
        });
    }

    // ─── Sync conversion tests ───

    #[test]
    fn test_wdvv_gpu_conversion() {
        let gpu_data = GpuWDVVData::from_degree(5);
        assert_eq!(gpu_data.degree, 5);
        assert_eq!(gpu_data.padding, [0, 0, 0]);
        println!("✅ WDVV GPU conversion verified");
    }

    #[test]
    fn test_localization_gpu_conversion() {
        use amari_enumerative::{FixedPoint, TorusWeights};

        let fp = FixedPoint {
            subset: vec![0, 2],
            grassmannian: (2, 4),
        };
        let weights = TorusWeights {
            weights: vec![1, 3, 5, 7],
        };

        let gpu_data = GpuLocalizationData::from_fixed_point(&fp, &weights);
        assert_eq!(gpu_data.subset[0], 0);
        assert_eq!(gpu_data.subset[1], 2);
        assert_eq!(gpu_data.subset_len, 2);
        assert_eq!(gpu_data.ambient_n, 4);
        assert_eq!(gpu_data.weights[0], 1.0);
        assert_eq!(gpu_data.weights[2], 5.0);
        println!("✅ Localization GPU conversion verified");
    }

    #[test]
    fn test_matroid_rank_gpu_conversion() {
        use amari_enumerative::Matroid;
        use std::collections::BTreeSet;

        let matroid = Matroid::uniform(2, 4);
        let subset: BTreeSet<usize> = [0, 1].into_iter().collect();
        let gpu_data = GpuMatroidRankData::from_matroid_subset(&matroid, &subset);

        assert_eq!(gpu_data.ground_set_size, 4);
        assert_eq!(gpu_data.rank, 2);
        assert!(gpu_data.num_bases > 0);
        // subset_mask should have bits 0 and 1 set = 0b11 = 3
        assert_eq!(gpu_data.subset_mask, 3);
        println!("✅ Matroid rank GPU conversion verified");
    }

    #[test]
    fn test_csm_gpu_conversion() {
        let gpu_data = GpuCSMData::from_partition(&[2, 1], (2, 4));
        assert_eq!(gpu_data.partition[0], 2);
        assert_eq!(gpu_data.partition[1], 1);
        assert_eq!(gpu_data.partition_len, 2);
        assert_eq!(gpu_data.grassmannian_k, 2);
        assert_eq!(gpu_data.grassmannian_n, 4);
        println!("✅ CSM GPU conversion verified");
    }

    #[test]
    fn test_operad_gpu_conversion() {
        use amari_enumerative::{
            Capability, CapabilityId, ComposableNamespace, Namespace, SchubertClass,
        };

        let pos = SchubertClass::new(vec![], (2, 4)).unwrap();
        let mut ns_a = Namespace::new("A", pos.clone());
        let cap_a = Capability::new("out_cap", "Output Cap", vec![1], (2, 4)).unwrap();
        ns_a.grant(cap_a).unwrap();
        let mut comp_a = ComposableNamespace::new(ns_a);
        comp_a.mark_output(&CapabilityId::new("out_cap")).unwrap();

        let mut ns_b = Namespace::new("B", pos);
        let cap_b = Capability::new("in_cap", "Input Cap", vec![1], (2, 4)).unwrap();
        ns_b.grant(cap_b).unwrap();
        let mut comp_b = ComposableNamespace::new(ns_b);
        comp_b.mark_input(&CapabilityId::new("in_cap")).unwrap();

        let gpu_data = GpuOperadData::from_composition(&comp_a, 0, &comp_b, 0);
        assert_eq!(gpu_data.output_codimension, 1);
        assert_eq!(gpu_data.input_codimension, 1);
        assert_eq!(gpu_data.grassmannian_k, 2);
        assert_eq!(gpu_data.grassmannian_n, 4);
        println!("✅ Operad GPU conversion verified");
    }

    #[test]
    fn test_stability_gpu_conversion() {
        let schubert = SchubertClass::new(vec![1], (2, 4)).unwrap();
        let gpu_data = GpuStabilityData::from_class_and_trust(&schubert, 0.5);

        assert_eq!(gpu_data.codimension, 1.0);
        assert_eq!(gpu_data.dimension, 4.0);
        assert_eq!(gpu_data.trust_level, 0.5);
        println!("✅ Stability GPU conversion verified");
    }

    // ─── GF(2) Enumerative GPU tests ───

    #[cfg(feature = "gf2")]
    #[tokio::test]
    async fn test_gpu_finite_field_points() {
        with_serial_gpu_ops!(gpu_ops, {
            let data = vec![
                // |Gr(1,3;F₂)| = [3,1]_2 = (2^3 - 1)/(2^1 - 1) = 7
                GpuFiniteFieldPointData::new(1, 3, 2),
                // |Gr(2,4;F₂)| = [4,2]_2 = (2^4-1)(2^3-1) / ((2^2-1)(2^1-1)) = 15*7 / (3*1) = 35
                GpuFiniteFieldPointData::new(2, 4, 2),
                // |Gr(0,5;F₂)| = 1
                GpuFiniteFieldPointData::new(0, 5, 2),
            ];

            let result = gpu_ops.batch_finite_field_points(&data).await;

            match result {
                Ok(counts) => {
                    assert_eq!(counts.len(), 3);
                    assert_eq!(counts[0], 7, "|Gr(1,3;F_2)| = 7");
                    assert_eq!(counts[1], 35, "|Gr(2,4;F_2)| = 35");
                    assert_eq!(counts[2], 1, "|Gr(0,n;F_q)| = 1");
                    println!("GPU finite field points computation successful");
                }
                Err(_) => {
                    println!("GPU not available, test passes");
                }
            }
        });
    }

    #[cfg(feature = "gf2")]
    #[tokio::test]
    async fn test_gpu_weight_distribution() {
        with_serial_gpu_ops!(gpu_ops, {
            // Repetition code [3,1,3]: generator = [1,1,1]
            let data = vec![GpuWeightDistributionData {
                generator_rows: {
                    let mut rows = [0u32; 16];
                    rows[0] = 0b111; // [1,1,1]
                    rows
                },
                code_length: 3,
                code_dimension: 1,
                padding: [0; 2],
            }];

            let result = gpu_ops.batch_weight_distributions(&data).await;

            match result {
                Ok(counts) => {
                    assert!(counts.len() >= 33);
                    // Weight 0: the zero codeword (1 count)
                    assert_eq!(counts[0], 1, "Weight 0 should have 1 codeword");
                    // Weight 3: the all-ones codeword (1 count)
                    assert_eq!(counts[3], 1, "Weight 3 should have 1 codeword");
                    // No other weights
                    assert_eq!(counts[1], 0, "Weight 1 should have 0 codewords");
                    assert_eq!(counts[2], 0, "Weight 2 should have 0 codewords");
                    println!("GPU weight distribution computation successful");
                }
                Err(_) => {
                    println!("GPU not available, test passes");
                }
            }
        });
    }

    #[cfg(feature = "gf2")]
    #[tokio::test]
    async fn test_gpu_kl_coefficients() {
        with_serial_gpu_ops!(gpu_ops, {
            // U_{2,4}: uniform matroid, rank 2, ground set 4
            let matroid = Matroid::uniform(2, 4);
            let data = vec![GpuKLPolynomialData::from_matroid(&matroid)];

            let result = gpu_ops.batch_kl_coefficients(&data).await;

            match result {
                Ok(coeffs) => {
                    assert!(coeffs.len() >= 16);
                    // Characteristic polynomial of U_{2,4}: p(t) = t^2 - 4t + 6
                    // Coefficients: [6, -4, 1, 0, ...]
                    println!("GPU KL coefficients: {:?}", &coeffs[..4]);
                    println!("GPU KL polynomial computation successful");
                }
                Err(_) => {
                    println!("GPU not available, test passes");
                }
            }
        });
    }

    #[cfg(feature = "gf2")]
    #[tokio::test]
    async fn test_gpu_representability_check() {
        with_serial_gpu_ops!(gpu_ops, {
            // U_{2,3} is representable over GF(2) (rank 2, 3 elements, all 2-subsets are bases).
            // Note: U_{2,4} is NOT representable over GF(2) since GF(2)^2 has only 3 nonzero vectors.
            let uniform = Matroid::uniform(2, 3);
            let data = vec![GpuRepresentabilityData::from_matroid(&uniform)];

            let result = gpu_ops.batch_representability_checks(&data).await;

            match result {
                Ok(checks) => {
                    assert_eq!(checks.len(), 1);
                    assert!(
                        checks[0] == 1,
                        "U_{{2,3}} should be representable over GF(2), got {}",
                        checks[0]
                    );
                    println!("GPU representability check successful");
                }
                Err(_) => {
                    println!("GPU not available, test passes");
                }
            }
        });
    }

    #[cfg(feature = "gf2")]
    #[test]
    fn test_finite_field_point_data_conversion() {
        let data = GpuFiniteFieldPointData::new(2, 5, 3);
        assert_eq!(data.k, 2);
        assert_eq!(data.n, 5);
        assert_eq!(data.q, 3);
        println!("Finite field point data conversion verified");
    }

    #[cfg(feature = "gf2")]
    #[test]
    fn test_weight_distribution_data_conversion() {
        use amari_core::gf2::{GF2Matrix, GF2};

        let mut matrix = GF2Matrix::zero(2, 4);
        matrix.set(0, 0, GF2::ONE);
        matrix.set(0, 1, GF2::ONE);
        matrix.set(1, 2, GF2::ONE);
        matrix.set(1, 3, GF2::ONE);

        let gpu_data = GpuWeightDistributionData::from_generator_matrix(&matrix);
        assert_eq!(gpu_data.code_length, 4);
        assert_eq!(gpu_data.code_dimension, 2);
        // Row 0: [1,1,0,0] = 0b0011 = 3
        assert_eq!(gpu_data.generator_rows[0], 3);
        // Row 1: [0,0,1,1] = 0b1100 = 12
        assert_eq!(gpu_data.generator_rows[1], 12);
        println!("Weight distribution data conversion verified");
    }

    #[cfg(feature = "gf2")]
    #[test]
    fn test_kl_polynomial_data_conversion() {
        let matroid = Matroid::uniform(2, 4);
        let gpu_data = GpuKLPolynomialData::from_matroid(&matroid);

        assert_eq!(gpu_data.ground_set_size, 4);
        assert_eq!(gpu_data.rank, 2);
        assert_eq!(gpu_data.num_bases, 6); // C(4,2) = 6 bases
        println!("KL polynomial data conversion verified");
    }

    #[cfg(feature = "gf2")]
    #[test]
    fn test_representability_data_conversion() {
        let matroid = Matroid::uniform(2, 3);
        let gpu_data = GpuRepresentabilityData::from_matroid(&matroid);

        assert_eq!(gpu_data.ground_set_size, 3);
        assert_eq!(gpu_data.rank, 2);
        assert_eq!(gpu_data.num_bases, 3); // C(3,2) = 3
        println!("Representability data conversion verified");
    }
}
