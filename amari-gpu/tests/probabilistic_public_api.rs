#![cfg(feature = "probabilistic")]

mod common;

use amari_gpu::{GpuProbabilistic, GpuProbabilisticError, GpuProbabilisticResult};
use common::direct_gpu_runtime_available;

#[test]
fn test_probabilistic_public_errors_and_pre_gpu_validation() {
    let explicit_result: GpuProbabilisticResult<()> = Ok(());
    assert!(explicit_result.is_ok());
    let explicit_error = GpuProbabilisticError::InvalidParameters("expected".to_string());
    assert!(explicit_error.to_string().contains("expected"));

    let zero_dimension = pollster::block_on(GpuProbabilistic::new(0));
    assert!(matches!(
        zero_dimension,
        Err(GpuProbabilisticError::InvalidParameters(_))
    ));
}

#[tokio::test]
async fn test_probabilistic_public_api_validation_and_cpu_gpu_parity() {
    if !direct_gpu_runtime_available() {
        return;
    }
    let gpu = match GpuProbabilistic::new(3).await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };
    assert_eq!(gpu.dimension(), 3);

    assert!(matches!(
        gpu.batch_sample_gaussian(4, &[0.0, 1.0], &[1.0, 1.0, 1.0])
            .await,
        Err(GpuProbabilisticError::DimensionMismatch { .. })
    ));
    assert!(matches!(
        gpu.batch_sample_gaussian(4, &[0.0, 1.0, 2.0], &[1.0, -1.0, 1.0])
            .await,
        Err(GpuProbabilisticError::InvalidParameters(_))
    ));
    assert!(matches!(
        gpu.batch_mean(&[]).await,
        Err(GpuProbabilisticError::InvalidParameters(_))
    ));
    assert!(matches!(
        gpu.batch_mean(&[1.0, 2.0]).await,
        Err(GpuProbabilisticError::DimensionMismatch { .. })
    ));
    assert!(matches!(
        gpu.batch_mean(&[1.0, 2.0, 3.0, 4.0, 5.0]).await,
        Err(GpuProbabilisticError::DimensionMismatch {
            expected: 3,
            actual: 5
        })
    ));
    assert!(matches!(
        gpu.batch_variance(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]).await,
        Err(GpuProbabilisticError::InvalidParameters(_))
    ));

    // Small batches use the documented CPU fallback after GPU context creation.
    let small_samples = vec![
        1.0, 2.0, 3.0, // sample 1
        2.0, 3.0, 4.0, // sample 2
        3.0, 4.0, 5.0, // sample 3
    ];
    let small_mean = gpu.batch_mean(&small_samples).await.unwrap();
    assert_eq!(small_mean, vec![2.0, 3.0, 4.0]);
    let small_variance = gpu
        .batch_variance(&small_samples, &small_mean)
        .await
        .unwrap();
    assert_eq!(small_variance, vec![1.0, 1.0, 1.0]);

    // Large batches use the GPU kernels; compare against exact CPU expectations.
    let mut large_samples = Vec::new();
    for i in 0..128 {
        large_samples.push(i as f64);
        large_samples.push((2 * i) as f64);
        large_samples.push(5.0);
    }
    let large_mean = gpu.batch_mean(&large_samples).await.unwrap();
    assert!((large_mean[0] - 63.5).abs() < 1e-4);
    assert!((large_mean[1] - 127.0).abs() < 1e-4);
    assert!((large_mean[2] - 5.0).abs() < 1e-4);

    let large_variance = gpu
        .batch_variance(&large_samples, &large_mean)
        .await
        .unwrap();
    let expected_var0 = (0..128)
        .map(|i| {
            let d = i as f64 - 63.5;
            d * d
        })
        .sum::<f64>()
        / 127.0;
    assert!((large_variance[0] - expected_var0).abs() < 1e-3);
    assert!((large_variance[1] - 4.0 * expected_var0).abs() < 1e-2);
    assert!(large_variance[2].abs() < 1e-6);

    // Zero standard deviation gives deterministic samples and exercises the GPU
    // sampling path without statistical tolerances.
    let deterministic = gpu
        .batch_sample_gaussian(128, &[1.0, -2.0, 0.5], &[0.0, 0.0, 0.0])
        .await
        .unwrap();
    assert_eq!(deterministic.len(), 128 * 3);
    for chunk in deterministic.chunks_exact(3) {
        assert!((chunk[0] - 1.0).abs() < 1e-6);
        assert!((chunk[1] + 2.0).abs() < 1e-6);
        assert!((chunk[2] - 0.5).abs() < 1e-6);
    }
}
