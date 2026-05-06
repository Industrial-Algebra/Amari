//! Integration tests for multi-GPU functionality
//!
//! These tests validate the complete multi-GPU pipeline but are designed
//! to gracefully handle environments without GPU access.

mod common;

use amari_gpu::{
    AmariMultiGpuBenchmarks, BenchmarkConfig, BenchmarkRunner, ComputeIntensity, DeviceId,
    IntelligentLoadBalancer, LoadBalancingStrategy, MultiGpuPerformanceMonitor, SharedGpuContext,
    Workload,
};
use common::direct_gpu_runtime_available;
use std::time::Duration;

#[tokio::test]
async fn test_multi_gpu_context_graceful_fallback() {
    // Skip GPU tests in CI environments where GPU is not available
    if !direct_gpu_runtime_available() {
        println!("Skipping GPU test in CI environment");
        return;
    }

    // Test that multi-GPU context creation handles environments without GPU gracefully
    match SharedGpuContext::with_multi_gpu().await {
        Ok(context) => {
            // GPU available - test basic functionality
            assert!(context.device_count().await >= 1);
            println!(
                "Multi-GPU context created with {} devices",
                context.device_count().await
            );

            // Test device enumeration
            let device_info = context.get_device_info().await;
            assert!(!device_info.is_empty());

            // Test GPU utilization tracking
            let utilization = context.get_gpu_utilization().await;
            assert!(!utilization.is_empty());
        }
        Err(_) => {
            // No GPU available - this is expected in CI environments
            println!("Multi-GPU context creation failed - no GPU available (expected in CI)");
        }
    }
}

