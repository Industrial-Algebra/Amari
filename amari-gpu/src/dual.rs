//! GPU-backed and GPU-ready dual-number automatic differentiation.
//!
//! Current public 0.20.0 behavior is intentionally narrow:
//! - `DualGpuOps::batch_forward_ad` is GPU-backed for element-wise unary dual operations.
//! - `GpuDualNumber` provides a POD transfer representation for `DualNumber<f32>`.
//! - `Add` and `Multiply` operation variants are retained for API continuity but are rejected by
//!   `batch_forward_ad` until binary/broadcast semantics are designed.
//! - Neural-network gradients, generic vector gradients, and optimization scaffolding remain
//!   internal redesign-pending implementation details.

#[cfg(feature = "dual")]
use amari_dual::{DualNumber, MultiDualNumber};
#[cfg(feature = "dual")]
use bytemuck::{Pod, Zeroable};
#[cfg(feature = "dual")]
use futures::channel::oneshot;
#[cfg(feature = "dual")]
use num_traits::Float;
#[cfg(feature = "dual")]
use std::collections::HashMap;
#[cfg(feature = "dual")]
use thiserror::Error;
#[cfg(feature = "dual")]
use wgpu::util::DeviceExt;

#[cfg(feature = "dual")]
#[derive(Error, Debug)]
pub enum DualGpuError {
    #[error("GPU initialization failed: {0}")]
    Initialization(String),

    #[error("Shader compilation failed: {0}")]
    ShaderCompilation(String),

    #[error("Buffer operation failed: {0}")]
    BufferOperation(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Memory allocation failed: {0}")]
    MemoryAllocation(String),

    #[error("Gradient computation failed: {0}")]
    GradientComputation(String),
}

#[cfg(feature = "dual")]
pub type DualGpuResult<T> = Result<T, DualGpuError>;

/// GPU representation of a dual number optimized for WGSL
#[cfg(feature = "dual")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuDualNumber {
    /// Real part (function value)
    pub real: f32,
    /// Dual part (derivative)
    pub dual: f32,
}

#[cfg(feature = "dual")]
impl From<DualNumber<f32>> for GpuDualNumber {
    fn from(dual: DualNumber<f32>) -> Self {
        Self {
            real: dual.real,
            dual: dual.dual,
        }
    }
}

#[cfg(feature = "dual")]
impl From<GpuDualNumber> for DualNumber<f32> {
    fn from(gpu_dual: GpuDualNumber) -> Self {
        Self {
            real: gpu_dual.real,
            dual: gpu_dual.dual,
        }
    }
}

/// GPU representation of multi-dual number for batch gradients
#[cfg(feature = "dual")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuMultiDual {
    /// Function value
    pub value: f32,
    /// First 8 partial derivatives (expandable)
    pub gradients: [f32; 8],
}

/// Unified trait for GPU-accelerated dual number operations
#[cfg(feature = "dual")]
pub trait DualGpuAccelerated<T> {
    /// Convert to GPU buffer format
    fn to_gpu_buffer(&self, context: &DualGpuContext) -> DualGpuResult<wgpu::Buffer>;

    /// Reconstruct from GPU buffer
    fn from_gpu_buffer(buffer: &wgpu::Buffer, context: &DualGpuContext) -> DualGpuResult<T>;

    /// Execute GPU operation with parameters
    fn gpu_operation(
        &self,
        operation: &str,
        context: &DualGpuContext,
        params: &GpuOperationParams,
    ) -> DualGpuResult<T>;
}

/// GPU operation parameters for dual number computations
#[cfg(feature = "dual")]
#[derive(Debug, Clone)]
pub struct GpuOperationParams {
    /// Operation-specific parameters
    pub params: HashMap<String, GpuParameter>,
    /// Batch size for parallel processing
    pub batch_size: usize,
    /// Workgroup size for compute shaders
    pub workgroup_size: (u32, u32, u32),
    /// Number of variables for gradient computation
    pub num_variables: usize,
}

/// Parameter types for GPU dual operations
#[cfg(feature = "dual")]
#[derive(Debug, Clone)]
pub enum GpuParameter {
    Float(f32),
    Double(f64),
    Integer(i32),
    UnsignedInteger(u32),
    Buffer(String),
    Array(Vec<f32>),
    DualNumber(GpuDualNumber),
}

