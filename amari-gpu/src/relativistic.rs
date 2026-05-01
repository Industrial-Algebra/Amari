//! GPU acceleration for relativistic physics computations
//!
//! This module provides GPU-accelerated implementations of relativistic physics
//! operations from amari-relativistic, including spacetime algebra operations
//! and a narrow Schwarzschild-style particle propagation kernel.
//!
//! The public v1 surface is GPU-backed but intentionally bounded: spacetime
//! vectors store coordinates as `(ct, x, y, z)`, Minkowski products compute
//! `ct² - x² - y² - z²`, and particle propagation uses a simplified GPU
//! geodesic step. Inputs are validated for finite values and basic parameter
//! consistency before dispatch.

use crate::GpuError;
use amari_relativistic::{particle::RelativisticParticle, spacetime::SpacetimeVector};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// GPU-accelerated spacetime vector operations using Cl(1,3) signature
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuSpacetimeVector {
    /// Temporal component (`ct`, in length units)
    pub t: f32,
    /// Spatial x component
    pub x: f32,
    /// Spatial y component
    pub y: f32,
    /// Spatial z component
    pub z: f32,
}

impl GpuSpacetimeVector {
    /// Create new GPU spacetime vector
    pub fn new(t: f32, x: f32, y: f32, z: f32) -> Self {
        Self { t, x, y, z }
    }

    /// Convert from CPU spacetime vector, preserving coordinates `[ct, x, y, z]`.
    pub fn from_spacetime_vector(sv: &SpacetimeVector) -> Self {
        let coords = sv.coordinates();
        Self::new(
            coords[0] as f32,
            coords[1] as f32,
            coords[2] as f32,
            coords[3] as f32,
        )
    }

    /// Convert to CPU spacetime vector, interpreting `t` as `ct`.
    pub fn to_spacetime_vector(&self) -> SpacetimeVector {
        SpacetimeVector::from_coordinates([
            self.t as f64,
            self.x as f64,
            self.y as f64,
            self.z as f64,
        ])
    }

    fn is_finite(&self) -> bool {
        self.t.is_finite() && self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

/// GPU-accelerated relativistic particle for trajectory calculations
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuRelativisticParticle {
    /// Spacetime position
    pub position: GpuSpacetimeVector,
    /// Four-velocity
    pub velocity: GpuSpacetimeVector,
    /// Rest mass
    pub mass: f32,
    /// Electric charge
    pub charge: f32,
    /// Proper time
    pub proper_time: f32,
    /// Padding for WGSL storage-buffer alignment (64-byte particle stride)
    pub _padding: [f32; 5],
}

/// GPU-accelerated trajectory calculation parameters
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuTrajectoryParams {
    /// Integration time step
    pub dt: f32,
    /// Number of integration steps
    pub steps: u32,
    /// Normalization tolerance
    pub tolerance: f32,
    /// Renormalization frequency
    pub renorm_freq: u32,
    /// Schwarzschild radius (for gravitational fields)
    pub schwarzschild_radius: f32,
    /// Central mass parameter (GM)
    pub gm_parameter: f32,
    /// Padding for alignment
    pub _padding: [f32; 2],
}

/// GPU compute context for relativistic physics
pub struct GpuRelativisticPhysics {
    device: wgpu::Device,
    queue: wgpu::Queue,
    spacetime_pipeline: wgpu::ComputePipeline,
    geodesic_pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)]
    trajectory_pipeline: wgpu::ComputePipeline,
}

impl GpuRelativisticPhysics {
    /// Initialize GPU context for relativistic physics computations
    pub async fn new() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .ok_or_else(|| {
                GpuError::InitializationError("No suitable GPU adapter found".to_string())
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Relativistic Physics GPU"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| {
                GpuError::InitializationError(format!("Failed to create device: {}", e))
            })?;

        // Compile compute shaders for different operations
        let spacetime_pipeline = Self::create_spacetime_pipeline(&device)?;
        let geodesic_pipeline = Self::create_geodesic_pipeline(&device)?;
        let trajectory_pipeline = Self::create_trajectory_pipeline(&device)?;

