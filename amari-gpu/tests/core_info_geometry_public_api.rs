mod common;

use amari_core::Multivector;
use amari_gpu::{AdaptiveCompute, GpuCliffordAlgebra, GpuError, GpuInfoGeometry};
use common::direct_gpu_runtime_available;

fn assert_close(actual: f64, expected: f64, tol: f64) {
    assert!(
        (actual - expected).abs() <= tol,
        "expected {expected}, got {actual}"
    );
}

#[tokio::test]
async fn test_adaptive_core_ga_cpu_baseline_and_validation() {
    if !direct_gpu_runtime_available() {
        return;
    }

    let adaptive = AdaptiveCompute::new::<3, 0, 0>().await;

    let e1 = Multivector::<3, 0, 0>::basis_vector(0);
    let e2 = Multivector::<3, 0, 0>::basis_vector(1);
    let single = adaptive.geometric_product(&e1, &e2).await;
    assert_eq!(single.to_vec(), e1.geometric_product(&e2).to_vec());

    assert!(adaptive
        .batch_geometric_product(&[], &[])
        .await
        .unwrap()
        .is_empty());
    assert!(matches!(
        adaptive
            .batch_geometric_product(&[1.0, 2.0], &[1.0, 2.0])
            .await,
        Err(GpuError::BufferError(_))
    ));
    assert!(matches!(
        adaptive
            .batch_geometric_product(&[f64::NAN; 8], &[0.0; 8])
            .await,
        Err(GpuError::BufferError(_))
    ));

    let a = e1.to_vec();
    let b = e2.to_vec();
    let result = adaptive.batch_geometric_product(&a, &b).await.unwrap();
    assert_eq!(result, e1.geometric_product(&e2).to_vec());
}

#[tokio::test]
async fn test_direct_core_ga_gpu_path_when_available() {
    if !direct_gpu_runtime_available() {
        return;
    }

    let gpu = match GpuCliffordAlgebra::new::<3, 0, 0>().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    assert_eq!(gpu.dimension(), 3);
    assert_eq!(gpu.basis_count(), 8);
    assert!(gpu
        .batch_geometric_product(&[], &[])
        .await
        .unwrap()
        .is_empty());
    assert!(matches!(
        gpu.batch_geometric_product(&[1.0, 2.0], &[1.0, 2.0]).await,
        Err(GpuError::BufferError(_))
    ));

    let e2 = Multivector::<3, 0, 0>::basis_vector(1);
    let e1 = Multivector::<3, 0, 0>::basis_vector(0);
    let expected = e2.geometric_product(&e1).to_vec();
    let result = gpu
        .batch_geometric_product(&e2.to_vec(), &e1.to_vec())
        .await
        .unwrap();
    assert_eq!(result.len(), expected.len());
    for (actual, expected) in result.iter().zip(expected.iter()) {
        assert_close(*actual, *expected, 1e-6);
    }

    // The shader is generated with the signature-specific basis count; this
    // catches the former hard-coded 8-basis shader when a 4D algebra is used.
    let gpu4 = match GpuCliffordAlgebra::new::<4, 0, 0>().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };
    assert_eq!(gpu4.dimension(), 4);
    assert_eq!(gpu4.basis_count(), 16);
    let scalar = {
        let mut coeffs = vec![0.0; 16];
        coeffs[0] = 2.0;
        coeffs
    };
    let product = gpu4
        .batch_geometric_product(&scalar, &scalar)
        .await
        .unwrap();
    assert_eq!(product.len(), 16);
    assert_close(product[0], 4.0, 1e-6);
    assert!(product[1..].iter().all(|value| value.abs() < 1e-6));
}

#[tokio::test]
async fn test_info_geometry_public_cpu_baselines_when_available() {
    if !direct_gpu_runtime_available() {
        return;
    }

    let gpu = match GpuInfoGeometry::new().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    let e1 = Multivector::<3, 0, 0>::basis_vector(0);
    let e2 = Multivector::<3, 0, 0>::basis_vector(1);
    let e3 = Multivector::<3, 0, 0>::basis_vector(2);
    let tensor = gpu.amari_chentsov_tensor(&e1, &e2, &e3).await.unwrap();
    assert_close(tensor, 1.0, 1e-12);

    let batch = gpu
        .amari_chentsov_tensor_batch(
            std::slice::from_ref(&e1),
            std::slice::from_ref(&e2),
            std::slice::from_ref(&e3),
        )
        .await
        .unwrap();
    assert_eq!(batch, vec![1.0]);
    assert!(matches!(
        gpu.amari_chentsov_tensor_batch(std::slice::from_ref(&e1), &[], std::slice::from_ref(&e3))
            .await,
        Err(GpuError::BufferError(_))
    ));

    let flat = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let from_flat = gpu
        .amari_chentsov_tensor_from_typed_arrays(&flat, 1)
        .await
        .unwrap();
    assert_eq!(from_flat, vec![1.0]);
    assert!(matches!(
        gpu.amari_chentsov_tensor_from_typed_arrays(&[1.0, f64::INFINITY], 0)
            .await,
        Err(GpuError::BufferError(_))
    ));

    let fisher = gpu.fisher_information_matrix(&[0.25, 0.5]).await.unwrap();
    assert_eq!(fisher.dimension(), 2);
    assert_eq!(fisher.matrix(), &[vec![4.0, 0.0], vec![0.0, 2.0]]);
    assert_eq!(fisher.eigenvalues().await.unwrap(), vec![4.0, 2.0]);
    assert!(matches!(
        gpu.fisher_information_matrix(&[0.5, -0.1]).await,
        Err(GpuError::BufferError(_))
    ));

    let divergences = gpu
        .bregman_divergence_batch(&[vec![0.5, 0.5]], &[vec![0.25, 0.75]])
        .await
        .unwrap();
    let expected = 0.5_f64 * (0.5_f64 / 0.25_f64).ln() + 0.5_f64 * (0.5_f64 / 0.75_f64).ln();
    assert_close(divergences[0], expected, 1e-12);
    assert!(matches!(
        gpu.bregman_divergence_batch(&[vec![0.5]], &[vec![0.25, 0.75]])
            .await,
        Err(GpuError::BufferError(_))
    ));

    let info = gpu.device_info().await.unwrap();
    assert!(info.is_initialized());
    assert!(info.supports_webgpu());
    assert!(info.description().contains("WebGPU"));
    assert_eq!(gpu.memory_usage().await.unwrap(), 0);
}