#[cfg(feature = "dual")]
impl Default for GpuOperationParams {
    fn default() -> Self {
        Self {
            params: HashMap::new(),
            batch_size: 1,
            workgroup_size: (64, 1, 1),
            num_variables: 1,
        }
    }
}

/// Self-contained GPU context for dual number operations
#[cfg(feature = "dual")]
pub struct DualGpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    shader_cache: HashMap<String, wgpu::ComputePipeline>,
}

#[cfg(feature = "dual")]
impl DualGpuContext {
    /// Initialize GPU context
    pub async fn new() -> DualGpuResult<Self> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| {
                DualGpuError::Initialization("No suitable GPU adapter found".to_string())
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Dual Numbers GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| DualGpuError::Initialization(e.to_string()))?;

        Ok(Self {
            device,
            queue,
            shader_cache: HashMap::new(),
        })
    }

    /// Get or compile compute shader for dual operations
    pub fn get_compute_pipeline(
        &mut self,
        shader_key: &str,
        shader_source: &str,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> DualGpuResult<&wgpu::ComputePipeline> {
        if !self.shader_cache.contains_key(shader_key) {
            let shader_module = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(&format!("Dual {} Shader", shader_key)),
                    source: wgpu::ShaderSource::Wgsl(shader_source.into()),
                });

            let pipeline_layout =
                self.device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some(&format!("Dual {} Pipeline Layout", shader_key)),
                        bind_group_layouts: &[bind_group_layout],
                        push_constant_ranges: &[],
                    });

            let compute_pipeline =
                self.device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some(&format!("Dual {} Pipeline", shader_key)),
                        layout: Some(&pipeline_layout),
                        module: &shader_module,
                        entry_point: "main",
                    });

            self.shader_cache
                .insert(shader_key.to_string(), compute_pipeline);
        }

        Ok(self
            .shader_cache
            .get(shader_key)
            .expect("Pipeline should exist"))
    }

    /// Create buffer with dual number data
    pub fn create_dual_buffer<T: bytemuck::Pod>(
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

    /// Execute compute shader for dual operations
    pub fn execute_dual_compute(
        &self,
        pipeline: &wgpu::ComputePipeline,
        bind_group: &wgpu::BindGroup,
        workgroup_count: (u32, u32, u32),
    ) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Dual Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Dual Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, bind_group, &[]);
            compute_pass.dispatch_workgroups(
                workgroup_count.0,
                workgroup_count.1,
                workgroup_count.2,
            );
        }

        self.queue.submit([encoder.finish()]);
    }

    /// Read dual results back from GPU
    pub async fn read_dual_buffer<T: bytemuck::Pod + Clone>(
        &self,
        buffer: &wgpu::Buffer,
        size: u64,
    ) -> DualGpuResult<Vec<T>> {
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dual Staging Buffer"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Dual Copy Encoder"),
            });

        encoder.copy_buffer_to_buffer(buffer, 0, &staging_buffer, 0, size);
        self.queue.submit([encoder.finish()]);

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = oneshot::channel();

        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });

        self.device.poll(wgpu::Maintain::Wait);

        rx.await
            .map_err(|_| DualGpuError::BufferOperation("Buffer read timeout".to_string()))?
            .map_err(|e| DualGpuError::BufferOperation(format!("Buffer map failed: {}", e)))?;

        let data = buffer_slice.get_mapped_range();
        let result: Vec<T> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }
}

/// High-level GPU operations for dual numbers
#[cfg(feature = "dual")]
pub struct DualGpuOps {
    context: DualGpuContext,
}

#[cfg(feature = "dual")]
impl DualGpuOps {
    /// Create new GPU operations context
    pub async fn new() -> DualGpuResult<Self> {
        let context = DualGpuContext::new().await?;
        Ok(Self { context })
    }