        Ok(Self {
            device,
            queue,
            spacetime_pipeline,
            geodesic_pipeline,
            trajectory_pipeline,
        })
    }

    /// Create compute pipeline for spacetime algebra operations
    fn create_spacetime_pipeline(device: &wgpu::Device) -> Result<wgpu::ComputePipeline, GpuError> {
        let shader_source = r#"
            @group(0) @binding(0) var<storage, read_write> vectors: array<vec4<f32>>;
            @group(0) @binding(1) var<storage, read_write> results: array<f32>;

            @compute @workgroup_size(64)
            fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
                let index = global_id.x;
                if (index >= arrayLength(&vectors)) {
                    return;
                }

                let v = vectors[index];

                // Minkowski inner product: t² - x² - y² - z²
                let minkowski_norm_sq = v.x * v.x - v.y * v.y - v.z * v.z - v.w * v.w;
                results[index] = minkowski_norm_sq;

                // Normalize four-velocity if needed (u·u = c²)
                let c_sq = 299792458.0 * 299792458.0;
                if (abs(minkowski_norm_sq - c_sq) > 1e-6) {
                    let norm = sqrt(abs(minkowski_norm_sq));
                    if (norm > 1e-12) {
                        let factor = sqrt(c_sq) / norm;
                        vectors[index] = vec4<f32>(v.x * factor, v.y * factor, v.z * factor, v.w * factor);
                    }
                }
            }
        "#;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Spacetime Algebra Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Spacetime Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
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
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Spacetime Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Spacetime Compute Pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: "main",
            }),
        )
    }

    /// Create compute pipeline for geodesic integration
    fn create_geodesic_pipeline(device: &wgpu::Device) -> Result<wgpu::ComputePipeline, GpuError> {
        let shader_source = r#"
            struct Particle {
                position: vec4<f32>,
                velocity: vec4<f32>,
                mass: f32,
                charge: f32,
                proper_time: f32,
                padding: array<f32, 5>,
            };

            struct TrajectoryParams {
                dt: f32,
                steps: u32,
                tolerance: f32,
                renorm_freq: u32,
                rs: f32,
                gm: f32,
                padding: vec2<f32>,
            };

            @group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
            @group(0) @binding(1) var<uniform> params: TrajectoryParams;

            // Schwarzschild metric Christoffel symbols (simplified)
            fn christoffel_t_rr(r: f32, rs: f32) -> f32 {
                let factor = rs / (2.0 * r * r);
                return factor * (1.0 - rs / r);
            }

            fn christoffel_r_tr(r: f32, rs: f32) -> f32 {
                return rs / (2.0 * r * r * (1.0 - rs / r));
            }

            fn christoffel_r_rr(r: f32, rs: f32) -> f32 {
                return -rs / (2.0 * r * (r - rs));
            }

            @compute @workgroup_size(64)
            fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
                let index = global_id.x;
                if (index >= arrayLength(&particles)) {
                    return;
                }

                var particle = particles[index];
                let pos = particle.position;
                let vel = particle.velocity;

                // Compute spatial radius
                let r = sqrt(pos.y * pos.y + pos.z * pos.z + pos.w * pos.w);

                if (r < params.rs * 1.1) {
                    // Too close to singularity, skip
                    return;
                }

                // Velocity Verlet step for geodesic equation
                // Simplified for Schwarzschild metric

                // Compute acceleration components
                let c_trr = christoffel_t_rr(r, params.rs);
                let c_rtr = christoffel_r_tr(r, params.rs);
                let c_rrr = christoffel_r_rr(r, params.rs);

                // Geodesic equation: d²x^μ/dτ² = -Γ^μ_αβ v^α v^β
                var accel = vec4<f32>(0.0, 0.0, 0.0, 0.0);

                // Simplified acceleration calculation
                accel.x = -c_trr * vel.y * vel.y; // dt component
                accel.y = -c_rtr * vel.x * vel.y - c_rrr * vel.y * vel.y; // dr component

                // Update position and velocity
                let dt = params.dt;
                particle.position = pos + vel * dt + 0.5 * accel * dt * dt;
                particle.velocity = vel + accel * dt;

                // Renormalize four-velocity periodically
                let step_mod = u32(particle.proper_time / dt) % params.renorm_freq;
                if (step_mod == 0u) {
                    let c_sq = 299792458.0 * 299792458.0;
                    let norm_sq = particle.velocity.x * particle.velocity.x -
                                  particle.velocity.y * particle.velocity.y -
                                  particle.velocity.z * particle.velocity.z -
                                  particle.velocity.w * particle.velocity.w;

                    if (abs(norm_sq - c_sq) > params.tolerance) {
                        let norm = sqrt(abs(norm_sq));
                        if (norm > 1e-12) {
                            let factor = sqrt(c_sq) / norm;
                            particle.velocity *= factor;
                        }
                    }
                }

                particle.proper_time += dt;
                particles[index] = particle;
            }
        "#;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Geodesic Integration Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Geodesic Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Geodesic Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        Ok(
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Geodesic Compute Pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: "main",
            }),
        )
    }

    /// Create compute pipeline for trajectory calculations
    fn create_trajectory_pipeline(
        device: &wgpu::Device,
    ) -> Result<wgpu::ComputePipeline, GpuError> {
        // For now, use the same pipeline as geodesic integration
        Self::create_geodesic_pipeline(device)
    }

    /// Compute Minkowski norm-squared values for multiple spacetime vectors.
    ///
    /// Returns `ct² - x² - y² - z²` for each vector.
    pub async fn compute_minkowski_products(
        &self,
        vectors: &[GpuSpacetimeVector],
    ) -> Result<Vec<f32>, GpuError> {
        if vectors.is_empty() {
            return Ok(Vec::new());
        }
        for (index, vector) in vectors.iter().enumerate() {
            if !vector.is_finite() {
                return Err(GpuError::BufferError(format!(
                    "spacetime vector {index} contains non-finite values"
                )));
            }
        }

        let vectors_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Spacetime Vectors Buffer"),
                contents: bytemuck::cast_slice(vectors),
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            });

        let results_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Results Buffer"),
            size: (vectors.len() * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let bind_group_layout = self.spacetime_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Spacetime Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: vectors_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: results_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Spacetime Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Spacetime Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.spacetime_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);

            let workgroup_size = 64;
            let num_workgroups = vectors.len().div_ceil(workgroup_size);
            compute_pass.dispatch_workgroups(num_workgroups as u32, 1, 1);
        }

        // Read back results
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: results_buffer.size(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        encoder.copy_buffer_to_buffer(
            &results_buffer,
            0,
            &staging_buffer,
            0,
            results_buffer.size(),
        );

        self.queue.submit([encoder.finish()]);

        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| {
            let _ = sender.send(v);
        });

        self.device.poll(wgpu::Maintain::wait()).panic_on_timeout();
        receiver
            .await
            .map_err(|_| GpuError::BufferError("Failed to receive buffer mapping".to_string()))?
            .map_err(|e| GpuError::BufferError(format!("Buffer mapping failed: {:?}", e)))?;

        let data = buffer_slice.get_mapped_range();
        let results: Vec<f32> = bytemuck::cast_slice(&data).to_vec();

        drop(data);
        staging_buffer.unmap();

        Ok(results)
    }

    /// Propagate multiple particles through spacetime using the simplified GPU geodesic kernel.
    pub async fn propagate_particles(
        &self,
        particles: &[GpuRelativisticParticle],
        params: &GpuTrajectoryParams,
    ) -> Result<Vec<GpuRelativisticParticle>, GpuError> {
        if particles.is_empty() || params.steps == 0 {
            return Ok(particles.to_vec());
        }
        Self::validate_trajectory_params(params)?;
        for (index, particle) in particles.iter().enumerate() {
            Self::validate_particle(index, particle)?;
        }

        let particles_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Particles Buffer"),
                contents: bytemuck::cast_slice(particles),
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            });

        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Trajectory Params Buffer"),
                contents: bytemuck::cast_slice(&[*params]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let bind_group_layout = self.geodesic_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Geodesic Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: particles_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        // Execute integration steps
        for _ in 0..params.steps {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Geodesic Compute Encoder"),
                });

            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Geodesic Compute Pass"),
                    timestamp_writes: None,
                });

                compute_pass.set_pipeline(&self.geodesic_pipeline);
                compute_pass.set_bind_group(0, &bind_group, &[]);

                let workgroup_size = 64;
                let num_workgroups = particles.len().div_ceil(workgroup_size);
                compute_pass.dispatch_workgroups(num_workgroups as u32, 1, 1);
            }

            self.queue.submit([encoder.finish()]);
            self.device.poll(wgpu::Maintain::wait()).panic_on_timeout();
        }

        // Read back final particle states
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particles Staging Buffer"),
            size: particles_buffer.size(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Copy Encoder"),
            });

        encoder.copy_buffer_to_buffer(
            &particles_buffer,
            0,
            &staging_buffer,
            0,
            particles_buffer.size(),
        );
        self.queue.submit([encoder.finish()]);

        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| {
            let _ = sender.send(v);
        });

        self.device.poll(wgpu::Maintain::wait()).panic_on_timeout();
        receiver
            .await
            .map_err(|_| GpuError::BufferError("Failed to receive buffer mapping".to_string()))?
            .map_err(|e| GpuError::BufferError(format!("Buffer mapping failed: {:?}", e)))?;

        let data = buffer_slice.get_mapped_range();
        let results: Vec<GpuRelativisticParticle> = bytemuck::cast_slice(&data).to_vec();

        drop(data);
        staging_buffer.unmap();

        Ok(results)
    }

    fn validate_trajectory_params(params: &GpuTrajectoryParams) -> Result<(), GpuError> {
        if !params.dt.is_finite() || params.dt <= 0.0 {
            return Err(GpuError::BufferError(
                "trajectory dt must be finite and positive".to_string(),
            ));
        }
        if !params.tolerance.is_finite() || params.tolerance < 0.0 {
            return Err(GpuError::BufferError(
                "trajectory tolerance must be finite and non-negative".to_string(),
            ));
        }
        if params.renorm_freq == 0 {
            return Err(GpuError::BufferError(
                "trajectory renorm_freq must be greater than zero".to_string(),
            ));
        }
        if !params.schwarzschild_radius.is_finite() || params.schwarzschild_radius < 0.0 {
            return Err(GpuError::BufferError(
                "schwarzschild_radius must be finite and non-negative".to_string(),
            ));
        }
        if !params.gm_parameter.is_finite() {
            return Err(GpuError::BufferError(
                "gm_parameter must be finite".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_particle(index: usize, particle: &GpuRelativisticParticle) -> Result<(), GpuError> {
        if !particle.position.is_finite() {
            return Err(GpuError::BufferError(format!(
                "particle {index} position contains non-finite values"
            )));
        }
        if !particle.velocity.is_finite() {
            return Err(GpuError::BufferError(format!(
                "particle {index} velocity contains non-finite values"
            )));
        }
        if !particle.mass.is_finite() || particle.mass < 0.0 {
            return Err(GpuError::BufferError(format!(
                "particle {index} mass must be finite and non-negative"
            )));
        }
        if !particle.charge.is_finite() {
            return Err(GpuError::BufferError(format!(
                "particle {index} charge must be finite"
            )));
        }
        if !particle.proper_time.is_finite() {
            return Err(GpuError::BufferError(format!(
                "particle {index} proper_time must be finite"
            )));
        }

        Ok(())
    }
}

/// Convert CPU relativistic particle to GPU format
impl From<&RelativisticParticle> for GpuRelativisticParticle {
    fn from(particle: &RelativisticParticle) -> Self {
        let pos = &particle.position;
        let vel = particle.four_velocity.as_spacetime_vector();

        Self {
            position: GpuSpacetimeVector::from_spacetime_vector(pos),
            velocity: GpuSpacetimeVector::from_spacetime_vector(vel),
            mass: particle.mass as f32,
            charge: particle.charge as f32,
            proper_time: 0.0, // Will be updated during integration
            _padding: [0.0; 5],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_spacetime_vector_conversion() {
        let cpu_vector = SpacetimeVector::new(1.0, 2.0, 3.0, 4.0);
        let gpu_vector = GpuSpacetimeVector::from_spacetime_vector(&cpu_vector);
        let converted_back = gpu_vector.to_spacetime_vector();

        assert!((converted_back.time() - 1.0).abs() < 1e-6);
        assert_eq!(converted_back.x(), 2.0);
        assert_eq!(converted_back.y(), 3.0);
        assert_eq!(converted_back.z(), 4.0);
    }

    #[tokio::test]
    #[ignore] // Skip in CI due to GPU hardware requirements
    async fn test_gpu_minkowski_products() {
        let gpu_physics = match GpuRelativisticPhysics::new().await {
            Ok(physics) => physics,
            Err(_) => {
                println!("GPU not available, skipping test");
                return;
            }
        };

        let vectors = vec![
            GpuSpacetimeVector::new(1.0, 0.5, 0.0, 0.0),
            GpuSpacetimeVector::new(2.0, 1.0, 0.0, 0.0),
        ];

        let results = gpu_physics
            .compute_minkowski_products(&vectors)
            .await
            .unwrap();

        // Check that we got results for each vector
        assert_eq!(results.len(), vectors.len());

        // Verify Minkowski inner product calculation (t² - x² - y² - z²)
        assert!((results[0] - (1.0 - 0.25)).abs() < 1e-6); // 1² - 0.5² = 0.75
        assert!((results[1] - (4.0 - 1.0)).abs() < 1e-6); // 2² - 1² = 3.0
    }
}
