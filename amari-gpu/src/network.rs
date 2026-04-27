//! GPU-accelerated geometric network analysis
//!
//! This module provides GPU acceleration for network analysis operations.
//!
//! The current GPU path is intentionally narrow and honest: it accelerates
//! pairwise Euclidean distances for vector-only `Cl(P,0,0)` embeddings with
//! `P <= 3`. Centrality and clustering reuse those GPU distances but complete
//! their reductions/medoid updates on the CPU. Adaptive dispatch falls back to
//! the `amari-network` CPU geometric-distance baseline for unsupported
//! signatures, non-vector multivectors, or small networks.

use crate::GpuError;
use amari_network::{Community, GeometricNetwork};
use bytemuck::{Pod, Zeroable};
use futures::channel::oneshot;
use thiserror::Error;
use wgpu::util::DeviceExt;

#[derive(Error, Debug)]
pub enum GpuNetworkError {
    #[error("GPU error: {0}")]
    Gpu(#[from] GpuError),

    #[error("Network error: {0}")]
    Network(#[from] amari_network::NetworkError),

    #[error("Invalid network size: {0}")]
    InvalidSize(usize),

    #[error("Unsupported GPU embedding: {0}")]
    UnsupportedEmbedding(String),

    #[error("Invalid position: {0}")]
    InvalidPosition(String),

    #[error("Buffer error: {0}")]
    BufferError(String),
}

pub type GpuNetworkResult<T> = Result<T, GpuNetworkError>;

/// GPU-accelerated geometric network analysis
pub struct GpuGeometricNetwork {
    device: wgpu::Device,
    queue: wgpu::Queue,
    distance_pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)]
    centrality_pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)]
    clustering_pipeline: wgpu::ComputePipeline,
}

/// Node position data for GPU computation
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GpuNodePosition {
    x: f32,
    y: f32,
    z: f32,
    padding: f32, // For 16-byte alignment
}

/// Edge data for GPU computation
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GpuEdgeData {
    source: u32,
    target: u32,
    weight: f32,
    padding: f32,
}

impl GpuGeometricNetwork {
    /// Initialize GPU context for network analysis
    pub async fn new() -> GpuNetworkResult<Self> {
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
                    label: Some("Amari Network GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::InitializationError(e.to_string()))?;

        let distance_pipeline = Self::create_distance_pipeline(&device)?;
        let centrality_pipeline = Self::create_centrality_pipeline(&device)?;
        let clustering_pipeline = Self::create_clustering_pipeline(&device)?;

        Ok(Self {
            device,
            queue,
            distance_pipeline,
            centrality_pipeline,
            clustering_pipeline,
        })
    }

    /// Compute all pairwise Euclidean distances using GPU acceleration.
    ///
    /// This GPU path supports vector-only `Cl(P,0,0)` embeddings with `P <= 3`.
    /// Use [`AdaptiveNetworkCompute`] for automatic CPU fallback on unsupported
    /// signatures or non-vector multivectors.
    pub async fn compute_all_pairwise_distances<const P: usize, const Q: usize, const R: usize>(
        &self,
        network: &GeometricNetwork<P, Q, R>,
    ) -> GpuNetworkResult<Vec<Vec<f64>>> {
        let num_nodes = network.num_nodes();
        if num_nodes == 0 {
            return Ok(Vec::new());
        }
        self.validate_gpu_distance_network(network)?;

        // Convert node positions to GPU format
        let gpu_positions: Vec<GpuNodePosition> = (0..num_nodes)
            .map(|i| {
                let pos = network.get_node(i).unwrap();
                GpuNodePosition {
                    x: pos.vector_component(0) as f32,
                    y: pos.vector_component(1) as f32,
                    z: pos.vector_component(2) as f32,
                    padding: 0.0,
                }
            })
            .collect();

        // Create GPU buffers
        let positions_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Node Positions"),
                contents: bytemuck::cast_slice(&gpu_positions),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let output_size = num_nodes * num_nodes * 4; // f32 = 4 bytes
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Distance Output"),
            size: output_size as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Distance Staging"),
            size: output_size as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.distance_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Distance Compute Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: positions_buffer.as_entire_binding(),
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
                label: Some("Distance Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Distance Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.distance_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            let workgroup_count = num_nodes.div_ceil(8);
            compute_pass.dispatch_workgroups(workgroup_count as u32, workgroup_count as u32, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size as u64);

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
                GpuNetworkError::BufferError("Failed to receive buffer mapping".to_string())
            })?
            .map_err(|e| GpuNetworkError::BufferError(format!("Buffer mapping failed: {:?}", e)))?;

        let data = buffer_slice.get_mapped_range();
        let result_f32: &[f32] = bytemuck::cast_slice(&data);

        // Convert back to nested Vec<Vec<f64>>
        let mut distances = vec![vec![0.0; num_nodes]; num_nodes];
        for i in 0..num_nodes {
            for j in 0..num_nodes {
                distances[i][j] = result_f32[i * num_nodes + j] as f64;
            }
        }

        drop(data);
        staging_buffer.unmap();

        Ok(distances)
    }