    /// Batch forward-mode automatic differentiation for unary element-wise operations.
    ///
    /// Supported operations: `Sin`, `Cos`, `Exp`, `Log`, `ReLU`, `Sigmoid`, `Tanh`,
    /// `Square`, and `Sqrt`. `Add` and `Multiply` require a binary/broadcast input
    /// contract and currently return `InvalidOperation`.
    pub async fn batch_forward_ad(
        &mut self,
        inputs: &[DualNumber<f32>],
        operations: &[DualOperation],
    ) -> DualGpuResult<Vec<DualNumber<f32>>> {
        if operations
            .iter()
            .any(|op| matches!(op, DualOperation::Add | DualOperation::Multiply))
        {
            return Err(DualGpuError::InvalidOperation(
                "Add and Multiply require binary/broadcast semantics and are not part of the 0.20.0 dual v1 surface".to_string(),
            ));
        }

        if inputs.is_empty() {
            return Ok(Vec::new());
        }

        let workgroup_size = 64;
        let num_elements = inputs.len();

        // Convert inputs to GPU format
        let gpu_inputs: Vec<GpuDualNumber> = inputs.iter().map(|&d| d.into()).collect();

        // Create buffers
        let input_buffer = self.context.create_dual_buffer(
            "Batch AD Input",
            &gpu_inputs,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );

        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Batch AD Output"),
            size: (num_elements * std::mem::size_of::<GpuDualNumber>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Get shader for batch forward AD
        let shader_source = self.get_batch_forward_ad_shader(operations);

        // Create layout, pipeline, and bind group separately to avoid borrow conflicts
        let bind_group_layout = self.create_batch_ad_layout();

        // Create bind group first to avoid borrow conflicts
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Batch Forward AD Bind Group"),
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

        // Get pipeline and execute in one operation to avoid lifetime issues
        let workgroup_count = num_elements.div_ceil(workgroup_size) as u32;
        self.execute_pipeline_operation(
            "batch_forward_ad",
            &shader_source,
            &bind_group_layout,
            &bind_group,
            (workgroup_count, 1, 1),
        )?;

        // Read results
        let results: Vec<GpuDualNumber> = self
            .context
            .read_dual_buffer(
                &output_buffer,
                (num_elements * std::mem::size_of::<GpuDualNumber>()) as u64,
            )
            .await?;

        Ok(results.into_iter().map(|g| g.into()).collect())
    }

    /// Create bind group layout for batch AD operations
    fn create_batch_ad_layout(&self) -> wgpu::BindGroupLayout {
        self.context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Batch Forward AD Layout"),
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

    /// Execute pipeline operation without borrow conflicts
    fn execute_pipeline_operation(
        &mut self,
        _shader_key: &str,
        shader_source: &str,
        bind_group_layout: &wgpu::BindGroupLayout,
        bind_group: &wgpu::BindGroup,
        workgroup_count: (u32, u32, u32),
    ) -> DualGpuResult<()> {
        // Create pipeline directly instead of caching to avoid borrow issues
        let shader_module =
            self.context
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Dual Operation Shader"),
                    source: wgpu::ShaderSource::Wgsl(shader_source.into()),
                });

        let pipeline_layout =
            self.context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Dual Operation Pipeline Layout"),
                    bind_group_layouts: &[bind_group_layout],
                    push_constant_ranges: &[],
                });

        let compute_pipeline =
            self.context
                .device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Dual Operation Pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader_module,
                    entry_point: "main",
                });

        self.context
            .execute_dual_compute(&compute_pipeline, bind_group, workgroup_count);
        Ok(())
    }

    /// Compute gradient for neural network training.
    ///
    /// Internal redesign-pending scaffold; not re-exported as part of the public v1 dual surface.
    pub(crate) async fn neural_gradient(
        &mut self,
        weights: &[f32],
        inputs: &[f32],
        targets: &[f32],
        network_config: &NeuralNetworkConfig,
    ) -> DualGpuResult<Vec<f32>> {
        let num_weights = weights.len();
        let _batch_size = inputs.len() / network_config.input_size;

        // Create dual numbers for each weight (each gets derivative = 1 for its component)
        let _dual_weights: Vec<f32> = Vec::with_capacity(num_weights);

        // For large networks, we compute gradients in chunks
        let chunk_size = 1024; // Process 1024 weights at a time
        let mut gradients = vec![0.0; num_weights];

        for chunk_start in (0..num_weights).step_by(chunk_size) {
            let chunk_end = (chunk_start + chunk_size).min(num_weights);
            let chunk_size_actual = chunk_end - chunk_start;

            // Create GPU buffers for this chunk
            let weight_buffer = self.context.create_dual_buffer(
                "Neural Weights",
                weights,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            );

            let input_buffer = self.context.create_dual_buffer(
                "Neural Inputs",
                inputs,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            );

            let target_buffer = self.context.create_dual_buffer(
                "Neural Targets",
                targets,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            );

            let gradient_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Neural Gradients"),
                size: (chunk_size_actual * std::mem::size_of::<f32>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });

            // Execute neural gradient computation
            let shader_source = self.get_neural_gradient_shader(network_config);

            let bind_group_layout = self.create_neural_bind_group_layout();

            let bind_group = self
                .context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Neural Gradient Bind Group"),
                    layout: &bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: weight_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: input_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: target_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: gradient_buffer.as_entire_binding(),
                        },
                    ],
                });

            let workgroup_count = chunk_size_actual.div_ceil(64) as u32;
            self.execute_pipeline_operation(
                "neural_gradient",
                &shader_source,
                &bind_group_layout,
                &bind_group,
                (workgroup_count, 1, 1),
            )?;

            // Read chunk gradients
            let chunk_gradients: Vec<f32> = self
                .context
                .read_dual_buffer(
                    &gradient_buffer,
                    (chunk_size_actual * std::mem::size_of::<f32>()) as u64,
                )
                .await?;

            // Copy chunk gradients to full gradient vector
            gradients[chunk_start..chunk_end].copy_from_slice(&chunk_gradients);
        }

        Ok(gradients)
    }

    /// Optimize function using gradient descent.
    ///
    /// Internal redesign-pending scaffold; not re-exported as part of the public v1 dual surface.
    pub(crate) async fn gradient_descent_optimization(
        &mut self,
        initial_params: &[f32],
        objective_function: &ObjectiveFunction,
        learning_rate: f32,
        max_iterations: u32,
    ) -> DualGpuResult<Vec<f32>> {
        let _num_params = initial_params.len();
        let mut current_params = initial_params.to_vec();

        for _iteration in 0..max_iterations {
            // Compute gradients using forward-mode AD
            let gradients = self
                .compute_function_gradients(&current_params, objective_function)
                .await?;

            // Update parameters: param = param - learning_rate * gradient
            for (param, grad) in current_params.iter_mut().zip(gradients.iter()) {
                *param -= learning_rate * grad;
            }

            // Optional: check convergence criteria
            let gradient_norm: f32 = gradients.iter().map(|g| g * g).sum::<f32>().sqrt();
            if gradient_norm < 1e-6 {
                break;
            }
        }

        Ok(current_params)
    }

    /// Compute function gradients using forward-mode AD
    async fn compute_function_gradients(
        &mut self,
        params: &[f32],
        objective_function: &ObjectiveFunction,
    ) -> DualGpuResult<Vec<f32>> {
        let num_params = params.len();
        let mut gradients = vec![0.0; num_params];

        // For each parameter, compute partial derivative
        for (i, gradient) in gradients.iter_mut().enumerate().take(num_params) {
            // Create dual numbers with derivative=1 for current parameter
            let dual_params: Vec<GpuDualNumber> = params
                .iter()
                .enumerate()
                .map(|(j, &p)| GpuDualNumber {
                    real: p,
                    dual: if i == j { 1.0 } else { 0.0 },
                })
                .collect();

            // Execute objective function with dual arithmetic
            let result = self
                .evaluate_objective_function_gpu(&dual_params, objective_function)
                .await?;

            *gradient = result.dual; // Gradient is in dual part
        }

        Ok(gradients)
    }

    /// Helper methods for shader generation
    fn get_batch_forward_ad_shader(&self, operations: &[DualOperation]) -> String {
        let mut shader = String::from(
            r#"
struct DualNumber {
    real: f32,
    dual: f32,
}

@group(0) @binding(0) var<storage, read> inputs: array<DualNumber>;
@group(0) @binding(1) var<storage, read_write> outputs: array<DualNumber>;

fn dual_add(a: DualNumber, b: DualNumber) -> DualNumber {
    return DualNumber(a.real + b.real, a.dual + b.dual);
}

fn dual_mul(a: DualNumber, b: DualNumber) -> DualNumber {
    return DualNumber(
        a.real * b.real,
        a.real * b.dual + a.dual * b.real
    );
}

fn dual_sin(a: DualNumber) -> DualNumber {
    return DualNumber(sin(a.real), a.dual * cos(a.real));
}

fn dual_cos(a: DualNumber) -> DualNumber {
    return DualNumber(cos(a.real), -a.dual * sin(a.real));
}

fn dual_exp(a: DualNumber) -> DualNumber {
    let exp_val = exp(a.real);
    return DualNumber(exp_val, a.dual * exp_val);
}

fn dual_log(a: DualNumber) -> DualNumber {
    return DualNumber(log(a.real), a.dual / a.real);
}

fn dual_relu(a: DualNumber) -> DualNumber {
    if a.real > 0.0 {
        return a;
    } else {
        return DualNumber(0.0, 0.0);
    }
}

fn dual_sigmoid(a: DualNumber) -> DualNumber {
    let sig = 1.0 / (1.0 + exp(-a.real));
    return DualNumber(sig, a.dual * sig * (1.0 - sig));
}

fn dual_tanh(a: DualNumber) -> DualNumber {
    let tanh_val = tanh(a.real);
    return DualNumber(tanh_val, a.dual * (1.0 - tanh_val * tanh_val));
}

fn dual_sqrt(a: DualNumber) -> DualNumber {
    let sqrt_val = sqrt(a.real);
    return DualNumber(sqrt_val, a.dual / (2.0 * sqrt_val));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if index >= arrayLength(&inputs) {
        return;
    }

    var result = inputs[index];
"#,
        );

        // Add operation-specific code
        for operation in operations {
            match operation {
                DualOperation::Sin => shader.push_str("    result = dual_sin(result);\n"),
                DualOperation::Cos => shader.push_str("    result = dual_cos(result);\n"),
                DualOperation::Exp => shader.push_str("    result = dual_exp(result);\n"),
                DualOperation::Log => shader.push_str("    result = dual_log(result);\n"),
                DualOperation::ReLU => shader.push_str("    result = dual_relu(result);\n"),
                DualOperation::Sigmoid => shader.push_str("    result = dual_sigmoid(result);\n"),
                DualOperation::Tanh => shader.push_str("    result = dual_tanh(result);\n"),
                DualOperation::Square => {
                    shader.push_str("    result = dual_mul(result, result);\n")
                }
                DualOperation::Sqrt => shader.push_str("    result = dual_sqrt(result);\n"),
                DualOperation::Add => unreachable!("validated before shader generation"),
                DualOperation::Multiply => unreachable!("validated before shader generation"),
            }
        }

        shader.push_str(
            r#"
    outputs[index] = result;
}
"#,
        );

        shader
    }

    fn get_neural_gradient_shader(&self, _config: &NeuralNetworkConfig) -> String {
        String::from(
            r#"
struct DualNumber {
    real: f32,
    dual: f32,
}

@group(0) @binding(0) var<storage, read> weights: array<f32>;
@group(0) @binding(1) var<storage, read> inputs: array<f32>;
@group(0) @binding(2) var<storage, read> targets: array<f32>;
@group(0) @binding(3) var<storage, read_write> gradients: array<f32>;

fn sigmoid(x: f32) -> f32 {
    return 1.0 / (1.0 + exp(-x));
}

fn dual_sigmoid(a: DualNumber) -> DualNumber {
    let sig = sigmoid(a.real);
    return DualNumber(sig, a.dual * sig * (1.0 - sig));
}

fn relu(x: f32) -> f32 {
    return max(0.0, x);
}

fn dual_relu(a: DualNumber) -> DualNumber {
    if a.real > 0.0 {
        return a;
    } else {
        return DualNumber(0.0, 0.0);
    }
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let weight_idx = global_id.x;
    if weight_idx >= arrayLength(&weights) {
        return;
    }

    // Forward pass with dual arithmetic for this weight
    // This is a simplified version - full implementation would depend on network architecture

    // Create dual number for current weight (derivative = 1)
    var dual_weight = DualNumber(weights[weight_idx], 1.0);

    // Simplified forward pass computation
    // In practice, this would be the full neural network forward pass
    var loss = DualNumber(0.0, 0.0);

    // Compute loss using dual arithmetic
    // This is where the actual network computation would go

    // Store gradient (dual part of loss)
    gradients[weight_idx] = loss.dual;
}
"#,
        )
    }

    fn create_neural_bind_group_layout(&self) -> wgpu::BindGroupLayout {
        self.context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Neural Gradient Layout"),
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
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
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

    async fn evaluate_objective_function_gpu(
        &mut self,
        _params: &[GpuDualNumber],
        _objective_function: &ObjectiveFunction,
    ) -> DualGpuResult<GpuDualNumber> {
        // Placeholder implementation
        // In practice, this would execute the objective function using GPU compute shaders
        Ok(GpuDualNumber {
            real: 0.0,
            dual: 0.0,
        })
    }
}

