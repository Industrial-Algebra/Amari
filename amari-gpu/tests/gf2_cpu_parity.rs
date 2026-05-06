#![cfg(feature = "gf2")]

mod common;

use amari_core::gf2::{BinaryMultivector, GF2Matrix, GF2Vector};
use amari_gpu::{GF2GpuOps, GpuGF2CliffordPair, GpuGF2HammingPair, GpuGF2MatVecData};
use common::direct_gpu_runtime_available;

fn binary_to_words<const N: usize, const R: usize>(mv: &BinaryMultivector<N, R>) -> [u32; 4] {
    let mut words = [0u32; 4];
    for blade in mv.nonzero_blades() {
        if blade < 128 {
            words[blade / 32] |= 1u32 << (blade % 32);
        }
    }
    words
}

fn binary_from_word<const N: usize, const R: usize>(word: u32) -> BinaryMultivector<N, R> {
    let mut bits = vec![0u8; BinaryMultivector::<N, R>::BASIS_COUNT];
    for (i, bit) in bits.iter_mut().enumerate() {
        if ((word >> i) & 1) != 0 {
            *bit = 1;
        }
    }
    BinaryMultivector::<N, R>::from_bits(&bits)
}

fn clifford_pair<const N: usize, const R: usize>(a: u32, b: u32) -> GpuGF2CliffordPair {
    GpuGF2CliffordPair {
        a_words: [a, 0, 0, 0],
        b_words: [b, 0, 0, 0],
        num_generators: (N + R) as u32,
        num_degenerate: R as u32,
        padding: [0; 2],
    }
}

fn vector_to_low_u32(vector: &GF2Vector) -> u32 {
    vector.as_words().first().copied().unwrap_or(0) as u32
}

fn lcg(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *seed
}

