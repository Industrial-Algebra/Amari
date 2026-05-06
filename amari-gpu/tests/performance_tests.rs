//! Comprehensive GPU performance test suite
//!
//! This test suite validates the performance optimizations implemented across
//! all Amari mathematical crates, including:
//! - Shared GPU context performance
//! - Buffer pooling effectiveness
//! - Workgroup size optimization
//! - Cross-crate GPU resource sharing
//! - Memory usage optimization

mod common;

use amari_gpu::*;
use common::direct_gpu_runtime_available;
use std::time::Instant;

/// Skip direct GPU tests in environments where GPU runtime access is unavailable.
fn skip_if_ci() -> bool {
    !direct_gpu_runtime_available()
}

#[tokio::test]
async fn test_shared_gpu_context_creation() {
    if skip_if_ci() {
        println!("Skipping GPU test in CI environment");
        return;
    }

    // Test that GPU context can be created successfully
    let start = Instant::now();
    let context1 = SharedGpuContext::global().await;
    let creation_time = start.elapsed();

    println!("GPU context creation time: {:?}", creation_time);
    assert!(context1.is_ok());

    // Test that workgroup optimization is available
    let ctx = context1.unwrap();
    let workgroup = ctx.get_optimal_workgroup("matrix_multiply", 1000);
    assert_eq!(workgroup, (16, 16, 1));

    println!("✅ GPU context and optimization infrastructure working");
}

#[tokio::test]
async fn test_buffer_pool_performance() {
    if skip_if_ci() {
        println!("Skipping GPU test in CI environment");
        return;
    }

    let context = SharedGpuContext::global().await.unwrap();

    // Test buffer pool hit rate improves over time
    let buffer_size = 1024 * 1024; // 1MB
    let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;

    // First allocation - should be slow (miss)
    let start = Instant::now();
    let buffer1 = context.get_buffer(buffer_size, usage, Some("test1"));
    let first_alloc_time = start.elapsed();

    // Return buffer to pool
    context.return_buffer(buffer1, buffer_size, usage);

    // Second allocation - should be fast (hit)
    let start = Instant::now();
    let _buffer2 = context.get_buffer(buffer_size, usage, Some("test2"));
    let second_alloc_time = start.elapsed();

    // Pool hit should be significantly faster
    assert!(second_alloc_time < first_alloc_time / 2);

    // Verify pool statistics
    let stats = context.buffer_pool_stats();
    assert!(stats.hit_rate_percent > 0.0);
    assert!(stats.total_buffers_reused > 0);
}

#[tokio::test]
async fn test_workgroup_optimization() {
    if skip_if_ci() {
        println!("Skipping GPU test in CI environment");
        return;
    }

    let context = SharedGpuContext::global().await.unwrap();

    // Test different operation types get appropriate workgroup sizes
    let matrix_wg = context.get_optimal_workgroup("matrix_multiply", 1000);
    assert_eq!(matrix_wg, (16, 16, 1)); // 2D workgroup for matrices

    let vector_wg = context.get_optimal_workgroup("vector_operation", 20000);
    println!("Vector workgroup: {:?}", vector_wg);
    assert_eq!(vector_wg, (256, 1, 1)); // Large 1D for big vectors

    let ca_wg = context.get_optimal_workgroup("cellular_automata", 1000);
    assert_eq!(ca_wg, (16, 16, 1)); // 2D for CA grids

    // Test workgroup declaration generation
    let decl = context.get_workgroup_declaration("neural_network", 5000);
    assert_eq!(decl, "@compute @workgroup_size(256)");

    let matrix_decl = context.get_workgroup_declaration("matrix_multiply", 1000);
    assert_eq!(matrix_decl, "@compute @workgroup_size(16, 16)");
}

#[tokio::test]
async fn test_cross_crate_gpu_sharing() {
    if skip_if_ci() {
        println!("Skipping GPU test in CI environment");
        return;
    }

    // Simulate accessing GPU resources sequentially (representing different crates)
    let context1 = SharedGpuContext::global().await.unwrap();
    let context2 = SharedGpuContext::global().await.unwrap();

    // Test that both contexts can access GPU resources
    let adapter_info1 = context1.adapter_info();
    let adapter_info2 = context2.adapter_info();

    println!(
        "Context 1 adapter: {} - {:?}",
        adapter_info1.name, adapter_info1.device_type
    );
    println!(
        "Context 2 adapter: {} - {:?}",
        adapter_info2.name, adapter_info2.device_type
    );

    // Both contexts should be functional and have the same adapter type
    assert_eq!(adapter_info1.device_type, adapter_info2.device_type);

    // Test buffer operations work on both contexts
    let buffer1 = context1.get_buffer(1024, wgpu::BufferUsages::STORAGE, Some("cross_test1"));
    let buffer2 = context2.get_buffer(1024, wgpu::BufferUsages::STORAGE, Some("cross_test2"));

    context1.return_buffer(buffer1, 1024, wgpu::BufferUsages::STORAGE);
    context2.return_buffer(buffer2, 1024, wgpu::BufferUsages::STORAGE);

    println!("✅ Cross-crate GPU resource sharing test passed");
}

