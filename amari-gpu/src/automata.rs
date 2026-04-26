//! GPU-backed and GPU-ready cellular automata operations.
//!
//! Current public behavior is intentionally explicit:
//! - `AutomataGpuOps::batch_apply_rules` is GPU-backed and applies the first rule to each cell.
//! - `AutomataGpuOps::batch_evolve_ca` repeats the GPU rule-application path for multiple steps.
//! - `AutomataGpuOps::calculate_total_energy` is GPU-backed and sums multivector component energy.
//! - `AutomataGpuOps::extract_neighborhoods` currently uses a CPU Moore-neighborhood baseline.
//! - CA-evolution and neighborhood shader pipelines are kept as validation-safe scaffolding until
//!   richer neighborhood-aware GPU evolution is restored.

pub use self::gpu_impl::*;

mod gpu_impl {
    use amari_automata::{AutomataError, CellState, RuleType};
    use bytemuck::{Pod, Zeroable};
    use std::collections::HashMap;
    use std::fmt;
    use std::mem;
    use wgpu::util::DeviceExt;

    /// GPU-optimized cellular automaton cell data
    #[repr(C)]
    #[derive(Copy, Clone, Debug, Pod, Zeroable)]
    pub struct GpuCellData {
        // Multivector components (up to 8 components for 3D geometric algebra)
        pub scalar: f32,
        pub e1: f32,
        pub e2: f32,
        pub e3: f32,
        pub e12: f32,
        pub e13: f32,
        pub e23: f32,
        pub e123: f32,
        // Cell metadata
        pub generation: f32,
        pub neighborhood_size: f32,
        pub rule_type: f32,
        pub boundary_condition: f32,
        // Padding for alignment
        pub padding: [f32; 4],
    }

    /// GPU-optimized CA rule configuration
    #[repr(C)]
    #[derive(Copy, Clone, Debug, Pod, Zeroable)]
    pub struct GpuRuleConfig {
        pub rule_type: f32,
        pub threshold: f32,
        pub damping_factor: f32,
        pub energy_conservation: f32,
        pub time_step: f32,
        pub spatial_scale: f32,
        pub geometric_weight: f32,
        pub nonlinear_factor: f32,
        pub boundary_type: f32,
        pub neighborhood_radius: f32,
        pub evolution_speed: f32,
        pub stability_factor: f32,
        pub padding: [f32; 4],
    }

    /// GPU-optimized batch evolution parameters
    #[repr(C)]
    #[derive(Copy, Clone, Debug, Pod, Zeroable)]
    pub struct GpuEvolutionParams {
        pub grid_width: f32,
        pub grid_height: f32,
        pub total_cells: f32,
        pub steps_per_batch: f32,
        pub current_generation: f32,
        pub max_generations: f32,
        pub convergence_threshold: f32,
        pub energy_scale: f32,
        // Performance parameters
        pub workgroup_size_x: f32,
        pub workgroup_size_y: f32,
        pub parallel_factor: f32,
        pub memory_optimization: f32,
        pub padding: [f32; 4],
    }

    /// Error types for GPU cellular automata operations
    #[derive(Debug)]
    pub enum AutomataGpuError {
        /// GPU device creation failed
        DeviceCreationFailed(String),
        /// Shader compilation failed
        ShaderCompilationFailed(String),
        /// Buffer allocation failed
        BufferAllocationFailed(String),
        /// Compute pipeline creation failed
        PipelineCreationFailed(String),
        /// Evolution computation failed
        EvolutionComputationFailed(String),
        /// General GPU error
        GpuError(String),
    }

    /// Result type for GPU operations
    pub type AutomataGpuResult<T> = Result<T, AutomataGpuError>;

    /// GPU context for cellular automata operations
    #[allow(dead_code)]
    pub struct AutomataGpuContext {
        device: wgpu::Device,
        queue: wgpu::Queue,
        #[allow(dead_code)]
        ca_evolution_pipeline: wgpu::ComputePipeline,
        rule_application_pipeline: wgpu::ComputePipeline,
        energy_calculation_pipeline: wgpu::ComputePipeline,
        #[allow(dead_code)]
        neighbor_extraction_pipeline: wgpu::ComputePipeline,
        #[allow(dead_code)]
        config: AutomataGpuConfig,
    }

