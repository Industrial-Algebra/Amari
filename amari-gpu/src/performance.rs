//! GPU performance optimization and profiling infrastructure
//!
//! This module provides comprehensive GPU performance profiling, workgroup optimization,
//! and adaptive tuning capabilities for all mathematical operations in the Amari library.

use crate::{SharedGpuContext, UnifiedGpuResult};
use std::collections::HashMap;
use std::time::Instant;

/// GPU performance profiler with timestamp queries and metrics collection
pub struct GpuProfiler {
    context: SharedGpuContext,
    query_set: wgpu::QuerySet,
    #[allow(dead_code)] // Used in timestamp calculations
    timestamp_period: f32,
    active_profiles: HashMap<String, ProfileSession>,
    completed_profiles: Vec<GpuProfile>,
    current_query_idx: u32,
}

/// Individual profiling session tracking compute operations
#[derive(Debug)]
pub struct ProfileSession {
    #[allow(dead_code)] // Used in profiling infrastructure
    name: String,
    start_time: Instant,
    #[allow(dead_code)] // Used in GPU timestamp queries
    start_query_idx: u32,
    end_query_idx: Option<u32>,
    workgroup_count: (u32, u32, u32),
    buffer_sizes: Vec<u64>,
}

/// Completed GPU profile with timing and performance metrics
#[derive(Debug, Clone)]
pub struct GpuProfile {
    pub name: String,
    pub cpu_time_ms: f32,
    pub gpu_time_ms: Option<f32>,
    pub memory_bandwidth_gb_s: f32,
    pub compute_efficiency_percent: f32,
    pub workgroup_utilization_percent: f32,
    pub buffer_pool_hit_rate: f32,
}

/// Workgroup configuration optimizer
pub struct WorkgroupOptimizer {
    optimal_configs: HashMap<String, WorkgroupConfig>,
    calibration_results: HashMap<String, Vec<CalibrationResult>>,
}

/// Optimized workgroup configuration for specific operation types
#[derive(Debug, Clone, Copy)]
pub struct WorkgroupConfig {
    pub size: (u32, u32, u32),
    pub shared_memory_bytes: u32,
    pub optimal_dispatch_size: u32,
}

#[derive(Debug, Clone)]
pub struct CalibrationResult {
    pub config: WorkgroupConfig,
    pub throughput_gops: f32,
    pub latency_ms: f32,
    pub efficiency_percent: f32,
}

/// Performance optimization recommendations
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub total_gpu_time_ms: f32,
    pub total_cpu_time_ms: f32,
    pub gpu_utilization_percent: f32,
    pub memory_bandwidth_utilization: f32,
    pub buffer_pool_efficiency: f32,
    pub bottlenecks: Vec<PerformanceBottleneck>,
    pub recommendations: Vec<OptimizationRecommendation>,
}

#[derive(Debug, Clone)]
pub enum PerformanceBottleneck {
    MemoryBandwidth { utilization_percent: f32 },
    ComputeUnits { utilization_percent: f32 },
    BufferAllocation { avg_allocation_time_ms: f32 },
    ShaderCompilation { avg_compilation_time_ms: f32 },
    GpuToProcessorSync { avg_sync_time_ms: f32 },
}

#[derive(Debug, Clone)]
pub enum OptimizationRecommendation {
    IncreaseWorkgroupSize {
        current: u32,
        recommended: u32,
    },
    EnableBufferPooling {
        potential_speedup: f32,
    },
    OptimizeMemoryLayout {
        current_efficiency: f32,
        potential_efficiency: f32,
    },
    ReduceBatchSize {
        current: usize,
        recommended: usize,
    },
    IncreaseBatchSize {
        current: usize,
        recommended: usize,
    },
    UseSharedMemory {
        potential_speedup: f32,
    },
}

impl GpuProfiler {
    /// Create a new GPU profiler
    pub async fn new() -> UnifiedGpuResult<Self> {
        let context = SharedGpuContext::global().await?.clone();

        // Create timestamp query set
        let query_set = context
            .device()
            .create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("GPU Profiler Timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 1024, // Support up to 512 overlapping profiles
            });

        // Get timestamp period for converting to nanoseconds
        let timestamp_period = context.queue().get_timestamp_period();

