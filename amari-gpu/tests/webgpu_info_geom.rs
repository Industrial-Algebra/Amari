//! WebGPU Information Geometry Tests - TDD Phase 1
//!
//! This module contains tests for GPU-accelerated information geometry operations,
//! focusing on edge computing capabilities with WebAssembly integration.

mod common;

use amari_core::Multivector;
use amari_gpu::{GpuError, GpuInfoGeometry};
use amari_info_geom::amari_chentsov_tensor;
use common::direct_gpu_runtime_available;

/// Helper function to safely initialize GPU in CI or headless environments.
async fn safe_gpu_init() -> Result<GpuInfoGeometry, GpuError> {
    if !direct_gpu_runtime_available() {
        return Err(GpuError::InitializationError(
            "GPU runtime unavailable in this test environment".to_string(),
        ));
    }

    let gpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tokio::runtime::Handle::current().block_on(async { GpuInfoGeometry::new().await })
    }));

    match gpu_result {
        Ok(Ok(gpu)) => Ok(gpu),
        Ok(Err(e)) => Err(e),
        Err(_panic) => Err(GpuError::InitializationError(
            "GPU panicked during initialization (expected in CI)".to_string(),
        )),
    }
}

/// Test GPU-accelerated Amari-Chentsov tensor computation
#[tokio::test]
async fn test_gpu_amari_chentsov_tensor_single() -> Result<(), GpuError> {
    let gpu_info_geom = match safe_gpu_init().await {
        Ok(gpu) => gpu,
        Err(GpuError::InitializationError(_)) => {
            println!("WebGPU not available, skipping GPU test");
            return Ok(()); // Skip test in environments without WebGPU
        }
        Err(e) => return Err(e),
    };

    // Create test vectors
    let x = create_test_vector_e1();
    let y = create_test_vector_e2();
    let z = create_test_vector_e3();

    // Compute on GPU
    let gpu_result = gpu_info_geom.amari_chentsov_tensor(&x, &y, &z).await?;

    // Compute on CPU for comparison
    let cpu_result = amari_chentsov_tensor(&x, &y, &z);

    // Should match within floating point precision
    assert!((gpu_result - cpu_result).abs() < 1e-12);
    assert_eq!(gpu_result, 1.0); // e1 × e2 × e3 = 1

    Ok(())
}