#[tokio::test]
async fn test_memory_usage_tracking() {
    if skip_if_ci() {
        println!("Skipping GPU test in CI environment");
        return;
    }

    let context = SharedGpuContext::global().await.unwrap();

    let initial_stats = context.buffer_pool_stats();

    // Allocate several buffers
    let buffers: Vec<_> = (0..10)
        .map(|i| {
            context.get_buffer(
                1024 * (i + 1) as u64,
                wgpu::BufferUsages::STORAGE,
                Some(&format!("test{}", i)),
            )
        })
        .collect();

    let after_alloc_stats = context.buffer_pool_stats();
    assert!(after_alloc_stats.total_buffers_created > initial_stats.total_buffers_created);

    // Return buffers
    for (i, buffer) in buffers.into_iter().enumerate() {
        context.return_buffer(buffer, 1024 * (i + 1) as u64, wgpu::BufferUsages::STORAGE);
    }

    let final_stats = context.buffer_pool_stats();
    assert!(final_stats.current_pooled_count > initial_stats.current_pooled_count);
    assert!(final_stats.total_pooled_memory_mb > 0.0);
}

#[tokio::test]
async fn test_shader_caching_performance() {
    if skip_if_ci() {
        println!("Skipping GPU test in CI environment");
        return;
    }

    let context = SharedGpuContext::global().await.unwrap();

    let simple_shader = r#"
        @compute @workgroup_size(64)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
            // Simple test shader
        }
    "#;

    // First shader compilation - should be slow
    let start = Instant::now();
    let pipeline1 = context.get_compute_pipeline("test_shader", simple_shader, "main");
    let first_compile_time = start.elapsed();
    assert!(pipeline1.is_ok());

    // Second compilation - should hit cache and be faster
    let start = Instant::now();
    let pipeline2 = context.get_compute_pipeline("test_shader", simple_shader, "main");
    let second_compile_time = start.elapsed();
    assert!(pipeline2.is_ok());

    // Cache hit should be significantly faster
    assert!(second_compile_time < first_compile_time / 10);
}

#[tokio::test]
async fn test_gpu_profiling_infrastructure() -> Result<(), Box<dyn std::error::Error>> {
    // Skip if no timestamp query support
    if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
        println!("Skipping GPU profiling test in CI");
        return Ok(());
    }

    use amari_gpu::performance::GpuProfiler;

    match GpuProfiler::new().await {
        Ok(_profiler) => {
            println!("✅ GPU profiler initialized successfully");
            // Note: Profiling methods not yet fully implemented
        }
        Err(_) => {
            println!("⚠️  GPU profiling unavailable (no timestamp query support)");
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_adaptive_dispatch_policy() -> Result<(), Box<dyn std::error::Error>> {
    use amari_gpu::performance::AdaptiveDispatchPolicy;

    let _policy = AdaptiveDispatchPolicy::new();

    // Note: Methods not yet implemented, this test validates compilation
    println!("AdaptiveDispatchPolicy created successfully");

    Ok(())
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_end_to_end_optimization() {
        if skip_if_ci() {
            println!("Skipping GPU test in CI environment");
            return;
        }

        // Test complete optimization pipeline across multiple operations
        let context = SharedGpuContext::global().await.unwrap();

        // Test operations from different mathematical domains
        let operations = [
            ("tropical_matrix", 1000),
            ("information_geometry", 5000),
            ("cellular_automata", 256 * 256),
            ("dual_number", 2000),
            ("fusion_system", 10000),
        ];

        for (op, size) in operations.iter() {
            // Get optimal workgroup
            let workgroup = context.get_optimal_workgroup(op, *size);
            assert!(workgroup.0 > 0 && workgroup.1 > 0 && workgroup.2 > 0);

            // Generate declaration
            let declaration = context.get_workgroup_declaration(op, *size);
            assert!(declaration.starts_with("@compute @workgroup_size"));

            // Verify buffer pool works
            let buffer = context.get_buffer(1024, wgpu::BufferUsages::STORAGE, Some(op));
            context.return_buffer(buffer, 1024, wgpu::BufferUsages::STORAGE);
        }

        // Verify pool statistics after all operations
        let stats = context.buffer_pool_stats();
        println!(
            "Final pool stats: {:.1}% hit rate, {} buffers created",
            stats.hit_rate_percent, stats.total_buffers_created
        );
    }

    #[tokio::test]
    async fn test_memory_efficiency() {
        if skip_if_ci() {
            println!("Skipping GPU test in CI environment");
            return;
        }

        let context = SharedGpuContext::global().await.unwrap();

        let initial_stats = context.buffer_pool_stats();
        let initial_memory = initial_stats.total_pooled_memory_mb;

        // Create and return many buffers
        for i in 0..100 {
            let size = 1024 * (i % 10 + 1) as u64;
            let buffer =
                context.get_buffer(size, wgpu::BufferUsages::STORAGE, Some("efficiency_test"));
            context.return_buffer(buffer, size, wgpu::BufferUsages::STORAGE);
        }

        let final_stats = context.buffer_pool_stats();

        // Memory usage should be reasonable (not growing unbounded)
        assert!(final_stats.total_pooled_memory_mb < initial_memory + 50.0); // Max 50MB increase

        // Hit rate should improve significantly
        if final_stats.total_buffers_created > 10 {
            assert!(final_stats.hit_rate_percent > 50.0);
        }
    }
}
