//! GPU Timeline Analysis and Performance Profiling Infrastructure
//!
//! This module provides advanced GPU profiling capabilities with timeline analysis,
//! bottleneck detection, and multi-GPU performance optimization insights.

use crate::{DeviceId, GpuDevice, UnifiedGpuResult};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// GPU timeline event representing a specific operation
#[derive(Debug, Clone)]
pub struct TimelineEvent {
    pub event_id: String,
    pub device_id: DeviceId,
    pub operation_type: String,
    pub start_time: Instant,
    pub end_time: Option<Instant>,
    pub gpu_timestamp_start: Option<u64>,
    pub gpu_timestamp_end: Option<u64>,
    pub memory_usage_mb: f32,
    pub workgroup_config: (u32, u32, u32),
    pub buffer_sizes: Vec<u64>,
    pub metadata: HashMap<String, String>,
}

impl TimelineEvent {
    /// Create a new timeline event
    pub fn new(
        event_id: String,
        device_id: DeviceId,
        operation_type: String,
        memory_usage_mb: f32,
        workgroup_config: (u32, u32, u32),
        buffer_sizes: Vec<u64>,
    ) -> Self {
        Self {
            event_id,
            device_id,
            operation_type,
            start_time: Instant::now(),
            end_time: None,
            gpu_timestamp_start: None,
            gpu_timestamp_end: None,
            memory_usage_mb,
            workgroup_config,
            buffer_sizes,
            metadata: HashMap::new(),
        }
    }

    /// Mark the event as completed
    pub fn complete(&mut self) {
        self.end_time = Some(Instant::now());
    }

    /// Get the CPU duration of the event
    pub fn cpu_duration(&self) -> Option<Duration> {
        self.end_time.map(|end| end.duration_since(self.start_time))
    }

    /// Get the GPU duration of the event (if available)
    pub fn gpu_duration_ns(&self) -> Option<u64> {
        match (self.gpu_timestamp_start, self.gpu_timestamp_end) {
            (Some(start), Some(end)) => end.checked_sub(start),
            _ => None,
        }
    }

    /// Calculate memory bandwidth utilization
    pub fn memory_bandwidth_gb_s(&self) -> f32 {
        if let Some(duration) = self.cpu_duration() {
            let total_bytes: u64 = self.buffer_sizes.iter().sum::<u64>() * 2; // Read + Write
            let duration_s = duration.as_secs_f32();
            if duration_s > 0.0 {
                (total_bytes as f32) / duration_s / 1e9
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Add metadata to the event
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }
}

/// Timeline analyzer for GPU performance analysis
pub struct GpuTimelineAnalyzer {
    events: Arc<Mutex<VecDeque<TimelineEvent>>>,
    max_events: usize,
    devices: Arc<Mutex<HashMap<DeviceId, Arc<GpuDevice>>>>,
}

impl GpuTimelineAnalyzer {
    /// Create a new timeline analyzer
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::with_capacity(max_events))),
            max_events,
            devices: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add a device to track
    pub fn add_device(&self, device: Arc<GpuDevice>) {
        if let Ok(mut devices) = self.devices.lock() {
            devices.insert(device.id, device);
        }
    }