    /// Configuration for GPU cellular automata operations
    #[derive(Clone)]
    pub struct AutomataGpuConfig {
        pub preferred_backend: wgpu::Backends,
        pub power_preference: wgpu::PowerPreference,
        pub max_grid_size: usize,
        pub max_generations: usize,
        pub workgroup_size: (u32, u32, u32),
    }

    impl Default for AutomataGpuConfig {
        fn default() -> Self {
            Self {
                preferred_backend: wgpu::Backends::all(),
                power_preference: wgpu::PowerPreference::HighPerformance,
                max_grid_size: 1024 * 1024,
                max_generations: 10000,
                workgroup_size: (16, 16, 1),
            }
        }
    }

    /// Main GPU operations for cellular automata
    pub struct AutomataGpuOps {
        context: AutomataGpuContext,
        #[allow(dead_code)]
        evolution_cache: HashMap<String, Vec<f32>>,
        #[allow(dead_code)]
        energy_cache: HashMap<String, f32>,
    }

    impl AutomataGpuOps {
        /// Create new GPU automata operations
        pub async fn new() -> AutomataGpuResult<Self> {
            Self::with_config(AutomataGpuConfig::default()).await
        }

        /// Create with custom configuration
        pub async fn with_config(config: AutomataGpuConfig) -> AutomataGpuResult<Self> {
            let context = AutomataGpuContext::new(config).await?;

            Ok(Self {
                context,
                evolution_cache: HashMap::new(),
                energy_cache: HashMap::new(),
            })
        }

        /// Evolve cellular automata for multiple steps through the GPU rule-application path.
        pub async fn batch_evolve_ca(
            &mut self,
            initial_states: &[GpuCellData],
            rule_configs: &[GpuRuleConfig],
            evolution_params: &GpuEvolutionParams,
        ) -> AutomataGpuResult<Vec<GpuCellData>> {
            if initial_states.is_empty() {
                return Ok(Vec::new());
            }
            if rule_configs.is_empty() {
                return Err(AutomataGpuError::EvolutionComputationFailed(
                    "At least one rule configuration is required".to_string(),
                ));
            }
            if !evolution_params.steps_per_batch.is_finite()
                || evolution_params.steps_per_batch < 0.0
            {
                return Err(AutomataGpuError::EvolutionComputationFailed(
                    "steps_per_batch must be finite and non-negative".to_string(),
                ));
            }

            let steps = evolution_params.steps_per_batch as usize;
            let mut current_states = initial_states.to_vec();

            for step in 0..steps {
                current_states = self
                    .single_evolution_step(&current_states, rule_configs, evolution_params, step)
                    .await?;
            }

            Ok(current_states)
        }

        /// Apply the first CA rule to a batch of cells on the GPU.
        pub async fn batch_apply_rules(
            &mut self,
            cells: &[GpuCellData],
            rules: &[GpuRuleConfig],
        ) -> AutomataGpuResult<Vec<GpuCellData>> {
            if cells.is_empty() {
                return Ok(Vec::new());
            }
            if rules.is_empty() {
                return Err(AutomataGpuError::EvolutionComputationFailed(
                    "At least one rule configuration is required".to_string(),
                ));
            }

            // Create buffers
            let cell_buffer =
                self.context
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Cell Data Buffer"),
                        contents: bytemuck::cast_slice(cells),
                        usage: wgpu::BufferUsages::STORAGE
                            | wgpu::BufferUsages::COPY_DST
                            | wgpu::BufferUsages::COPY_SRC,
                    });

