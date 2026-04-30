#![cfg(feature = "functional")]

mod common;

use amari_core::Multivector;
use amari_functional::{LinearOperator, MatrixOperator};
use amari_gpu::{
    AdaptiveFunctionalCompute, GpuHilbertSpace, GpuMatrixOperator, GpuSpectralDecomposition,
};
use common::direct_gpu_runtime_available;

fn mv<const P: usize, const Q: usize, const R: usize>(coeffs: &[f64]) -> Multivector<P, Q, R> {
    Multivector::from_coefficients(coeffs.to_vec())
}

fn assert_mv_close<const P: usize, const Q: usize, const R: usize>(
    actual: &Multivector<P, Q, R>,
    expected: &Multivector<P, Q, R>,
    tolerance: f64,
) {
    for (a, e) in actual.to_vec().iter().zip(expected.to_vec().iter()) {
        assert!((a - e).abs() <= tolerance, "actual={a}, expected={e}");
    }
}

#[tokio::test]
async fn test_functional_public_api_paths() {
    if !direct_gpu_runtime_available() {
        return;
    }
    let matrix = MatrixOperator::<2, 0, 0>::diagonal(&[2.0, 3.0, 4.0, 5.0]).unwrap();
    let vectors = vec![
        mv::<2, 0, 0>(&[1.0, 2.0, 3.0, 4.0]),
        mv::<2, 0, 0>(&[-1.0, 0.5, 0.0, 2.0]),
    ];

    if let Ok(gpu_matrix) = GpuMatrixOperator::from_matrix_operator(&matrix).await {
        assert_eq!(gpu_matrix.dimension(), 4);

        let applied = gpu_matrix.apply_batch(&vectors).await.unwrap();
        assert_eq!(applied.len(), vectors.len());
        for (actual, input) in applied.iter().zip(vectors.iter()) {
            let expected = matrix.apply(input).unwrap();
            assert_mv_close(actual, &expected, 1e-5);
        }

        let identity = MatrixOperator::<2, 0, 0>::identity();
        let gpu_identity = GpuMatrixOperator::from_matrix_operator(&identity)
            .await
            .unwrap();
        let product = gpu_matrix.multiply(&gpu_identity).await.unwrap();
        for row in 0..4 {
            for col in 0..4 {
                assert!((product.get(row, col) - matrix.get(row, col)).abs() < 1e-5);
            }
        }

        let roundtrip = gpu_matrix.to_matrix_operator().await.unwrap();
        for row in 0..4 {
            for col in 0..4 {
                assert!((roundtrip.get(row, col) - matrix.get(row, col)).abs() < 1e-5);
            }
        }

        let decomp = GpuSpectralDecomposition::compute(&gpu_matrix, 100, 1e-10)
            .await
            .unwrap();
        assert!(decomp.is_complete());
        assert_eq!(decomp.eigenvalues().len(), 4);
        assert!((decomp.spectral_radius() - 5.0).abs() < 1e-8);
        assert_eq!(decomp.condition_number().unwrap().round() as i32, 3);
        assert!(decomp.is_positive_definite());
        assert!(decomp.is_positive_semidefinite());

        let squared_batch = decomp
            .apply_function_batch(|lambda| lambda * lambda, &vectors)
            .await;
        assert_eq!(squared_batch.len(), vectors.len());
    }

    if let Ok(hilbert) = GpuHilbertSpace::<2, 0, 0>::new().await {
        let products = hilbert
            .inner_product_batch(&vectors, &vectors)
            .await
            .unwrap();
        assert_eq!(products.len(), vectors.len());
        assert!((products[0] - 30.0).abs() < 1e-5);
        assert!((products[1] - 5.25).abs() < 1e-5);

        let norms = hilbert.norm_batch(&vectors).await.unwrap();
        assert_eq!(norms.len(), vectors.len());
        assert!((norms[0] - 30.0_f64.sqrt()).abs() < 1e-5);
        assert!((norms[1] - 5.25_f64.sqrt()).abs() < 1e-5);

        let mismatch = hilbert.inner_product_batch(&vectors, &vectors[..1]).await;
        assert!(mismatch.is_err());
    }

    let adaptive = AdaptiveFunctionalCompute::<2, 0, 0>::new().await;
    let adaptive_applied = adaptive.apply_batch(&matrix, &vectors).await.unwrap();
    assert_eq!(adaptive_applied.len(), vectors.len());
    for (actual, input) in adaptive_applied.iter().zip(vectors.iter()) {
        let expected = matrix.apply(input).unwrap();
        assert_mv_close(actual, &expected, 1e-12);
    }
}