    /// Record a timeline event
    pub fn record_event(&self, event: TimelineEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push_back(event);

            // Keep only the most recent events
            while events.len() > self.max_events {
                events.pop_front();
            }
        }
    }

    /// Get all events in the specified time range
    pub fn get_events_in_range(&self, start: Instant, end: Instant) -> Vec<TimelineEvent> {
        if let Ok(events) = self.events.lock() {
            events
                .iter()
                .filter(|event| event.start_time >= start && event.start_time <= end)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get events for a specific device
    pub fn get_device_events(
        &self,
        device_id: DeviceId,
        limit: Option<usize>,
    ) -> Vec<TimelineEvent> {
        if let Ok(events) = self.events.lock() {
            let mut device_events: Vec<_> = events
                .iter()
                .filter(|event| event.device_id == device_id)
                .cloned()
                .collect();

            if let Some(limit) = limit {
                device_events.truncate(limit);
            }

            device_events
        } else {
            Vec::new()
        }
    }

    /// Analyze GPU utilization over time
    pub fn analyze_gpu_utilization(&self, window_duration: Duration) -> UtilizationAnalysis {
        let now = Instant::now();
        if window_duration.is_zero() {
            return UtilizationAnalysis {
                analysis_window: window_duration,
                device_stats: HashMap::new(),
                timestamp: now,
            };
        }
        let window_start = now - window_duration;

        let events = self.get_events_in_range(window_start, now);
        let mut device_utilization = HashMap::new();

        // Group events by device
        for event in events {
            let device_events = device_utilization
                .entry(event.device_id)
                .or_insert_with(Vec::new);
            device_events.push(event);
        }

        let mut device_stats = HashMap::new();

        for (device_id, events) in device_utilization {
            let total_duration: Duration =
                events.iter().filter_map(|event| event.cpu_duration()).sum();

            let utilization_percent =
                (total_duration.as_secs_f32() / window_duration.as_secs_f32()) * 100.0;
            let utilization_percent = utilization_percent.min(100.0); // Cap at 100%

            let avg_memory_bandwidth = if !events.is_empty() {
                events
                    .iter()
                    .map(|e| e.memory_bandwidth_gb_s())
                    .sum::<f32>()
                    / events.len() as f32
            } else {
                0.0
            };

            device_stats.insert(
                device_id,
                DeviceUtilizationStats {
                    utilization_percent,
                    operation_count: events.len(),
                    avg_memory_bandwidth_gb_s: avg_memory_bandwidth,
                    total_duration,
                },
            );
        }

        UtilizationAnalysis {
            analysis_window: window_duration,
            device_stats,
            timestamp: now,
        }
    }

    /// Detect performance bottlenecks
    pub fn detect_bottlenecks(&self, analysis_window: Duration) -> BottleneckAnalysis {
        let utilization = self.analyze_gpu_utilization(analysis_window);
        let events = self.get_events_in_range(Instant::now() - analysis_window, Instant::now());

        let mut bottlenecks = Vec::new();

        // Analyze GPU utilization bottlenecks
        for (device_id, stats) in &utilization.device_stats {
            if stats.utilization_percent < 50.0 {
                bottlenecks.push(PerformanceBottleneck::LowGpuUtilization {
                    device_id: *device_id,
                    utilization_percent: stats.utilization_percent,
                    recommendation: "Consider increasing batch size or workload complexity"
                        .to_string(),
                });
            }

            if stats.avg_memory_bandwidth_gb_s < 100.0 {
                // Assuming 100 GB/s baseline
                bottlenecks.push(PerformanceBottleneck::MemoryBandwidthUnderutilized {
                    device_id: *device_id,
                    bandwidth_gb_s: stats.avg_memory_bandwidth_gb_s,
                    recommendation: "Optimize memory access patterns or increase data parallelism"
                        .to_string(),
                });
            }
        }

        // Analyze synchronization bottlenecks
        let sync_analysis = self.analyze_synchronization_overhead(&events);
        if sync_analysis.avg_sync_overhead_percent > 20.0 {
            bottlenecks.push(PerformanceBottleneck::SynchronizationOverhead {
                overhead_percent: sync_analysis.avg_sync_overhead_percent,
                recommendation: "Reduce synchronization frequency or use asynchronous operations"
                    .to_string(),
            });
        }

        // Analyze workgroup efficiency
        let workgroup_analysis = self.analyze_workgroup_efficiency(&events);
        for (device_id, efficiency) in workgroup_analysis {
            if efficiency < 70.0 {
                bottlenecks.push(PerformanceBottleneck::InefficientWorkgroups {
                    device_id,
                    efficiency_percent: efficiency,
                    recommendation: "Optimize workgroup size or shared memory usage".to_string(),
                });
            }
        }

        let recommendations = self.generate_optimization_recommendations(&bottlenecks);

        BottleneckAnalysis {
            analysis_window,
            bottlenecks,
            recommendations,
            timestamp: Instant::now(),
        }
    }

    /// Analyze synchronization overhead
    fn analyze_synchronization_overhead(
        &self,
        events: &[TimelineEvent],
    ) -> SynchronizationAnalysis {
        let mut total_operation_time = Duration::ZERO;
        let mut total_sync_time = Duration::ZERO;

        // Group events by operation type to identify synchronization patterns
        let mut operation_groups = HashMap::new();
        for event in events {
            let group = operation_groups
                .entry(&event.operation_type)
                .or_insert_with(Vec::new);
            group.push(event);
        }

        // Estimate synchronization overhead by looking at gaps between operations
        for (_op_type, group_events) in operation_groups {
            for window in group_events.windows(2) {
                if let [event1, event2] = window {
                    if let (Some(end1), start2) = (event1.end_time, event2.start_time) {
                        if event1.device_id != event2.device_id {
                            // Cross-device gap indicates potential synchronization
                            let gap = start2.duration_since(end1);
                            total_sync_time += gap;
                        }
                        if let Some(duration1) = event1.cpu_duration() {
                            total_operation_time += duration1;
                        }
                    }
                }
            }
        }

        let sync_overhead_percent = if total_operation_time.as_nanos() > 0 {
            (total_sync_time.as_nanos() as f32 / total_operation_time.as_nanos() as f32) * 100.0
        } else {
            0.0
        };

        SynchronizationAnalysis {
            total_sync_time,
            total_operation_time,
            avg_sync_overhead_percent: sync_overhead_percent,
            cross_device_operations: events.len(),
        }
    }

    /// Analyze workgroup efficiency
    fn analyze_workgroup_efficiency(&self, events: &[TimelineEvent]) -> HashMap<DeviceId, f32> {
        let mut device_efficiency = HashMap::new();

        for event in events {
            if let Some(_duration) = event.cpu_duration() {
                let (x, y, z) = event.workgroup_config;
                let total_threads = x * y * z;

                // Estimate efficiency based on workgroup utilization
                // This is a simplified heuristic - in practice, you'd use GPU profiling data
                let theoretical_max_threads = 1024; // Common maximum
                let utilization = (total_threads as f32 / theoretical_max_threads as f32).min(1.0);

                // Factor in memory bandwidth and duration
                let memory_efficiency = (event.memory_bandwidth_gb_s() / 500.0).min(1.0); // Normalize to 500 GB/s

                let efficiency = (utilization * 0.6 + memory_efficiency * 0.4) * 100.0;

                let current_efficiency = device_efficiency.entry(event.device_id).or_insert(0.0);
                *current_efficiency = (*current_efficiency + efficiency) / 2.0; // Running average
            }
        }

        device_efficiency
    }

    /// Generate optimization recommendations
    fn generate_optimization_recommendations(
        &self,
        bottlenecks: &[PerformanceBottleneck],
    ) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();

        // Count bottleneck types
        let mut low_utilization_count = 0;
        let mut memory_issues = 0;
        let mut sync_issues = 0;
        let mut workgroup_issues = 0;

        for bottleneck in bottlenecks {
            match bottleneck {
                PerformanceBottleneck::LowGpuUtilization { .. } => low_utilization_count += 1,
                PerformanceBottleneck::MemoryBandwidthUnderutilized { .. } => memory_issues += 1,
                PerformanceBottleneck::SynchronizationOverhead { .. } => sync_issues += 1,
                PerformanceBottleneck::InefficientWorkgroups { .. } => workgroup_issues += 1,
            }
        }

        // Generate high-level recommendations
        if low_utilization_count > 0 {
            recommendations.push(OptimizationRecommendation {
                priority: RecommendationPriority::High,
                category: "GPU Utilization".to_string(),
                description: "Multiple devices showing low utilization".to_string(),
                action: "Consider increasing batch sizes or enabling more parallel operations"
                    .to_string(),
                estimated_improvement: format!("{}% performance gain", low_utilization_count * 15),
            });
        }

        if memory_issues > 0 {
            recommendations.push(OptimizationRecommendation {
                priority: RecommendationPriority::Medium,
                category: "Memory Optimization".to_string(),
                description: "Memory bandwidth underutilized".to_string(),
                action: "Optimize data layouts and reduce memory transfer overhead".to_string(),
                estimated_improvement: "10-25% performance gain".to_string(),
            });
        }

        if sync_issues > 0 {
            recommendations.push(OptimizationRecommendation {
                priority: RecommendationPriority::High,
                category: "Synchronization".to_string(),
                description: "High synchronization overhead detected".to_string(),
                action: "Implement asynchronous operations and reduce cross-device dependencies"
                    .to_string(),
                estimated_improvement: "20-40% performance gain".to_string(),
            });
        }

        if workgroup_issues > 0 {
            recommendations.push(OptimizationRecommendation {
                priority: RecommendationPriority::Low,
                category: "Workgroup Configuration".to_string(),
                description: "Suboptimal workgroup configurations".to_string(),
                action: "Tune workgroup sizes and shared memory usage".to_string(),
                estimated_improvement: "5-15% performance gain".to_string(),
            });
        }

        recommendations
    }
}