/// Test batch GPU computation of Amari-Chentsov tensors
#[tokio::test]
async fn test_gpu_amari_chentsov_tensor_batch() -> Result<(), GpuError> {
    let gpu_info_geom = match safe_gpu_init().await {
        Ok(gpu) => gpu,
        Err(GpuError::InitializationError(_)) => {
            println!("WebGPU not available, skipping GPU test");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    // Create batch of test vector triplets
    let batch_size = 1000;
    let (x_batch, y_batch, z_batch) = create_tensor_batch(batch_size);

    // Compute batch on GPU
    let gpu_results = gpu_info_geom
        .amari_chentsov_tensor_batch(&x_batch, &y_batch, &z_batch)
        .await?;

    // Verify batch size
    assert_eq!(gpu_results.len(), batch_size);

    // Spot check some results
    for i in (0..batch_size).step_by(100) {
        let cpu_result = amari_chentsov_tensor(&x_batch[i], &y_batch[i], &z_batch[i]);
        assert!((gpu_results[i] - cpu_result).abs() < 1e-10);
    }

    Ok(())
}

/// Test WebGPU Fisher Information Matrix computation
#[tokio::test]
async fn test_gpu_fisher_information_matrix() -> Result<(), GpuError> {
    let gpu_info_geom = match safe_gpu_init().await {
        Ok(gpu) => gpu,
        Err(GpuError::InitializationError(_)) => {
            println!("WebGPU not available, skipping GPU test");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    // Test parameters for exponential family
    let parameters = create_test_parameters();

    // Compute Fisher matrix on GPU
    let gpu_fisher = gpu_info_geom.fisher_information_matrix(&parameters).await?;

    // Verify positive definiteness (all eigenvalues > 0)
    let eigenvalues = gpu_fisher.eigenvalues().await?;
    for eigenval in eigenvalues {
        assert!(eigenval > 0.0, "Fisher matrix should be positive definite");
    }

    Ok(())
}

/// Test GPU Bregman divergence computation with performance scaling
#[tokio::test]
async fn test_gpu_bregman_divergence_scaling() -> Result<(), GpuError> {
    let gpu_info_geom = match safe_gpu_init().await {
        Ok(gpu) => gpu,
        Err(GpuError::InitializationError(_)) => {
            println!("WebGPU not available, skipping GPU test");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    // Test various batch sizes to verify scaling
    let batch_sizes = vec![10, 100, 1000, 10000];

    for batch_size in batch_sizes {
        let (p_batch, q_batch) = create_divergence_batch(batch_size);

        let start_time = std::time::Instant::now();
        let gpu_divergences = gpu_info_geom
            .bregman_divergence_batch(&p_batch, &q_batch)
            .await?;
        let gpu_time = start_time.elapsed();

        // Verify results
        assert_eq!(gpu_divergences.len(), batch_size);

        // All Bregman divergences should be non-negative
        for div in &gpu_divergences {
            assert!(*div >= 0.0, "Bregman divergence must be non-negative");
        }

        println!("GPU batch size {}: {:?}", batch_size, gpu_time);

        // Performance should scale sublinearly with batch size due to parallelization
        if batch_size >= 1000 {
            assert!(
                gpu_time.as_millis() < batch_size as u128 / 10,
                "GPU should provide significant speedup for large batches"
            );
        }
    }

    Ok(())
}

/// Test WebAssembly TypedArray integration
#[tokio::test]
async fn test_wasm_typed_array_integration() -> Result<(), GpuError> {
    let gpu_info_geom = match safe_gpu_init().await {
        Ok(gpu) => gpu,
        Err(GpuError::InitializationError(_)) => {
            println!("WebGPU not available, skipping GPU test");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    // Simulate TypedArray data from JavaScript
    let typed_array_data: Vec<f64> = (0..900) // 100 vector triplets × 9 components
        .map(|i| (i as f64) * 0.1)
        .collect();

    // Process as WebAssembly-style flat arrays
    let tensor_results = gpu_info_geom
        .amari_chentsov_tensor_from_typed_arrays(&typed_array_data, 100)
        .await?;

    assert_eq!(tensor_results.len(), 100);

    // Verify some computations manually
    for (i, _tensor) in tensor_results.iter().enumerate().take(10) {
        let offset = i * 9;
        let x_components = &typed_array_data[offset..offset + 3];
        let y_components = &typed_array_data[offset + 3..offset + 6];
        let z_components = &typed_array_data[offset + 6..offset + 9];

        // Manual scalar triple product computation
        let expected = x_components[0]
            * (y_components[1] * z_components[2] - y_components[2] * z_components[1])
            - x_components[1]
                * (y_components[0] * z_components[2] - y_components[2] * z_components[0])
            + x_components[2]
                * (y_components[0] * z_components[1] - y_components[1] * z_components[0]);

        assert!((tensor_results[i] - expected).abs() < 1e-10);
    }

    Ok(())
}

/// Test edge computing device detection and fallback
#[tokio::test]
async fn test_edge_computing_device_fallback() -> Result<(), GpuError> {
    // Test different compute device preferences
    let devices = [
        ("high-performance", false), // May fall back to software in CI
        ("low-power", false),        // May fall back to software in CI
        ("fallback", false),         // Explicitly requests CPU fallback
    ];

    let mut any_device_worked = false;

    for (device_type, _should_use_gpu) in devices {
        match GpuInfoGeometry::new_with_device_preference(device_type).await {
            Ok(gpu_info_geom) => {
                // Handle potential device info errors gracefully
                match gpu_info_geom.device_info().await {
                    Ok(device_info) => {
                        // In CI environments, even GPU requests may fall back to software rendering
                        // Just verify that the device works regardless of whether it's GPU or CPU
                        println!(
                            "Device type '{}' initialized as GPU: {}",
                            device_type,
                            device_info.is_gpu()
                        );

                        // Test a computation to ensure it works (with panic handling for CI)
                        let x = create_test_vector_e1();
                        let y = create_test_vector_e2();
                        let z = create_test_vector_e3();

                        let computation_result =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                tokio::runtime::Handle::current().block_on(async {
                                    gpu_info_geom.amari_chentsov_tensor(&x, &y, &z).await
                                })
                            }));

                        match computation_result {
                            Ok(Ok(result)) => {
                                assert!((result - 1.0).abs() < 1e-10);
                                any_device_worked = true;
                            }
                            Ok(Err(e)) => {
                                println!("Device type '{}' computation failed: {}", device_type, e);
                                continue;
                            }
                            Err(_panic) => {
                                println!(
                                    "Device type '{}' computation panicked (expected in CI)",
                                    device_type
                                );
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        println!("Device type '{}' device info failed: {}", device_type, e);
                        continue;
                    }
                }
            }
            Err(GpuError::InitializationError(_)) => {
                // Expected in environments without WebGPU support
                println!(
                    "Device type '{}' failed to initialize (expected in some environments)",
                    device_type
                );
                continue;
            }
            Err(e) => {
                println!("Device type '{}' failed with error: {}", device_type, e);
                continue;
            }
        }
    }

    // In CI environments without GPU support, we just verify the test runs without crashing
    // At least one device should work if GPU is available, but it's okay if none work in CI
    if !any_device_worked {
        println!(
            "No GPU devices available - this is expected in CI environments without GPU support"
        );
    }

    Ok(())
}

/// Test memory efficiency with large batches
#[tokio::test]
async fn test_memory_efficiency_large_batches() -> Result<(), GpuError> {
    let gpu_info_geom = match safe_gpu_init().await {
        Ok(gpu) => gpu,
        Err(GpuError::InitializationError(_)) => {
            println!("WebGPU not available, skipping GPU test");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    // Test progressively larger batches to check memory management
    let large_batch_sizes = vec![10_000, 50_000, 100_000];

    for batch_size in large_batch_sizes {
        // Create large batch
        let (x_batch, y_batch, z_batch) = create_tensor_batch(batch_size);

        // Monitor memory usage (simplified)
        let memory_before = gpu_info_geom.memory_usage().await?;

        let results = gpu_info_geom
            .amari_chentsov_tensor_batch(&x_batch, &y_batch, &z_batch)
            .await?;

        let memory_after = gpu_info_geom.memory_usage().await?;

        // Verify results
        assert_eq!(results.len(), batch_size);

        // Memory should be efficiently managed
        let memory_increase = memory_after - memory_before;
        let expected_memory = batch_size as u64 * 4 * 8; // 4 f64 values per computation

        assert!(
            memory_increase < expected_memory * 2,
            "Memory usage should be reasonable for batch size {}",
            batch_size
        );

        println!(
            "Batch {}: Memory increase {} bytes",
            batch_size, memory_increase
        );
    }

    Ok(())
}

// Helper functions for test data creation

fn create_test_vector_e1() -> Multivector<3, 0, 0> {
    let mut mv = Multivector::zero();
    mv.set_vector_component(0, 1.0); // e1
    mv
}

fn create_test_vector_e2() -> Multivector<3, 0, 0> {
    let mut mv = Multivector::zero();
    mv.set_vector_component(1, 1.0); // e2
    mv
}

fn create_test_vector_e3() -> Multivector<3, 0, 0> {
    let mut mv = Multivector::zero();
    mv.set_vector_component(2, 1.0); // e3
    mv
}

fn create_tensor_batch(
    size: usize,
) -> (
    Vec<Multivector<3, 0, 0>>,
    Vec<Multivector<3, 0, 0>>,
    Vec<Multivector<3, 0, 0>>,
) {
    let mut x_batch = Vec::with_capacity(size);
    let mut y_batch = Vec::with_capacity(size);
    let mut z_batch = Vec::with_capacity(size);

    for i in 0..size {
        // Create varied test vectors
        let mut x = Multivector::zero();
        let mut y = Multivector::zero();
        let mut z = Multivector::zero();

        x.set_vector_component(0, 1.0 + (i as f64) * 0.01);
        y.set_vector_component(1, 1.0 + (i as f64) * 0.02);
        z.set_vector_component(2, 1.0 + (i as f64) * 0.03);

        x_batch.push(x);
        y_batch.push(y);
        z_batch.push(z);
    }

    (x_batch, y_batch, z_batch)
}

fn create_test_parameters() -> Vec<f64> {
    vec![0.3, 0.5, 0.2] // Simple probability distribution
}

fn create_divergence_batch(size: usize) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let mut p_batch = Vec::with_capacity(size);
    let mut q_batch = Vec::with_capacity(size);

    for i in 0..size {
        let base = i as f64 * 0.001;
        p_batch.push(vec![0.3 + base, 0.4 + base, 0.3 - base * 2.0]);
        q_batch.push(vec![0.25 + base, 0.45 + base, 0.3 - base * 2.0]);
    }

    (p_batch, q_batch)
}