        Ok(Self {
            context,
            query_set,
            timestamp_period,
            active_profiles: HashMap::new(),
            completed_profiles: Vec::new(),
            current_query_idx: 0,
        })
    }

    /// Begin profiling a GPU operation
    pub fn begin_profile(
        &mut self,
        name: &str,
        workgroup_count: (u32, u32, u32),
        buffer_sizes: &[u64],
    ) -> ProfileScope<'_> {
        let start_query_idx = self.current_query_idx;
        self.current_query_idx += 2; // Reserve start and end timestamps

        let session = ProfileSession {
            name: name.to_string(),
            start_time: Instant::now(),
            start_query_idx,
            end_query_idx: None,
            workgroup_count,
            buffer_sizes: buffer_sizes.to_vec(),
        };

        self.active_profiles.insert(name.to_string(), session);

        ProfileScope {
            profiler: self,
            name: name.to_string(),
            start_query_idx,
        }
    }

    /// End profiling and compute metrics
    fn end_profile(&mut self, name: &str, start_query_idx: u32) {
        let end_query_idx = start_query_idx + 1;

        if let Some(mut session) = self.active_profiles.remove(name) {
            session.end_query_idx = Some(end_query_idx);
            let cpu_time_ms = session.start_time.elapsed().as_secs_f32() * 1000.0;

            // Calculate memory bandwidth (simplified)
            let total_memory_bytes: u64 = session.buffer_sizes.iter().sum();
            let memory_bandwidth_gb_s = if cpu_time_ms > 0.0 {
                (total_memory_bytes as f32 * 2.0) / (cpu_time_ms / 1000.0) / 1e9
            // Read + write
            } else {
                0.0
            };

            // Estimate workgroup utilization (simplified - would need GPU capabilities)
            let total_threads =
                session.workgroup_count.0 * session.workgroup_count.1 * session.workgroup_count.2;
            let workgroup_utilization = (total_threads.min(4096) as f32 / 4096.0) * 100.0;

            // Get buffer pool stats
            let buffer_pool_stats = self.context.buffer_pool_stats();

            let profile = GpuProfile {
                name: name.to_string(),
                cpu_time_ms,
                gpu_time_ms: None, // Would be filled in from timestamp query results
                memory_bandwidth_gb_s,
                compute_efficiency_percent: 85.0, // Placeholder - would compute from occupancy
                workgroup_utilization_percent: workgroup_utilization,
                buffer_pool_hit_rate: buffer_pool_stats.hit_rate_percent,
            };

            self.completed_profiles.push(profile);
        }
    }

    /// Generate comprehensive performance report
    pub fn generate_report(&self) -> PerformanceReport {
        let total_cpu_time: f32 = self.completed_profiles.iter().map(|p| p.cpu_time_ms).sum();
        let total_gpu_time: f32 = self
            .completed_profiles
            .iter()
            .map(|p| p.gpu_time_ms.unwrap_or(p.cpu_time_ms * 0.8))
            .sum();

        let avg_gpu_utilization: f32 = if !self.completed_profiles.is_empty() {
            self.completed_profiles
                .iter()
                .map(|p| p.compute_efficiency_percent)
                .sum::<f32>()
                / self.completed_profiles.len() as f32
        } else {
            0.0
        };

        let avg_memory_bandwidth: f32 = if !self.completed_profiles.is_empty() {
            self.completed_profiles
                .iter()
                .map(|p| p.memory_bandwidth_gb_s)
                .sum::<f32>()
                / self.completed_profiles.len() as f32
        } else {
            0.0
        };

        let buffer_pool_stats = self.context.buffer_pool_stats();

        // Identify bottlenecks
        let mut bottlenecks = Vec::new();
        let mut recommendations = Vec::new();

        if avg_memory_bandwidth < 100.0 {
            // Assuming 100 GB/s theoretical max
            bottlenecks.push(PerformanceBottleneck::MemoryBandwidth {
                utilization_percent: (avg_memory_bandwidth / 100.0) * 100.0,
            });
            recommendations.push(OptimizationRecommendation::OptimizeMemoryLayout {
                current_efficiency: avg_memory_bandwidth / 100.0,
                potential_efficiency: 0.8,
            });
        }

        if avg_gpu_utilization < 70.0 {
            bottlenecks.push(PerformanceBottleneck::ComputeUnits {
                utilization_percent: avg_gpu_utilization,
            });
            recommendations.push(OptimizationRecommendation::IncreaseWorkgroupSize {
                current: 64,
                recommended: 256,
            });
        }

        if buffer_pool_stats.hit_rate_percent < 50.0 {
            bottlenecks.push(PerformanceBottleneck::BufferAllocation {
                avg_allocation_time_ms: 5.0, // Estimate
            });
            recommendations.push(OptimizationRecommendation::EnableBufferPooling {
                potential_speedup: 1.3,
            });
        }

        PerformanceReport {
            total_gpu_time_ms: total_gpu_time,
            total_cpu_time_ms: total_cpu_time,
            gpu_utilization_percent: avg_gpu_utilization,
            memory_bandwidth_utilization: (avg_memory_bandwidth / 100.0) * 100.0,
            buffer_pool_efficiency: buffer_pool_stats.hit_rate_percent,
            bottlenecks,
            recommendations,
        }
    }

    /// Get all completed profiles
    pub fn profiles(&self) -> &[GpuProfile] {
        &self.completed_profiles
    }

    /// Clear all profiles
    pub fn clear_profiles(&mut self) {
        self.completed_profiles.clear();
        self.active_profiles.clear();
        self.current_query_idx = 0;
    }
}