/// Device utilization statistics
#[derive(Debug, Clone)]
pub struct DeviceUtilizationStats {
    pub utilization_percent: f32,
    pub operation_count: usize,
    pub avg_memory_bandwidth_gb_s: f32,
    pub total_duration: Duration,
}

/// GPU utilization analysis result
#[derive(Debug, Clone)]
pub struct UtilizationAnalysis {
    pub analysis_window: Duration,
    pub device_stats: HashMap<DeviceId, DeviceUtilizationStats>,
    pub timestamp: Instant,
}

/// Synchronization analysis result
#[derive(Debug, Clone)]
pub struct SynchronizationAnalysis {
    pub total_sync_time: Duration,
    pub total_operation_time: Duration,
    pub avg_sync_overhead_percent: f32,
    pub cross_device_operations: usize,
}

/// Performance bottleneck types
#[derive(Debug, Clone)]
pub enum PerformanceBottleneck {
    LowGpuUtilization {
        device_id: DeviceId,
        utilization_percent: f32,
        recommendation: String,
    },
    MemoryBandwidthUnderutilized {
        device_id: DeviceId,
        bandwidth_gb_s: f32,
        recommendation: String,
    },
    SynchronizationOverhead {
        overhead_percent: f32,
        recommendation: String,
    },
    InefficientWorkgroups {
        device_id: DeviceId,
        efficiency_percent: f32,
        recommendation: String,
    },
}