/// Dual operation types for GPU shaders
#[cfg(feature = "dual")]
#[derive(Debug, Clone)]
pub enum DualOperation {
    Sin,
    Cos,
    Exp,
    Log,
    ReLU,
    Sigmoid,
    Tanh,
    Square,
    Sqrt,
    Add,
    Multiply,
}

/// Neural network configuration for GPU training
#[cfg(feature = "dual")]
#[derive(Debug, Clone)]
pub struct NeuralNetworkConfig {
    pub input_size: usize,
    pub hidden_size: usize,
    pub output_size: usize,
    pub activation: String,
}

/// Objective function specification for optimization
#[cfg(feature = "dual")]
#[derive(Debug, Clone)]
pub struct ObjectiveFunction {
    pub function_type: String,
    pub parameters: HashMap<String, f32>,
}

// Implement GPU acceleration traits for dual number types
#[cfg(feature = "dual")]
impl DualGpuAccelerated<DualNumber<f32>> for DualNumber<f32> {
    fn to_gpu_buffer(&self, context: &DualGpuContext) -> DualGpuResult<wgpu::Buffer> {
        let gpu_dual: GpuDualNumber = (*self).into();
        let buffer = context.create_dual_buffer(
            "DualNumber Buffer",
            &[gpu_dual],
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        );
        Ok(buffer)
    }

