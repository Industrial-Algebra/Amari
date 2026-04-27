#![cfg(feature = "gf2")]

use amari_core::gf2::{GF2Matrix, GF2Vector, GF2};
use amari_gpu::{
    GF2GpuContext, GF2GpuError, GF2GpuOps, GF2GpuResult, GpuGF2CliffordPair, GpuGF2HammingPair,
    GpuGF2MatVecData,
};

#[test]
fn test_gf2_public_constructors_and_errors() {
    let explicit_result: GF2GpuResult<()> = Ok(());
    assert!(explicit_result.is_ok());
    let explicit_error = GF2GpuError::Computation("expected".to_string());
    assert!(explicit_error.to_string().contains("expected"));

    let _context_new = GF2GpuContext::new;

    let pair = GpuGF2CliffordPair::from_bits(&[0, 1], &[0, 0, 1], 3, 0);
    assert_eq!(pair.a_words[0], 0b10);
    assert_eq!(pair.b_words[0], 0b100);
    assert_eq!(pair.num_generators, 3);

    let mut matrix = GF2Matrix::zero(2, 3);
    matrix.set(0, 0, GF2::ONE);
    matrix.set(0, 2, GF2::ONE);
    matrix.set(1, 1, GF2::ONE);
    let vector = GF2Vector::from_bits(&[1, 1, 0]);
    let matvec = GpuGF2MatVecData::from_matrix_and_vector(&matrix, &vector);
    assert_eq!(matvec.matrix_rows[0], 0b101);
    assert_eq!(matvec.matrix_rows[1], 0b010);
    assert_eq!(matvec.vector, 0b011);

    let a = GF2Vector::from_bits(&[1, 0, 1]);
    let b = GF2Vector::from_bits(&[0, 1, 1]);
    let hamming = GpuGF2HammingPair::from_vectors(&a, &b);
    assert_eq!(hamming.a_words[0], 0b101);
    assert_eq!(hamming.b_words[0], 0b110);
    assert_eq!(hamming.dim, 3);
}

#[tokio::test]
async fn test_gf2_public_gpu_ops_and_validation() {
    let mut gpu = match GF2GpuOps::new().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    assert!(gpu
        .batch_gf2_geometric_product(&[])
        .await
        .unwrap()
        .is_empty());
    assert!(gpu.batch_gf2_matvec(&[]).await.unwrap().is_empty());
    assert!(gpu
        .batch_gf2_hamming_distance(&[])
        .await
        .unwrap()
        .is_empty());

    let products = gpu
        .batch_gf2_geometric_product(&[
            GpuGF2CliffordPair::from_bits(&[0, 1], &[0, 0, 1], 3, 0),
            GpuGF2CliffordPair::from_bits(&[1], &[0, 1], 3, 0),
        ])
        .await
        .unwrap();
    assert_eq!(products.len(), 2);
    assert_eq!(products[0][0] & (1 << 3), 1 << 3);
    assert_eq!(products[1][0] & (1 << 1), 1 << 1);

    let invalid_clifford = GpuGF2CliffordPair::from_bits(&[1], &[1], 8, 0);
    assert!(matches!(
        gpu.batch_gf2_geometric_product(&[invalid_clifford]).await,
        Err(GF2GpuError::Computation(_))
    ));

    let matrix = GF2Matrix::identity(3);
    let vector = GF2Vector::from_bits(&[1, 0, 1]);
    let matvec = GpuGF2MatVecData::from_matrix_and_vector(&matrix, &vector);
    assert_eq!(gpu.batch_gf2_matvec(&[matvec]).await.unwrap(), vec![0b101]);

    let invalid_matvec = GpuGF2MatVecData {
        matrix_rows: [0; 16],
        vector: 0,
        nrows: 17,
        ncols: 1,
        padding: 0,
    };
    assert!(matches!(
        gpu.batch_gf2_matvec(&[invalid_matvec]).await,
        Err(GF2GpuError::Computation(_))
    ));

    let hamming = GpuGF2HammingPair {
        a_words: [0b1111, 0, 0, 0],
        b_words: [0, 0, 0, 0],
        dim: 3,
        padding: [0; 3],
    };
    assert_eq!(
        gpu.batch_gf2_hamming_distance(&[hamming]).await.unwrap(),
        vec![3]
    );

    let invalid_hamming = GpuGF2HammingPair {
        a_words: [0; 4],
        b_words: [0; 4],
        dim: 129,
        padding: [0; 3],
    };
    assert!(matches!(
        gpu.batch_gf2_hamming_distance(&[invalid_hamming]).await,
        Err(GF2GpuError::Computation(_))
    ));
}