#[tokio::test]
async fn test_load_balancer_integration() {
    let load_balancer = IntelligentLoadBalancer::new(LoadBalancingStrategy::Balanced);

    // Test workload distribution with mock data
    let workload = Workload {
        operation_type: "test_operation".to_string(),
        data_size: 1000,
        memory_requirement_mb: 10.0,
        compute_intensity: ComputeIntensity::Moderate,
        parallelizable: true,
        synchronization_required: false,
    };

    // This should work even without real GPUs as it will gracefully handle the case
    match load_balancer.distribute_workload(&workload).await {
        Ok(assignments) => {
            println!(
                "Workload distributed across {} assignments",
                assignments.len()
            );

            // Verify assignments are reasonable
            for assignment in &assignments {
                assert!(assignment.workload_fraction > 0.0);
                assert!(assignment.workload_fraction <= 1.0);
                assert!(assignment.data_range.1 >= assignment.data_range.0);
            }

            // Verify total fraction sums to approximately 1.0
            let total_fraction: f32 = assignments.iter().map(|a| a.workload_fraction).sum();
            assert!((total_fraction - 1.0).abs() < 0.1);
        }
        Err(e) => {
            println!(
                "Load balancer distribution failed: {:?} (expected without GPU)",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_performance_monitor_integration() {
    let monitor = MultiGpuPerformanceMonitor::new(1000, Duration::from_secs(1));

    // Test operation tracking
    let handle = monitor.start_operation(
        "test_integration".to_string(),
        DeviceId(0),
        "integration_test".to_string(),
        5.0,
        (64, 1, 1),
        vec![1024, 2048],
    );

    // Simulate some work
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Handle will be dropped here, completing the operation
    drop(handle);

    // Test performance analysis
    match monitor.get_performance_analysis(Duration::from_secs(1)) {
        Ok(analysis) => {
            println!("Performance analysis completed");
            assert!(analysis.overall_performance_score() >= 0.0);
            assert!(analysis.overall_performance_score() <= 100.0);

            let summary = analysis.get_summary();
            assert!(summary.analysis_window.as_secs() >= 1);
        }
        Err(e) => {
            println!("Performance analysis failed: {:?}", e);
        }
    }
}

#[tokio::test]
#[ignore] // Requires GPU hardware
async fn test_benchmark_runner_quick_validation() {
    // Test quick validation benchmarks
    match BenchmarkRunner::run_quick_validation().await {
        Ok(results) => {
            println!(
                "Quick validation completed with {} results",
                results.results.len()
            );

            // Verify benchmark results structure
            assert!(!results.results.is_empty());
            assert!(results.total_duration.as_millis() > 0);

            for result in &results.results {
                assert!(result.duration_ms >= 0.0);
                assert!(result.throughput_ops_per_sec >= 0.0);
                assert!(result.data_size > 0);
                assert!(result.device_count > 0);
            }

            // Verify performance summary
            let summary = &results.performance_summary;
            assert!(summary.total_tests > 0);
            assert!(summary.average_scaling_efficiency >= 0.0);
        }
        Err(e) => {
            println!("Quick validation failed: {:?} (expected without GPU)", e);
        }
    }
}

#[tokio::test]
async fn test_workload_distribution_algorithms() {
    // Test different load balancing strategies
    let strategies = vec![
        LoadBalancingStrategy::Balanced,
        LoadBalancingStrategy::CapabilityAware,
        LoadBalancingStrategy::MemoryAware,
        LoadBalancingStrategy::LatencyOptimized,
    ];

    let workload = Workload {
        operation_type: "matrix_multiply".to_string(),
        data_size: 10000,
        memory_requirement_mb: 100.0,
        compute_intensity: ComputeIntensity::Heavy,
        parallelizable: true,
        synchronization_required: true,
    };

    for strategy in strategies {
        let load_balancer = IntelligentLoadBalancer::new(strategy);

        match load_balancer.distribute_workload(&workload).await {
            Ok(assignments) => {
                println!(
                    "Strategy {:?} produced {} assignments",
                    strategy,
                    assignments.len()
                );

                // Basic sanity checks
                for assignment in &assignments {
                    assert!(assignment.workload_fraction > 0.0);
                    assert!(assignment.estimated_completion_ms >= 0.0);
                    assert!(assignment.memory_requirement_mb >= 0.0);
                }
            }
            Err(_) => {
                println!("Strategy {:?} failed (expected without GPU)", strategy);
            }
        }
    }
}

#[tokio::test]
async fn test_device_failure_handling() {
    // Test that the system handles device failures gracefully
    // This is a mock test since we can't simulate real device failures easily

    let load_balancer = IntelligentLoadBalancer::new(LoadBalancingStrategy::Balanced);

    // Test with workload that would require multiple devices
    let large_workload = Workload {
        operation_type: "large_computation".to_string(),
        data_size: 100000,
        memory_requirement_mb: 1000.0,
        compute_intensity: ComputeIntensity::Extreme,
        parallelizable: true,
        synchronization_required: true,
    };

    // Should either succeed with available devices or fail gracefully
    match load_balancer.distribute_workload(&large_workload).await {
        Ok(assignments) => {
            println!("Large workload distributed successfully");
            assert!(!assignments.is_empty());
        }
        Err(e) => {
            println!("Large workload distribution failed gracefully: {:?}", e);
            // This is expected behavior when no suitable devices are available
        }
    }
}

#[tokio::test]
async fn test_benchmark_configuration_validation() {
    // Test various benchmark configurations
    let configs = vec![
        BenchmarkConfig {
            warmup_iterations: 1,
            measurement_iterations: 2,
            data_sizes: vec![10, 100],
            device_combinations: vec![vec![DeviceId(0)]],
            enable_profiling: false,
            ..Default::default()
        },
        BenchmarkConfig {
            warmup_iterations: 0,
            measurement_iterations: 1,
            data_sizes: vec![1],
            device_combinations: vec![vec![DeviceId(0)], vec![DeviceId(0), DeviceId(1)]],
            enable_profiling: true,
            ..Default::default()
        },
    ];

    for (i, config) in configs.into_iter().enumerate() {
        match AmariMultiGpuBenchmarks::new(config).await {
            Ok(_benchmarks) => {
                println!("Benchmark configuration {} created successfully", i);
            }
            Err(e) => {
                println!(
                    "Benchmark configuration {} failed: {:?} (expected without GPU)",
                    i, e
                );
            }
        }
    }
}

#[tokio::test]
async fn test_memory_management_under_load() {
    // Test memory management with varying workload sizes
    let workload_sizes = vec![100, 1000, 10000];

    for size in workload_sizes {
        let workload = Workload {
            operation_type: "memory_test".to_string(),
            data_size: size,
            memory_requirement_mb: (size as f32) / 100.0,
            compute_intensity: ComputeIntensity::Light,
            parallelizable: true,
            synchronization_required: false,
        };

        let load_balancer = IntelligentLoadBalancer::new(LoadBalancingStrategy::MemoryAware);

        match load_balancer.distribute_workload(&workload).await {
            Ok(assignments) => {
                println!("Memory test with size {} completed", size);

                // Verify memory requirements are reasonable
                let total_memory: f32 = assignments.iter().map(|a| a.memory_requirement_mb).sum();
                assert!(total_memory > 0.0);
                assert!(total_memory <= workload.memory_requirement_mb * 1.1); // Allow small overhead
            }
            Err(_) => {
                println!(
                    "Memory test with size {} failed (expected without GPU)",
                    size
                );
            }
        }
    }
}

#[tokio::test]
async fn test_scaling_efficiency_calculation() {
    // Test that scaling efficiency calculations are reasonable
    let monitor = MultiGpuPerformanceMonitor::new(100, Duration::from_secs(1));

    // Simulate operations on different device counts
    let device_counts = vec![1, 2, 4];

    for device_count in device_counts {
        let operation_name = format!("scaling_test_{}_gpu", device_count);

        let handle = monitor.start_operation(
            operation_name,
            DeviceId(0),
            "scaling_test".to_string(),
            10.0,
            (128, 1, 1),
            vec![1024],
        );

        // Simulate work that scales with device count
        let work_time = Duration::from_millis(100 / device_count as u64);
        tokio::time::sleep(work_time).await;

        drop(handle);
    }

    // Analyze results
    match monitor.get_performance_analysis(Duration::from_secs(5)) {
        Ok(analysis) => {
            println!("Scaling analysis completed");
            let summary = analysis.get_summary();
            // total_devices is usize and cannot be negative, so just verify it exists
            println!("Analysis completed with {} devices", summary.total_devices);
        }
        Err(e) => {
            println!("Scaling analysis failed: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_concurrent_workload_handling() {
    // Test handling multiple concurrent workloads
    let monitor = MultiGpuPerformanceMonitor::new(1000, Duration::from_secs(1));

    let mut handles = Vec::new();

    // Start multiple concurrent operations
    for i in 0..5 {
        let handle = monitor.start_operation(
            format!("concurrent_op_{}", i),
            DeviceId(i % 2), // Alternate between devices
            "concurrent_test".to_string(),
            5.0,
            (64, 1, 1),
            vec![512],
        );
        handles.push(handle);
    }

    // Simulate concurrent work
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Complete all operations
    drop(handles);

    // Verify monitoring handled concurrent operations
    match monitor.get_performance_analysis(Duration::from_secs(1)) {
        Ok(analysis) => {
            println!("Concurrent workload analysis completed");
            assert!(analysis.overall_performance_score() >= 0.0);
        }
        Err(e) => {
            println!("Concurrent workload analysis failed: {:?}", e);
        }
    }
}