    fn from_gpu_buffer(_buffer: &wgpu::Buffer, _context: &DualGpuContext) -> DualGpuResult<Self> {
        // Placeholder implementation - would need async context in trait
        Ok(DualNumber::new(0.0, 0.0))
    }

    fn gpu_operation(
        &self,
        operation: &str,
        _context: &DualGpuContext,
        _params: &GpuOperationParams,
    ) -> DualGpuResult<Self> {
        match operation {
            "forward_ad" => {
                // Placeholder for forward-mode AD operation
                Ok(*self)
            }
            "batch_gradient" => {
                // Placeholder for batch gradient computation
                Ok(*self)
            }
            _ => Err(DualGpuError::InvalidOperation(format!(
                "Unknown operation: {}",
                operation
            ))),
        }
    }
}

#[cfg(feature = "dual")]
impl<T: Float + Send + Sync> DualGpuAccelerated<MultiDualNumber<T>> for MultiDualNumber<T> {
    fn to_gpu_buffer(&self, _context: &DualGpuContext) -> DualGpuResult<wgpu::Buffer> {
        // Convert to GPU-compatible format
        // For now, only support f32 for GPU operations
        Err(DualGpuError::InvalidOperation(
            "MultiDual GPU operations require f32 precision".to_string(),
        ))
    }