/// Bottleneck analysis result
#[derive(Debug, Clone)]
pub struct BottleneckAnalysis {
    pub analysis_window: Duration,
    pub bottlenecks: Vec<PerformanceBottleneck>,
    pub recommendations: Vec<OptimizationRecommendation>,
    pub timestamp: Instant,
}

/// Optimization recommendation priority
#[derive(Debug, Clone)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

/// Optimization recommendation
#[derive(Debug, Clone)]
pub struct OptimizationRecommendation {
    pub priority: RecommendationPriority,
    pub category: String,
    pub description: String,
    pub action: String,
    pub estimated_improvement: String,
}

/// Multi-GPU performance monitor
pub struct MultiGpuPerformanceMonitor {
    timeline_analyzer: GpuTimelineAnalyzer,
    monitoring_enabled: bool,
    analysis_interval: Duration,
    last_analysis: Instant,
}

impl MultiGpuPerformanceMonitor {
    /// Create a new performance monitor
    pub fn new(max_events: usize, analysis_interval: Duration) -> Self {
        Self {
            timeline_analyzer: GpuTimelineAnalyzer::new(max_events),
            monitoring_enabled: true,
            analysis_interval,
            last_analysis: Instant::now(),
        }
    }

    /// Add a device to monitor
    pub fn add_device(&self, device: Arc<GpuDevice>) {
        self.timeline_analyzer.add_device(device);
    }

    /// Start monitoring an operation
    pub fn start_operation(
        &self,
        operation_id: String,
        device_id: DeviceId,
        operation_type: String,
        memory_usage_mb: f32,
        workgroup_config: (u32, u32, u32),
        buffer_sizes: Vec<u64>,
    ) -> OperationHandle<'_> {
        let event = TimelineEvent::new(
            operation_id.clone(),
            device_id,
            operation_type,
            memory_usage_mb,
            workgroup_config,
            buffer_sizes,
        );

        OperationHandle {
            event,
            monitor: self,
        }
    }

    /// Complete an operation
    fn complete_operation(&self, mut event: TimelineEvent) {
        if self.monitoring_enabled {
            event.complete();
            self.timeline_analyzer.record_event(event);
        }
    }

    /// Get performance analysis
    pub fn get_performance_analysis(
        &self,
        window_duration: Duration,
    ) -> UnifiedGpuResult<PerformanceAnalysisReport> {
        let utilization = self
            .timeline_analyzer
            .analyze_gpu_utilization(window_duration);
        let bottlenecks = self.timeline_analyzer.detect_bottlenecks(window_duration);

        Ok(PerformanceAnalysisReport {
            utilization_analysis: utilization,
            bottleneck_analysis: bottlenecks,
            timestamp: Instant::now(),
        })
    }

    /// Enable or disable monitoring
    pub fn set_monitoring_enabled(&mut self, enabled: bool) {
        self.monitoring_enabled = enabled;
    }

    /// Check if automatic analysis should be performed
    pub fn should_perform_analysis(&self) -> bool {
        self.monitoring_enabled && self.last_analysis.elapsed() >= self.analysis_interval
    }
}