#[tokio::test]
async fn test_gf2_gpu_matches_cpu_baselines_and_laws() {
    if !direct_gpu_runtime_available() {
        return;
    }

    let mut gpu = match GF2GpuOps::new().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    // Clifford parity: compare a representative Cl(3,0;F₂) batch against
    // amari-core's BinaryMultivector CPU implementation.
    let cl3_cases = [
        (0b0000_0010, 0b0000_0100),
        (0b0000_1011, 0b0000_0110),
        (0b1010_0101, 0b0101_1010),
        (0b1111_1111, 0b0001_0011),
    ];
    let cl3_pairs: Vec<_> = cl3_cases
        .iter()
        .map(|&(a, b)| clifford_pair::<3, 0>(a, b))
        .collect();
    let gpu_cl3 = gpu.batch_gf2_geometric_product(&cl3_pairs).await.unwrap();
    for ((a, b), gpu_words) in cl3_cases.iter().zip(gpu_cl3.iter()) {
        let cpu = binary_from_word::<3, 0>(*a).geometric_product(&binary_from_word::<3, 0>(*b));
        assert_eq!(*gpu_words, binary_to_words(&cpu));
    }

    // Degenerate Clifford parity: e3 is degenerate in Cl(2,1;F₂), so e3*e3 = 0.
    let cl21_cases = [
        // In Cl(2,1), the degenerate basis vector is generator 2,
        // whose blade index is 1 << 2 = 4, so its coefficient bit is bit 4.
        (0b0001_0000, 0b0001_0000),
        (0b0001_0010, 0b0001_0100),
        (0b0011_0011, 0b0101_0101),
    ];
    let cl21_pairs: Vec<_> = cl21_cases
        .iter()
        .map(|&(a, b)| clifford_pair::<2, 1>(a, b))
        .collect();
    let gpu_cl21 = gpu.batch_gf2_geometric_product(&cl21_pairs).await.unwrap();
    for ((a, b), gpu_words) in cl21_cases.iter().zip(gpu_cl21.iter()) {
        let cpu = binary_from_word::<2, 1>(*a).geometric_product(&binary_from_word::<2, 1>(*b));
        assert_eq!(*gpu_words, binary_to_words(&cpu));
    }
    assert_eq!(gpu_cl21[0], [0, 0, 0, 0]);

    // Algebraic laws via GPU results: associativity and left distributivity in Cl(3,0;F₂).
    let a = 0b0000_1011;
    let b = 0b0101_0010;
    let c = 0b0011_0101;
    let ab = gpu
        .batch_gf2_geometric_product(&[clifford_pair::<3, 0>(a, b)])
        .await
        .unwrap()[0];
    let bc = gpu
        .batch_gf2_geometric_product(&[clifford_pair::<3, 0>(b, c)])
        .await
        .unwrap()[0];
    let ab_c = gpu
        .batch_gf2_geometric_product(&[clifford_pair::<3, 0>(ab[0], c)])
        .await
        .unwrap()[0];
    let a_bc = gpu
        .batch_gf2_geometric_product(&[clifford_pair::<3, 0>(a, bc[0])])
        .await
        .unwrap()[0];
    assert_eq!(ab_c, a_bc, "GF(2) Clifford product should be associative");

    let a_plus_b = a ^ b;
    let lhs = gpu
        .batch_gf2_geometric_product(&[clifford_pair::<3, 0>(a_plus_b, c)])
        .await
        .unwrap()[0];
    let ac = gpu
        .batch_gf2_geometric_product(&[clifford_pair::<3, 0>(a, c)])
        .await
        .unwrap()[0];
    let bc = gpu
        .batch_gf2_geometric_product(&[clifford_pair::<3, 0>(b, c)])
        .await
        .unwrap()[0];
    assert_eq!(lhs[0], ac[0] ^ bc[0], "left distributivity over XOR");

    // Matrix-vector parity: deterministic sweep across shapes and row patterns.
    let matrices_and_vectors = [
        (
            GF2Matrix::from_rows(vec![
                GF2Vector::from_bits(&[1, 0, 1, 1]),
                GF2Vector::from_bits(&[0, 1, 1, 0]),
                GF2Vector::from_bits(&[1, 1, 0, 1]),
            ]),
            GF2Vector::from_bits(&[1, 1, 0, 1]),
        ),
        (
            GF2Matrix::from_rows(vec![
                GF2Vector::from_bits(&[1, 1, 1, 0, 0]),
                GF2Vector::from_bits(&[0, 1, 0, 1, 1]),
            ]),
            GF2Vector::from_bits(&[1, 0, 1, 1, 0]),
        ),
        (
            GF2Matrix::identity(8),
            GF2Vector::from_bits(&[1, 0, 1, 0, 1, 1, 0, 1]),
        ),
    ];
    let matvec_data: Vec<_> = matrices_and_vectors
        .iter()
        .map(|(m, v)| GpuGF2MatVecData::from_matrix_and_vector(m, v))
        .collect();
    let gpu_matvec = gpu.batch_gf2_matvec(&matvec_data).await.unwrap();
    for ((matrix, vector), gpu_word) in matrices_and_vectors.iter().zip(gpu_matvec.iter()) {
        let cpu = matrix.mul_vec(vector);
        assert_eq!(*gpu_word, vector_to_low_u32(&cpu));
    }

    // Hamming parity, including cases where high unused bits are set in the final word.
    let hamming_pairs = vec![
        GpuGF2HammingPair {
            a_words: [0b1111, 0, 0, 0],
            b_words: [0, 0, 0, 0],
            dim: 3,
            padding: [0; 3],
        },
        GpuGF2HammingPair::from_vectors(
            &GF2Vector::from_bits(&[1, 0, 1, 1, 0, 1, 0]),
            &GF2Vector::from_bits(&[0, 0, 1, 0, 0, 1, 1]),
        ),
        GpuGF2HammingPair {
            a_words: [0xFFFF_0000, 0xF0F0_F0F0, 0, 0],
            b_words: [0x0000_FFFF, 0x0F0F_0F0F, 0, 0],
            dim: 40,
            padding: [0; 3],
        },
    ];
    let gpu_distances = gpu
        .batch_gf2_hamming_distance(&hamming_pairs)
        .await
        .unwrap();
    assert_eq!(gpu_distances[0], 3);
    assert_eq!(gpu_distances[1], 3);
    let masked_second_word_diff = 0xFFu32;
    let expected_third = 32 + masked_second_word_diff.count_ones();
    assert_eq!(gpu_distances[2], expected_third);

    // Lightweight deterministic property sweep for matvec linearity:
    // M(x+y) = Mx + My over GF(2).
    let mut seed = 0x000A_11CE_u32;
    for _ in 0..12 {
        let rows: Vec<_> = (0..4)
            .map(|_| {
                GF2Vector::from_bits(
                    &(0..8)
                        .map(|_| (lcg(&mut seed) & 1) as u8)
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let matrix = GF2Matrix::from_rows(rows);
        let x = GF2Vector::from_bits(
            &(0..8)
                .map(|_| (lcg(&mut seed) & 1) as u8)
                .collect::<Vec<_>>(),
        );
        let y = GF2Vector::from_bits(
            &(0..8)
                .map(|_| (lcg(&mut seed) & 1) as u8)
                .collect::<Vec<_>>(),
        );
        let x_plus_y = &x + &y;

        let data = vec![
            GpuGF2MatVecData::from_matrix_and_vector(&matrix, &x_plus_y),
            GpuGF2MatVecData::from_matrix_and_vector(&matrix, &x),
            GpuGF2MatVecData::from_matrix_and_vector(&matrix, &y),
        ];
        let result = gpu.batch_gf2_matvec(&data).await.unwrap();
        assert_eq!(result[0], result[1] ^ result[2]);

        let cpu = matrix.mul_vec(&x_plus_y);
        assert_eq!(result[0], vector_to_low_u32(&cpu));
    }
}