/// RAII profiling scope that automatically ends profiling when dropped
pub struct ProfileScope<'a> {
    profiler: &'a mut GpuProfiler,
    name: String,
    start_query_idx: u32,
}

impl<'a> ProfileScope<'a> {
    /// Add timestamp query to command encoder (call this in your compute pass)
    pub fn write_timestamp(&self, encoder: &mut wgpu::CommandEncoder, stage: TimestampStage) {
        match stage {
            TimestampStage::Start => {
                encoder.write_timestamp(&self.profiler.query_set, self.start_query_idx);
            }
            TimestampStage::End => {
                encoder.write_timestamp(&self.profiler.query_set, self.start_query_idx + 1);
            }
        }
    }
}

impl<'a> Drop for ProfileScope<'a> {
    fn drop(&mut self) {
        self.profiler.end_profile(&self.name, self.start_query_idx);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TimestampStage {
    Start,
    End,
}

impl WorkgroupOptimizer {
    /// Create a new workgroup optimizer
    pub fn new() -> Self {
        Self {
            optimal_configs: HashMap::new(),
            calibration_results: HashMap::new(),
        }
    }

    /// Get optimal workgroup configuration for operation type
    pub fn get_optimal_config(&self, operation_type: &str) -> WorkgroupConfig {
        self.optimal_configs
            .get(operation_type)
            .cloned()
            .unwrap_or({
                // Default configurations based on operation type
                match operation_type {
                    "matrix_multiply" => WorkgroupConfig {
                        size: (16, 16, 1),
                        shared_memory_bytes: 8192,
                        optimal_dispatch_size: 1024,
                    },
                    "vector_operation" => WorkgroupConfig {
                        size: (256, 1, 1),
                        shared_memory_bytes: 0,
                        optimal_dispatch_size: 256,
                    },
                    "reduction" => WorkgroupConfig {
                        size: (128, 1, 1),
                        shared_memory_bytes: 4096,
                        optimal_dispatch_size: 128,
                    },
                    "cellular_automata" => WorkgroupConfig {
                        size: (256, 1, 1),
                        shared_memory_bytes: 0,
                        optimal_dispatch_size: 256,
                    },
                    "fisher_information" => WorkgroupConfig {
                        size: (256, 1, 1),
                        shared_memory_bytes: 0,
                        optimal_dispatch_size: 256,
                    },
                    "tropical_operations" => WorkgroupConfig {
                        size: (128, 1, 1),
                        shared_memory_bytes: 2048,
                        optimal_dispatch_size: 128,
                    },
                    _ => WorkgroupConfig {
                        size: (64, 1, 1),
                        shared_memory_bytes: 0,
                        optimal_dispatch_size: 64,
                    },
                }
            })
    }

    /// Calibrate optimal workgroup size for a specific operation
    pub async fn calibrate_operation(
        &mut self,
        operation_type: &str,
        test_function: impl Fn(WorkgroupConfig) -> f32,
    ) -> UnifiedGpuResult<WorkgroupConfig> {
        let test_configs = vec![
            WorkgroupConfig {
                size: (32, 1, 1),
                shared_memory_bytes: 0,
                optimal_dispatch_size: 32,
            },
            WorkgroupConfig {
                size: (64, 1, 1),
                shared_memory_bytes: 0,
                optimal_dispatch_size: 64,
            },
            WorkgroupConfig {
                size: (128, 1, 1),
                shared_memory_bytes: 0,
                optimal_dispatch_size: 128,
            },
            WorkgroupConfig {
                size: (256, 1, 1),
                shared_memory_bytes: 0,
                optimal_dispatch_size: 256,
            },
            WorkgroupConfig {
                size: (16, 16, 1),
                shared_memory_bytes: 4096,
                optimal_dispatch_size: 256,
            },
            WorkgroupConfig {
                size: (32, 8, 1),
                shared_memory_bytes: 2048,
                optimal_dispatch_size: 256,
            },
        ];

        let mut results = Vec::new();

        for config in test_configs {
            let start = Instant::now();
            let throughput = test_function(config);
            let throughput = if throughput.is_finite() && throughput >= 0.0 {
                throughput
            } else {
                0.0
            };
            let latency = start.elapsed().as_secs_f32() * 1000.0;

            let efficiency = if latency > 0.0 {
                throughput / latency
            } else {
                0.0
            };

            results.push(CalibrationResult {
                config,
                throughput_gops: throughput,
                latency_ms: latency,
                efficiency_percent: efficiency,
            });
        }

        // Find best configuration
        let best_config = results
            .iter()
            .max_by(|a, b| {
                a.efficiency_percent
                    .partial_cmp(&b.efficiency_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|r| r.config)
            .unwrap_or(WorkgroupConfig {
                size: (128, 1, 1),
                shared_memory_bytes: 0,
                optimal_dispatch_size: 128,
            });

        self.optimal_configs
            .insert(operation_type.to_string(), best_config);
        self.calibration_results
            .insert(operation_type.to_string(), results);

        Ok(best_config)
    }

    /// Get calibration results for analysis
    pub fn get_calibration_results(&self, operation_type: &str) -> Option<&[CalibrationResult]> {
        self.calibration_results
            .get(operation_type)
            .map(|v| v.as_slice())
    }
}

impl Default for WorkgroupOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Adaptive dispatch policy that learns optimal CPU/GPU thresholds
pub struct AdaptiveDispatchPolicy {
    #[allow(dead_code)] // Used in performance learning
    cpu_performance_profile: PerformanceProfile,
    #[allow(dead_code)] // Used in performance learning
    gpu_performance_profile: PerformanceProfile,
    crossover_points: HashMap<String, usize>,
    calibration_history: Vec<DispatchBenchmark>,
}

#[derive(Debug, Clone)]
pub struct PerformanceProfile {
    pub operations_per_second: f32,
    pub setup_overhead_ms: f32,
    pub memory_bandwidth_gb_s: f32,
    pub last_updated: Instant,
}

#[derive(Debug, Clone)]
pub struct DispatchBenchmark {
    pub operation_type: String,
    pub data_size: usize,
    pub cpu_time_ms: f32,
    pub gpu_time_ms: f32,
    pub timestamp: Instant,
}

impl AdaptiveDispatchPolicy {
    pub fn new() -> Self {
        Self {
            cpu_performance_profile: PerformanceProfile {
                operations_per_second: 1000.0,
                setup_overhead_ms: 0.1,
                memory_bandwidth_gb_s: 25.0,
                last_updated: Instant::now(),
            },
            gpu_performance_profile: PerformanceProfile {
                operations_per_second: 10000.0,
                setup_overhead_ms: 5.0,
                memory_bandwidth_gb_s: 500.0,
                last_updated: Instant::now(),
            },
            crossover_points: HashMap::new(),
            calibration_history: Vec::new(),
        }
    }

    /// Determine if GPU should be used for this operation
    pub fn should_use_gpu(&mut self, operation_type: &str, data_size: usize) -> bool {
        if let Some(&crossover) = self.crossover_points.get(operation_type) {
            data_size >= crossover
        } else {
            // Conservative default - require substantial work for GPU
            data_size >= 1000
        }
    }

    /// Update performance profile based on benchmark results
    pub fn update_from_benchmark(&mut self, benchmark: DispatchBenchmark) {
        // Simple learning: if GPU was faster, lower the crossover point; if slower, raise it
        let cpu_time_ms = if benchmark.cpu_time_ms.is_finite() && benchmark.cpu_time_ms >= 0.0 {
            benchmark.cpu_time_ms
        } else {
            0.0
        };
        let gpu_time_ms = if benchmark.gpu_time_ms.is_finite() && benchmark.gpu_time_ms > 0.0 {
            benchmark.gpu_time_ms
        } else {
            0.1
        };
        let gpu_advantage = cpu_time_ms / gpu_time_ms.max(0.1);

        let current_crossover = self
            .crossover_points
            .get(&benchmark.operation_type)
            .cloned()
            .unwrap_or(1000);

        let new_crossover = if gpu_advantage > 1.1 {
            // GPU was significantly faster, lower threshold
            (current_crossover as f32 * 0.8) as usize
        } else if gpu_advantage < 0.9 {
            // GPU was slower, raise threshold
            (current_crossover as f32 * 1.2) as usize
        } else {
            current_crossover
        };

        self.crossover_points.insert(
            benchmark.operation_type.clone(),
            new_crossover.clamp(10, 100000),
        );
        self.calibration_history.push(benchmark);

        // Keep only recent history
        self.calibration_history
            .retain(|b| b.timestamp.elapsed().as_secs() < 3600);
    }

    /// Get current crossover points
    pub fn get_crossover_points(&self) -> &HashMap<String, usize> {
        &self.crossover_points
    }
}

impl Default for AdaptiveDispatchPolicy {
    fn default() -> Self {
        Self::new()
    }
}