    /// Compute geometric centrality using GPU distances plus CPU reduction.
    pub async fn compute_geometric_centrality<const P: usize, const Q: usize, const R: usize>(
        &self,
        network: &GeometricNetwork<P, Q, R>,
    ) -> GpuNetworkResult<Vec<f64>> {
        // For centrality, we need the distance matrix first
        let distances = self.compute_all_pairwise_distances(network).await?;

        // For now, use CPU computation for centrality based on GPU distances
        // In a full implementation, this would also be GPU-accelerated
        let num_nodes = network.num_nodes();
        let mut centrality = vec![0.0; num_nodes];

        for i in 0..num_nodes {
            let total_distance: f64 = distances[i].iter().sum();
            centrality[i] = if total_distance > 0.0 {
                (num_nodes as f64 - 1.0) / total_distance
            } else {
                0.0
            };
        }

        Ok(centrality)
    }

    /// GPU-distance-assisted k-medoids clustering for community detection.
    ///
    /// Pairwise distances are computed by the GPU path; assignment, medoid
    /// updates, and cohesion scoring are CPU-side for 0.20.0.
    pub async fn geometric_clustering<const P: usize, const Q: usize, const R: usize>(
        &self,
        network: &GeometricNetwork<P, Q, R>,
        k: usize,
        max_iterations: usize,
    ) -> GpuNetworkResult<Vec<Community<P, Q, R>>> {
        let num_nodes = network.num_nodes();
        if k > num_nodes || k == 0 {
            return Err(GpuNetworkError::InvalidSize(k));
        }
        if max_iterations == 0 {
            return Err(GpuNetworkError::InvalidSize(max_iterations));
        }

        // For simplicity, use CPU-based k-means with GPU distance calculations
        let distances = self.compute_all_pairwise_distances(network).await?;

        // Initialize centroids (use first k nodes)
        let mut centroids = Vec::with_capacity(k);
        for i in 0..k {
            let centroid_idx = (i * num_nodes) / k;
            centroids.push(centroid_idx);
        }

        let mut assignments = vec![0; num_nodes];

        for _iteration in 0..max_iterations {
            let mut changed = false;

            // Assign nodes to nearest centroid
            for node in 0..num_nodes {
                let mut best_cluster = 0;
                let mut best_distance = f64::INFINITY;

                for (cluster, &centroid) in centroids.iter().enumerate().take(k) {
                    let distance = distances[node][centroid];

                    if distance < best_distance {
                        best_distance = distance;
                        best_cluster = cluster;
                    }
                }

                if assignments[node] != best_cluster {
                    assignments[node] = best_cluster;
                    changed = true;
                }
            }

            if !changed {
                break;
            }

            // Update centroids (find medoid of each cluster)
            for (cluster, centroid) in centroids.iter_mut().enumerate().take(k) {
                let cluster_nodes: Vec<usize> = assignments
                    .iter()
                    .enumerate()
                    .filter(|(_, &c)| c == cluster)
                    .map(|(node, _)| node)
                    .collect();

                if !cluster_nodes.is_empty() {
                    let mut best_medoid = cluster_nodes[0];
                    let mut best_total_distance = f64::INFINITY;

                    for &candidate in &cluster_nodes {
                        let total_distance: f64 = cluster_nodes
                            .iter()
                            .map(|&other| distances[candidate][other])
                            .sum();

                        if total_distance < best_total_distance {
                            best_total_distance = total_distance;
                            best_medoid = candidate;
                        }
                    }

                    *centroid = best_medoid;
                }
            }
        }

        // Convert assignments to communities
        let mut communities = Vec::with_capacity(k);
        for (cluster, &centroid) in centroids.iter().enumerate().take(k) {
            let nodes: Vec<usize> = assignments
                .iter()
                .enumerate()
                .filter(|(_, &c)| c == cluster)
                .map(|(node, _)| node)
                .collect();

            if !nodes.is_empty() {
                let centroid_pos = network.get_node(centroid).unwrap().clone();
                let cohesion_score = Self::compute_cluster_cohesion(&nodes, &distances);
                communities.push(Community {
                    nodes,
                    geometric_centroid: centroid_pos,
                    cohesion_score,
                });
            }
        }

        Ok(communities)
    }