/// Handle for tracking an operation
pub struct OperationHandle<'a> {
    event: TimelineEvent,
    monitor: &'a MultiGpuPerformanceMonitor,
}

impl<'a> OperationHandle<'a> {
    /// Add metadata to the operation
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.event.add_metadata(key, value);
    }

    /// Set GPU timestamps
    pub fn set_gpu_timestamps(&mut self, start: u64, end: u64) {
        self.event.gpu_timestamp_start = Some(start);
        self.event.gpu_timestamp_end = Some(end);
    }
}

impl<'a> Drop for OperationHandle<'a> {
    fn drop(&mut self) {
        // Automatically complete the operation when the handle is dropped
        let event = std::mem::replace(
            &mut self.event,
            TimelineEvent::new(
                "dropped".to_string(),
                crate::DeviceId(0),
                "dropped".to_string(),
                0.0,
                (1, 1, 1),
                vec![],
            ),
        );
        self.monitor.complete_operation(event);
    }
}

/// Combined performance analysis report
#[derive(Debug, Clone)]
pub struct PerformanceAnalysisReport {
    pub utilization_analysis: UtilizationAnalysis,
    pub bottleneck_analysis: BottleneckAnalysis,
    pub timestamp: Instant,
}

impl PerformanceAnalysisReport {
    /// Get overall performance score (0-100)
    pub fn overall_performance_score(&self) -> f32 {
        let avg_utilization = if !self.utilization_analysis.device_stats.is_empty() {
            self.utilization_analysis
                .device_stats
                .values()
                .map(|stats| stats.utilization_percent)
                .sum::<f32>()
                / self.utilization_analysis.device_stats.len() as f32
        } else {
            0.0
        };

        // Penalize for bottlenecks
        let bottleneck_penalty = self.bottleneck_analysis.bottlenecks.len() as f32 * 5.0;
        let score = avg_utilization - bottleneck_penalty;
        score.clamp(0.0, 100.0)
    }

    /// Get summary statistics
    pub fn get_summary(&self) -> PerformanceSummary {
        let total_devices = self.utilization_analysis.device_stats.len();
        let high_priority_issues = self
            .bottleneck_analysis
            .recommendations
            .iter()
            .filter(|rec| {
                matches!(
                    rec.priority,
                    RecommendationPriority::High | RecommendationPriority::Critical
                )
            })
            .count();

        PerformanceSummary {
            overall_score: self.overall_performance_score(),
            total_devices,
            active_bottlenecks: self.bottleneck_analysis.bottlenecks.len(),
            high_priority_recommendations: high_priority_issues,
            analysis_window: self.utilization_analysis.analysis_window,
        }
    }
}

/// Performance summary statistics
#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub overall_score: f32,
    pub total_devices: usize,
    pub active_bottlenecks: usize,
    pub high_priority_recommendations: usize,
    pub analysis_window: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeline_event_creation() {
        let event = TimelineEvent::new(
            "test_event".to_string(),
            crate::DeviceId(0),
            "test_operation".to_string(),
            100.0,
            (64, 1, 1),
            vec![1024, 2048],
        );

        assert_eq!(event.event_id, "test_event");
        assert_eq!(event.device_id, crate::DeviceId(0));
        assert_eq!(event.memory_usage_mb, 100.0);
        assert!(event.end_time.is_none());
    }

    #[test]
    fn test_timeline_analyzer() {
        let analyzer = GpuTimelineAnalyzer::new(100);

        let mut event = TimelineEvent::new(
            "test".to_string(),
            crate::DeviceId(0),
            "matrix_multiply".to_string(),
            50.0,
            (16, 16, 1),
            vec![1024],
        );

        std::thread::sleep(std::time::Duration::from_millis(10));
        event.complete();

        analyzer.record_event(event);

        let events = analyzer.get_device_events(crate::DeviceId(0), None);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_performance_monitor() {
        let monitor = MultiGpuPerformanceMonitor::new(100, Duration::from_secs(1));

        let _handle = monitor.start_operation(
            "test_op".to_string(),
            crate::DeviceId(0),
            "test".to_string(),
            10.0,
            (64, 1, 1),
            vec![512],
        );

        // Handle will be dropped here, completing the operation

        let analysis = monitor.get_performance_analysis(Duration::from_secs(1));
        assert!(analysis.is_ok());
    }
}