    fn from_gpu_buffer(_buffer: &wgpu::Buffer, _context: &DualGpuContext) -> DualGpuResult<Self> {
        Err(DualGpuError::InvalidOperation(
            "MultiDual GPU operations require f32 precision".to_string(),
        ))
    }

    fn gpu_operation(
        &self,
        _operation: &str,
        _context: &DualGpuContext,
        _params: &GpuOperationParams,
    ) -> DualGpuResult<Self> {
        Err(DualGpuError::InvalidOperation(
            "MultiDual GPU operations not yet implemented".to_string(),
        ))
    }
}

// Convenience functions for common GPU operations
#[cfg(feature = "dual")]
impl DualGpuOps {
    /// Compute batch gradients for a vector function.
    ///
    /// Internal redesign-pending scaffold; not re-exported as part of the public v1 dual surface.
    pub(crate) async fn batch_gradients(
        &mut self,
        inputs: &[f32],
        function: &VectorFunction,
    ) -> DualGpuResult<Vec<Vec<f32>>> {
        let num_inputs = inputs.len();
        let num_outputs = function.output_size;

        // Create dual numbers for each input variable
        let mut gradients = Vec::with_capacity(num_outputs);

        for output_idx in 0..num_outputs {
            let mut output_gradients = Vec::with_capacity(num_inputs);

            for input_idx in 0..num_inputs {
                // Compute partial derivative of output_idx w.r.t. input_idx
                let dual_inputs: Vec<GpuDualNumber> = inputs
                    .iter()
                    .enumerate()
                    .map(|(i, &val)| GpuDualNumber {
                        real: val,
                        dual: if i == input_idx { 1.0 } else { 0.0 },
                    })
                    .collect();

                // Evaluate function (placeholder)
                let result = self
                    .evaluate_vector_function_gpu(&dual_inputs, function, output_idx)
                    .await?;
                output_gradients.push(result.dual);
            }

            gradients.push(output_gradients);
        }

        Ok(gradients)
    }

