#![cfg(feature = "topology")]

mod common;

use amari_gpu::{AdaptiveTopologyCompute, GpuTopology};
use amari_topology::{CriticalType, Simplex, SimplicialComplex};
use common::direct_gpu_runtime_available;

fn assert_distance_matrix_close(actual: &[f64], expected: &[f64], tolerance: f64) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected.iter()) {
        assert!((a - e).abs() <= tolerance, "actual={a}, expected={e}");
    }
}

fn peak_grid() -> (Vec<f64>, usize, usize) {
    let width = 5;
    let height = 5;
    let mut values = vec![0.0; width * height];
    for y in 0..height {
        for x in 0..width {
            let dx = x as f64 - 2.0;
            let dy = y as f64 - 2.0;
            values[y * width + x] = -(dx * dx + dy * dy);
        }
    }
    (values, width, height)
}

#[tokio::test]
async fn test_topology_public_api_paths() {
    if !direct_gpu_runtime_available() {
        return;
    }
    let points = vec![(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)];
    let expected_distances = vec![
        0.0,
        1.0,
        1.0,
        1.0,
        0.0,
        2.0_f64.sqrt(),
        1.0,
        2.0_f64.sqrt(),
        0.0,
    ];

    let adaptive = AdaptiveTopologyCompute::new().await;
    let distances = adaptive.compute_distance_matrix(&points).await.unwrap();
    assert_distance_matrix_close(&distances, &expected_distances, 1e-10);

    let filtration = adaptive
        .build_rips_filtration(&points, 1.01, 2)
        .await
        .unwrap();
    assert!(filtration
        .iter()
        .any(|(simplex, time)| simplex == &[0] && *time == 0.0));
    assert!(filtration
        .iter()
        .any(|(simplex, time)| simplex == &[0, 1] && (*time - 1.0).abs() < 1e-10));
    assert!(!filtration.iter().any(|(simplex, _)| simplex.len() == 3));

    let (values, width, height) = peak_grid();
    let critical_points = adaptive
        .find_critical_points_2d(&values, width, height)
        .await
        .unwrap();
    assert!(critical_points
        .iter()
        .any(|cp| cp.position == (2, 2) && matches!(cp.critical_type, CriticalType::Maximum)));

    assert!(adaptive
        .find_critical_points_2d(&values, width - 1, height)
        .await
        .is_err());
    assert!(adaptive
        .find_critical_points_2d(&[0.0; 4], 2, 2)
        .await
        .is_err());

    if let Ok(gpu) = GpuTopology::new().await {
        let gpu_distances = gpu.compute_distance_matrix(&points).await.unwrap();
        assert_distance_matrix_close(&gpu_distances, &expected_distances, 1e-5);

        let gpu_filtration = gpu
            .build_rips_filtration(&gpu_distances, points.len(), 1.01, 2)
            .await
            .unwrap();
        assert_eq!(gpu_filtration, filtration);

        assert!(gpu
            .build_rips_filtration(&gpu_distances[..8], points.len(), 1.01, 1)
            .await
            .is_err());

        let gpu_critical_points = gpu
            .find_critical_points_2d(&values, width, height)
            .await
            .unwrap();
        assert!(gpu_critical_points
            .iter()
            .any(|cp| cp.position == (2, 2) && matches!(cp.critical_type, CriticalType::Maximum)));

        let mut complex = SimplicialComplex::new();
        complex.add_simplex(Simplex::new(vec![0]));
        complex.add_simplex(Simplex::new(vec![1]));
        complex.add_simplex(Simplex::new(vec![0, 1]));
        let betti = gpu.compute_betti_numbers(&complex).await.unwrap();
        assert_eq!(betti.first().copied(), Some(1));
    }
}