            let rule_buffer =
                self.context
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Rule Config Buffer"),
                        contents: bytemuck::cast_slice(rules),
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    });

            let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Output Buffer"),
                size: core::mem::size_of_val(cells) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });

            // Create bind group
            let bind_group_layout = self
                .context
                .rule_application_pipeline
                .get_bind_group_layout(0);
            let bind_group = self
                .context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Rule Application Bind Group"),
                    layout: &bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: cell_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: rule_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: output_buffer.as_entire_binding(),
                        },
                    ],
                });

            // Execute compute pass
            let mut encoder =
                self.context
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Rule Application Encoder"),
                    });

            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Rule Application Pass"),
                    timestamp_writes: None,
                });

                compute_pass.set_pipeline(&self.context.rule_application_pipeline);
                compute_pass.set_bind_group(0, &bind_group, &[]);

                let workgroup_count = cells.len().div_ceil(256) as u32;
                compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
            }

            // Read back results
            let staging_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Staging Buffer"),
                size: output_buffer.size(),
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            encoder.copy_buffer_to_buffer(
                &output_buffer,
                0,
                &staging_buffer,
                0,
                output_buffer.size(),
            );
            self.context.queue.submit(Some(encoder.finish()));

            let buffer_slice = staging_buffer.slice(..);
            let (sender, receiver) = futures::channel::oneshot::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });

            self.context.device.poll(wgpu::Maintain::Wait);
            receiver
                .await
                .map_err(|_| {
                    AutomataGpuError::EvolutionComputationFailed(
                        "Buffer mapping channel closed".to_string(),
                    )
                })?
                .map_err(|e| {
                    AutomataGpuError::EvolutionComputationFailed(format!(
                        "Buffer mapping failed: {:?}",
                        e
                    ))
                })?;

            let data = buffer_slice.get_mapped_range();
            let result: Vec<GpuCellData> = bytemuck::cast_slice(&data).to_vec();
            drop(data);
            staging_buffer.unmap();

            Ok(result)
        }

        /// Calculate total energy of cellular automata system
        pub async fn calculate_total_energy(
            &mut self,
            cells: &[GpuCellData],
        ) -> AutomataGpuResult<f32> {
            if cells.is_empty() {
                return Ok(0.0);
            }

            // Create energy calculation buffers
            let cell_buffer =
                self.context
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Cell Energy Buffer"),
                        contents: bytemuck::cast_slice(cells),
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    });

            let energy_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Energy Result Buffer"),
                size: mem::size_of::<f32>() as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });

            // Create bind group
            let bind_group_layout = self
                .context
                .energy_calculation_pipeline
                .get_bind_group_layout(0);
            let bind_group = self
                .context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Energy Calculation Bind Group"),
                    layout: &bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: cell_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: energy_buffer.as_entire_binding(),
                        },
                    ],
                });

            // Execute compute pass
            let mut encoder =
                self.context
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Energy Calculation Encoder"),
                    });

            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Energy Calculation Pass"),
                    timestamp_writes: None,
                });

                compute_pass.set_pipeline(&self.context.energy_calculation_pipeline);
                compute_pass.set_bind_group(0, &bind_group, &[]);
                compute_pass.dispatch_workgroups(1, 1, 1);
            }

            // Read back result
            let staging_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Energy Staging Buffer"),
                size: mem::size_of::<f32>() as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            encoder.copy_buffer_to_buffer(
                &energy_buffer,
                0,
                &staging_buffer,
                0,
                mem::size_of::<f32>() as u64,
            );
            self.context.queue.submit(Some(encoder.finish()));

            let buffer_slice = staging_buffer.slice(..);
            let (sender, receiver) = futures::channel::oneshot::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });

            self.context.device.poll(wgpu::Maintain::Wait);
            receiver
                .await
                .map_err(|_| {
                    AutomataGpuError::EvolutionComputationFailed(
                        "Buffer mapping channel closed".to_string(),
                    )
                })?
                .map_err(|e| {
                    AutomataGpuError::EvolutionComputationFailed(format!(
                        "Buffer mapping failed: {:?}",
                        e
                    ))
                })?;

            let data = buffer_slice.get_mapped_range();
            let energy: f32 = bytemuck::cast_slice(&data)[0];
            drop(data);
            staging_buffer.unmap();

            Ok(energy)
        }

        /// Extract Moore neighborhoods for all cells using a CPU baseline.
        ///
        /// This preserves public correctness while the neighborhood-aware GPU evolution
        /// kernel is redesigned and validated.
        pub async fn extract_neighborhoods(
            &mut self,
            cells: &[GpuCellData],
            grid_width: usize,
            grid_height: usize,
        ) -> AutomataGpuResult<Vec<Vec<GpuCellData>>> {
            if cells.is_empty() {
                return Ok(Vec::new());
            }
            if grid_width == 0 || grid_height == 0 || grid_width * grid_height != cells.len() {
                return Err(AutomataGpuError::EvolutionComputationFailed(format!(
                    "Expected {} cells for {}x{} grid, got {}",
                    grid_width * grid_height,
                    grid_width,
                    grid_height,
                    cells.len()
                )));
            }

            let offsets = [
                (-1isize, -1isize),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ];

            let neighborhoods = (0..cells.len())
                .map(|idx| {
                    let x = idx % grid_width;
                    let y = idx / grid_width;
                    offsets
                        .iter()
                        .map(|(dx, dy)| {
                            let nx = (x as isize + dx).rem_euclid(grid_width as isize) as usize;
                            let ny = (y as isize + dy).rem_euclid(grid_height as isize) as usize;
                            cells[ny * grid_width + nx]
                        })
                        .collect()
                })
                .collect();

            Ok(neighborhoods)
        }

        /// Single evolution step implementation
        async fn single_evolution_step(
            &mut self,
            cells: &[GpuCellData],
            rules: &[GpuRuleConfig],
            params: &GpuEvolutionParams,
            step: usize,
        ) -> AutomataGpuResult<Vec<GpuCellData>> {
            // Apply rules to evolve one generation
            let mut evolved_cells = self.batch_apply_rules(cells, rules).await?;

            // Update generation counter
            for cell in &mut evolved_cells {
                cell.generation = params.current_generation + step as f32 + 1.0;
            }

            Ok(evolved_cells)
        }
    }

    impl AutomataGpuContext {
        /// Create new GPU context
        async fn new(config: AutomataGpuConfig) -> AutomataGpuResult<Self> {
            // Request adapter
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: config.preferred_backend,
                flags: wgpu::InstanceFlags::default(),
                dx12_shader_compiler: wgpu::Dx12Compiler::default(),
                gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
            });

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: config.power_preference,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .ok_or_else(|| {
                    AutomataGpuError::DeviceCreationFailed("No suitable adapter found".to_string())
                })?;

            // Request device and queue
            let (device, queue) = adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("Automata GPU Device"),
                        required_features: wgpu::Features::empty(),
                        required_limits: wgpu::Limits::default(),
                    },
                    None,
                )
                .await
                .map_err(|e| {
                    AutomataGpuError::DeviceCreationFailed(format!(
                        "Device request failed: {:?}",
                        e
                    ))
                })?;

            // Create compute pipelines
            let ca_evolution_pipeline = Self::create_ca_evolution_pipeline(&device)?;
            let rule_application_pipeline = Self::create_rule_application_pipeline(&device)?;
            let energy_calculation_pipeline = Self::create_energy_calculation_pipeline(&device)?;
            let neighbor_extraction_pipeline = Self::create_neighbor_extraction_pipeline(&device)?;

            Ok(Self {
                device,
                queue,
                ca_evolution_pipeline,
                rule_application_pipeline,
                energy_calculation_pipeline,
                neighbor_extraction_pipeline,
                config,
            })
        }

        /// Create CA evolution compute pipeline
        fn create_ca_evolution_pipeline(
            device: &wgpu::Device,
        ) -> AutomataGpuResult<wgpu::ComputePipeline> {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("CA Evolution Scaffolding Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    "@compute @workgroup_size(1) fn ca_evolution_main() { return; }".into(),
                ),
            });

            Ok(
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("CA Evolution Scaffolding Pipeline"),
                    layout: None,
                    module: &shader,
                    entry_point: "ca_evolution_main",
                }),
            )
        }

        /// Create rule application compute pipeline
        fn create_rule_application_pipeline(
            device: &wgpu::Device,
        ) -> AutomataGpuResult<wgpu::ComputePipeline> {
            let shader_source = crate::shaders::RULE_APPLICATION;
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Rule Application Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_source.into()),
            });

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Rule Application Bind Group Layout"),
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
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
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
                label: Some("Rule Application Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            Ok(
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Rule Application Pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: "rule_application_main",
                }),
            )
        }

        /// Create energy calculation pipeline
        fn create_energy_calculation_pipeline(
            device: &wgpu::Device,
        ) -> AutomataGpuResult<wgpu::ComputePipeline> {
            let shader_source = crate::shaders::ENERGY_CALCULATION;
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Energy Calculation Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_source.into()),
            });

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Energy Calculation Bind Group Layout"),
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
                });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Energy Calculation Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            Ok(
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Energy Calculation Pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: "energy_calculation_main",
                }),
            )
        }

        /// Create neighbor extraction pipeline
        fn create_neighbor_extraction_pipeline(
            device: &wgpu::Device,
        ) -> AutomataGpuResult<wgpu::ComputePipeline> {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Neighbor Extraction Scaffolding Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    "@compute @workgroup_size(1) fn neighbor_extraction_main() { return; }".into(),
                ),
            });

            Ok(
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Neighbor Extraction Scaffolding Pipeline"),
                    layout: None,
                    module: &shader,
                    entry_point: "neighbor_extraction_main",
                }),
            )
        }
    }

    impl Default for GpuCellData {
        fn default() -> Self {
            Self {
                scalar: 0.0,
                e1: 0.0,
                e2: 0.0,
                e3: 0.0,
                e12: 0.0,
                e13: 0.0,
                e23: 0.0,
                e123: 0.0,
                generation: 0.0,
                neighborhood_size: 8.0,
                rule_type: 0.0,
                boundary_condition: 0.0,
                padding: [0.0; 4],
            }
        }
    }

    impl Default for GpuRuleConfig {
        fn default() -> Self {
            Self {
                rule_type: 0.0, // Geometric rule
                threshold: 0.5,
                damping_factor: 0.1,
                energy_conservation: 1.0,
                time_step: 1.0,
                spatial_scale: 1.0,
                geometric_weight: 1.0,
                nonlinear_factor: 0.0,
                boundary_type: 0.0, // Periodic
                neighborhood_radius: 1.0,
                evolution_speed: 1.0,
                stability_factor: 0.9,
                padding: [0.0; 4],
            }
        }
    }

    impl Default for GpuEvolutionParams {
        fn default() -> Self {
            Self {
                grid_width: 64.0,
                grid_height: 64.0,
                total_cells: 4096.0,
                steps_per_batch: 10.0,
                current_generation: 0.0,
                max_generations: 1000.0,
                convergence_threshold: 0.001,
                energy_scale: 1.0,
                workgroup_size_x: 16.0,
                workgroup_size_y: 16.0,
                parallel_factor: 1.0,
                memory_optimization: 1.0,
                padding: [0.0; 4],
            }
        }
    }

    // Conversion traits for seamless integration with existing code
    impl From<&CellState<3, 0, 0>> for GpuCellData {
        fn from(cell: &CellState<3, 0, 0>) -> Self {
            // Simple conversion using scalar part only for now
            Self {
                scalar: cell.scalar_part() as f32,
                e1: 0.0, // Current conversion preserves scalar component only.
                e2: 0.0,
                e3: 0.0,
                e12: 0.0,
                e13: 0.0,
                e23: 0.0,
                e123: 0.0,
                generation: 0.0,
                neighborhood_size: 8.0,
                rule_type: 0.0,
                boundary_condition: 0.0,
                padding: [0.0; 4],
            }
        }
    }

    impl From<&RuleType> for GpuRuleConfig {
        fn from(rule_type: &RuleType) -> Self {
            Self {
                rule_type: match rule_type {
                    RuleType::Geometric => 0.0,
                    RuleType::GameOfLife => 1.0,
                    RuleType::Reversible => 2.0,
                    RuleType::RotorCA => 3.0,
                    RuleType::GradePreserving => 4.0,
                    RuleType::Conservative => 5.0,
                },
                ..Self::default()
            }
        }
    }

    impl fmt::Display for AutomataGpuError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::DeviceCreationFailed(msg) => write!(f, "GPU device creation failed: {}", msg),
                Self::ShaderCompilationFailed(msg) => {
                    write!(f, "Shader compilation failed: {}", msg)
                }
                Self::BufferAllocationFailed(msg) => write!(f, "Buffer allocation failed: {}", msg),
                Self::PipelineCreationFailed(msg) => write!(f, "Pipeline creation failed: {}", msg),
                Self::EvolutionComputationFailed(msg) => {
                    write!(f, "Evolution computation failed: {}", msg)
                }
                Self::GpuError(msg) => write!(f, "GPU error: {}", msg),
            }
        }
    }

    // Note: Error trait not available in no_std
    // impl std::error::Error for AutomataGpuError {}

    impl From<AutomataGpuError> for AutomataError {
        fn from(gpu_error: AutomataGpuError) -> Self {
            AutomataError::InvalidConfiguration(format!("GPU error: {}", gpu_error))
        }
    }
}