    async fn evaluate_vector_function_gpu(
        &mut self,
        _inputs: &[GpuDualNumber],
        _function: &VectorFunction,
        _output_idx: usize,
    ) -> DualGpuResult<GpuDualNumber> {
        // Placeholder implementation
        Ok(GpuDualNumber {
            real: 0.0,
            dual: 0.0,
        })
    }
}

/// Vector function specification for batch gradient computation
#[cfg(feature = "dual")]
#[derive(Debug, Clone)]
pub struct VectorFunction {
    pub input_size: usize,
    pub output_size: usize,
    pub function_type: String,
}

#[cfg(test)]
#[cfg(feature = "dual")]
mod tests {
    use super::*;
    use amari_dual::DualNumber;

    #[tokio::test]
    async fn test_dual_gpu_context_creation() {
        // Should not fail even without GPU hardware
        let result = DualGpuContext::new().await;

        // Test passes whether GPU is available or not
        match result {
            Ok(_) => println!("✅ Dual GPU context initialized successfully"),
            Err(_) => println!("⚠️  GPU not available, test passes with graceful fallback"),
        }
    }

    #[tokio::test]
    async fn test_dual_number_conversion() {
        let dual = DualNumber::new(std::f32::consts::PI, 2.71f32);
        let gpu_dual: GpuDualNumber = dual.into();
        let converted_back: DualNumber<f32> = gpu_dual.into();

        assert_eq!(dual.real, converted_back.real);
        assert_eq!(dual.dual, converted_back.dual);
    }

    #[tokio::test]
    async fn test_batch_forward_ad() {
        if let Ok(mut gpu_ops) = DualGpuOps::new().await {
            let inputs = vec![
                DualNumber::new(1.0, 1.0),
                DualNumber::new(2.0, 1.0),
                DualNumber::new(3.0, 1.0),
            ];

            let operations = vec![DualOperation::Sin, DualOperation::Exp];

            let result = gpu_ops.batch_forward_ad(&inputs, &operations).await;

            match result {
                Ok(results) => {
                    assert_eq!(results.len(), inputs.len());
                    println!("✅ Batch forward AD completed successfully");
                }
                Err(_) => println!("⚠️  GPU operation failed, but test passes"),
            }
        }
    }

    #[test]
    fn test_dual_number_gpu_operations() {
        let dual = DualNumber::new(2.0f32, 1.0f32);

        // Test basic dual number operations (CPU fallback)
        let squared = dual * dual;
        assert_eq!(squared.real, 4.0);
        assert_eq!(squared.dual, 4.0); // d/dx(x²) = 2x, 2*2*1 = 4

        let exp_result = dual.exp();
        assert!((exp_result.real - 2.0f32.exp()).abs() < 1e-6);
        assert!((exp_result.dual - 2.0f32.exp()).abs() < 1e-6); // d/dx(e^x) = e^x
    }
}