    /// Determine if GPU acceleration should be used based on network size.
    pub fn should_use_gpu(num_nodes: usize) -> bool {
        // GPU is beneficial for networks with many nodes
        num_nodes >= 100
    }

    /// Return whether this signature can use the current GPU distance kernel.
    pub fn supports_gpu_distance<const P: usize, const Q: usize, const R: usize>() -> bool {
        Q == 0 && R == 0 && P <= 3
    }

    // Private helper methods

    fn validate_gpu_distance_network<const P: usize, const Q: usize, const R: usize>(
        &self,
        network: &GeometricNetwork<P, Q, R>,
    ) -> GpuNetworkResult<()> {
        if !Self::supports_gpu_distance::<P, Q, R>() {
            return Err(GpuNetworkError::UnsupportedEmbedding(format!(
                "GPU network distance supports vector-only Cl(P,0,0) with P <= 3; got Cl({P},{Q},{R})"
            )));
        }

        for node_idx in 0..network.num_nodes() {
            let node = network.get_node(node_idx).ok_or_else(|| {
                GpuNetworkError::InvalidPosition(format!("node {node_idx} is missing"))
            })?;
            for (coeff_idx, &coeff) in node.as_slice().iter().enumerate() {
                if !coeff.is_finite() {
                    return Err(GpuNetworkError::InvalidPosition(format!(
                        "node {node_idx} coefficient {coeff_idx} is not finite"
                    )));
                }
                let is_vector_component = coeff_idx.is_power_of_two()
                    && coeff_idx > 0
                    && coeff_idx.trailing_zeros() < P as u32;
                if !is_vector_component && coeff.abs() > 1e-12 {
                    return Err(GpuNetworkError::UnsupportedEmbedding(format!(
                        "node {node_idx} has non-vector coefficient at blade {coeff_idx}"
                    )));
                }
            }
        }

        Ok(())
    }

    fn compute_cluster_cohesion(nodes: &[usize], distances: &[Vec<f64>]) -> f64 {
        if nodes.len() <= 1 {
            return 1.0;
        }

        let mut total = 0.0;
        let mut count = 0usize;
        for (idx, &a) in nodes.iter().enumerate() {
            for &b in nodes.iter().skip(idx + 1) {
                total += distances[a][b];
                count += 1;
            }
        }

        if count == 0 {
            1.0
        } else {
            1.0 / (1.0 + total / count as f64)
        }
    }

    fn create_distance_pipeline(device: &wgpu::Device) -> Result<wgpu::ComputePipeline, GpuError> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Distance Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(DISTANCE_COMPUTE_SHADER.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Distance Compute Pipeline"),
            layout: None,
            module: &shader,
            entry_point: "main",
        });

        Ok(pipeline)
    }

    fn create_centrality_pipeline(
        device: &wgpu::Device,
    ) -> Result<wgpu::ComputePipeline, GpuError> {
        // For now, reuse distance pipeline
        Self::create_distance_pipeline(device)
    }

    fn create_clustering_pipeline(
        device: &wgpu::Device,
    ) -> Result<wgpu::ComputePipeline, GpuError> {
        // For now, reuse distance pipeline
        Self::create_distance_pipeline(device)
    }
}

/// WGSL compute shader for pairwise distance calculations
const DISTANCE_COMPUTE_SHADER: &str = r#"
struct NodePosition {
    x: f32,
    y: f32,
    z: f32,
    padding: f32,
}

@group(0) @binding(0)
var<storage, read> positions: array<NodePosition>;

