#![cfg(feature = "holographic")]

mod common;

use amari_gpu::holographic::ProductCl3x32;
use amari_gpu::{GpuHolographic, GpuHolographicError, GpuHolographicMemory, GpuOpticalField};
use amari_holographic::optical::{LeeEncoderConfig, OpticalRotorField};
use amari_holographic::BindingAlgebra;
use common::direct_gpu_runtime_available;

fn assert_close(actual: f64, expected: f64, tol: f64) {
    assert!(
        (actual - expected).abs() <= tol,
        "expected {expected}, got {actual}"
    );
}

fn make_basis_batch(batch_size: usize, key_idx: usize, value_idx: usize) -> (Vec<f64>, Vec<f64>) {
    let dim = ProductCl3x32::DIMENSION;
    let mut keys = vec![0.0; batch_size * dim];
    let mut values = vec![0.0; batch_size * dim];
    for batch in 0..batch_size {
        let start = batch * dim;
        keys[start + key_idx] = 1.0;
        values[start + value_idx] = 1.0;
    }
    (keys, values)
}

fn cpu_bind_expected(keys: &[f64], values: &[f64]) -> Vec<f64> {
    let dim = ProductCl3x32::DIMENSION;
    let mut expected = Vec::with_capacity(keys.len());
    for (key, value) in keys.chunks_exact(dim).zip(values.chunks_exact(dim)) {
        let key = ProductCl3x32::from_coefficients(key).unwrap();
        let value = ProductCl3x32::from_coefficients(value).unwrap();
        expected.extend(key.bind(&value).to_coefficients());
    }
    expected
}

#[test]
fn test_holographic_public_imports_and_pre_gpu_validation() {
    assert!(!GpuHolographic::should_use_gpu(99));
    assert!(GpuHolographic::should_use_gpu(100));
    assert!(!GpuOpticalField::should_use_gpu(4095));
    assert!(GpuOpticalField::should_use_gpu(4096));

    let memory_type = std::any::type_name::<GpuHolographicMemory>();
    assert!(memory_type.contains("GpuHolographicMemory"));

    let unsupported = futures::executor::block_on(GpuHolographic::new(128));
    assert!(matches!(
        unsupported,
        Err(GpuHolographicError::DimensionMismatch {
            expected: 256,
            actual: 128
        })
    ));

    let invalid_optical = futures::executor::block_on(GpuOpticalField::new((0, 16)));
    assert!(matches!(
        invalid_optical,
        Err(GpuHolographicError::DimensionMismatch { .. })
    ));
}

#[tokio::test]
async fn test_holographic_gpu_batch_ops_when_available() {
    if !direct_gpu_runtime_available() {
        return;
    }
    let gpu = match GpuHolographic::new_product_cl3x32().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    assert!(gpu.batch_bind(&[], &[]).await.unwrap().is_empty());
    assert!(matches!(
        gpu.batch_similarity(&[1.0, 2.0], &[1.0, 2.0]).await,
        Err(GpuHolographicError::DimensionMismatch { .. })
    ));
    let mut nonfinite = vec![0.0; ProductCl3x32::DIMENSION];
    nonfinite[7] = f64::INFINITY;
    assert!(matches!(
        gpu.batch_bind(&nonfinite, &vec![0.0; ProductCl3x32::DIMENSION])
            .await,
        Err(GpuHolographicError::BufferError(_))
    ));
    assert!(matches!(
        gpu.find_most_similar(&vec![0.0; ProductCl3x32::DIMENSION], &[])
            .await,
        Err(GpuHolographicError::DimensionMismatch { .. })
    ));

    // e2 * e1 = -e12 in amari-holographic's Cl3 basis. This catches the
    // formerly over-simplified GPU sign/index path.
    let (keys, values) = make_basis_batch(100, 2, 1);
    let gpu_bound = gpu.batch_bind(&keys, &values).await.unwrap();
    let expected = cpu_bind_expected(&keys, &values);
    assert_eq!(gpu_bound.len(), expected.len());
    for (actual, expected) in gpu_bound.iter().zip(expected.iter()) {
        assert_close(*actual, *expected, 1e-6);
    }

    let similarities = gpu.batch_similarity(&keys, &keys).await.unwrap();
    assert_eq!(similarities.len(), 100);
    for sim in similarities {
        assert_close(sim, 1.0, 1e-6);
    }

    // Bundling intentionally follows the CPU correctness path for v1.
    let bundled = gpu.batch_bundle(&keys, &values).await.unwrap();
    assert_eq!(bundled.len(), keys.len());
}

#[tokio::test]
async fn test_optical_public_ops_when_available() {
    if !direct_gpu_runtime_available() {
        return;
    }
    let dims = (16, 16);
    let gpu = match GpuOpticalField::new(dims).await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    assert_eq!(gpu.dimensions(), dims);
    assert_eq!(gpu.field_size(), 256);

    let field_a = OpticalRotorField::uniform(0.0, 1.0, dims);
    let field_b = OpticalRotorField::uniform(std::f32::consts::FRAC_PI_4, 1.0, dims);
    let bound = gpu.bind(&field_a, &field_b).await.unwrap();
    assert!((bound.phase_at(0, 0) - std::f32::consts::FRAC_PI_4).abs() < 1e-5);

    let self_sim = gpu.similarity(&field_a, &field_a).await.unwrap();
    assert!((self_sim - 1.0).abs() < 1e-6);

    let sims = gpu
        .batch_similarity(
            &[field_a.clone(), field_b.clone()],
            &[field_a.clone(), field_b.clone()],
        )
        .await
        .unwrap();
    assert_eq!(sims.len(), 2);
    assert!((sims[0] - 1.0).abs() < 1e-6);
    assert!((sims[1] - 1.0).abs() < 1e-6);

    let config = LeeEncoderConfig::new(dims, 0.25);
    let hologram = gpu.encode_lee(&field_a, &config).await.unwrap();
    assert_eq!(hologram.dimensions(), dims);
    assert_eq!(hologram.len(), 256);

    let mismatched = OpticalRotorField::uniform(0.0, 1.0, (8, 8));
    assert!(matches!(
        gpu.bind(&field_a, &mismatched).await,
        Err(GpuHolographicError::DimensionMismatch { .. })
    ));
}