@group(0) @binding(1)
var<storage, read_write> distances: array<f32>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let i = global_id.x;
    let j = global_id.y;
    let num_nodes = arrayLength(&positions);

    if (i >= num_nodes || j >= num_nodes) {
        return;
    }

    let idx = i * num_nodes + j;

    if (i == j) {
        distances[idx] = 0.0;
        return;
    }

    let pos_i = positions[i];
    let pos_j = positions[j];

    let dx = pos_i.x - pos_j.x;
    let dy = pos_i.y - pos_j.y;
    let dz = pos_i.z - pos_j.z;

    let distance = sqrt(dx * dx + dy * dy + dz * dz);
    distances[idx] = distance;
}
"#;

/// Adaptive GPU/CPU dispatcher for network operations
pub struct AdaptiveNetworkCompute {
    gpu: Option<GpuGeometricNetwork>,
}

impl AdaptiveNetworkCompute {
    fn compute_pairwise_geometric_distances_cpu<const P: usize, const Q: usize, const R: usize>(
        network: &GeometricNetwork<P, Q, R>,
    ) -> GpuNetworkResult<Vec<Vec<f64>>> {
        let num_nodes = network.num_nodes();
        let mut distances = vec![vec![0.0; num_nodes]; num_nodes];

        for (i, row) in distances.iter_mut().enumerate() {
            for (j, distance) in row.iter_mut().enumerate() {
                *distance = network.geometric_distance(i, j)?;
            }
        }

        Ok(distances)
    }

    /// Create with optional GPU acceleration
    pub async fn new() -> Self {
        // Use panic-safe GPU detection like in adaptive verification
        let gpu = {
            let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                pollster::block_on(async { GpuGeometricNetwork::new().await.ok() })
            }));

            // GPU initialization panicked or failed - gracefully fall back to CPU
            panic_result.unwrap_or_default()
        };

        Self { gpu }
    }

    /// Compute pairwise distances with adaptive dispatch
    pub async fn compute_all_pairwise_distances<const P: usize, const Q: usize, const R: usize>(
        &self,
        network: &GeometricNetwork<P, Q, R>,
    ) -> GpuNetworkResult<Vec<Vec<f64>>> {
        let num_nodes = network.num_nodes();

        if let Some(gpu) = &self.gpu {
            if GpuGeometricNetwork::should_use_gpu(num_nodes)
                && GpuGeometricNetwork::supports_gpu_distance::<P, Q, R>()
            {
                if let Ok(distances) = gpu.compute_all_pairwise_distances(network).await {
                    return Ok(distances);
                }
            }
        }

        // CPU geometric-distance fallback. Do not use graph shortest paths here:
        // this API returns geometric distances between embedded node positions.
        Self::compute_pairwise_geometric_distances_cpu(network)
    }

    /// Compute centrality with adaptive dispatch
    pub async fn compute_geometric_centrality<const P: usize, const Q: usize, const R: usize>(
        &self,
        network: &GeometricNetwork<P, Q, R>,
    ) -> GpuNetworkResult<Vec<f64>> {
        let num_nodes = network.num_nodes();

        if let Some(gpu) = &self.gpu {
            if GpuGeometricNetwork::should_use_gpu(num_nodes)
                && GpuGeometricNetwork::supports_gpu_distance::<P, Q, R>()
            {
                if let Ok(centrality) = gpu.compute_geometric_centrality(network).await {
                    return Ok(centrality);
                }
            }
        }

        // CPU fallback
        network
            .compute_geometric_centrality()
            .map_err(GpuNetworkError::Network)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_use_gpu() {
        assert!(!GpuGeometricNetwork::should_use_gpu(10));
        assert!(GpuGeometricNetwork::should_use_gpu(1000));
    }

    #[tokio::test]
    async fn test_adaptive_network_creation() {
        // Test adaptive behavior: should work with or without GPU
        let adaptive = AdaptiveNetworkCompute::new().await;

        // Should always succeed - adaptive design gracefully falls back to CPU
        match &adaptive.gpu {
            Some(_) => {
                println!("✅ GPU network acceleration available");
            }
            None => {
                println!("✅ GPU not available, using CPU fallback for network operations");
            }
        }

        // The adaptive compute should be created successfully regardless of GPU availability
        // This tests the core adaptive design principle
    }
}
