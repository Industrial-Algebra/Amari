use amari_core::{Multivector, Vector};
use amari_gpu::{AdaptiveNetworkCompute, GpuGeometricNetwork, GpuNetworkError, GpuNetworkResult};
use amari_network::GeometricNetwork;

fn sample_network() -> GeometricNetwork<3, 0, 0> {
    let mut network = GeometricNetwork::<3, 0, 0>::new();
    let a = network.add_node(Vector::from_components(0.0, 0.0, 0.0).mv);
    let b = network.add_node(Vector::from_components(3.0, 4.0, 0.0).mv);
    let c = network.add_node(Vector::from_components(0.0, 0.0, 12.0).mv);
    network.add_undirected_edge(a, b, 1.0).unwrap();
    network.add_undirected_edge(b, c, 2.0).unwrap();
    network
}

fn assert_matrix_close(actual: &[Vec<f64>], expected: &[Vec<f64>], tol: f64) {
    assert_eq!(actual.len(), expected.len());
    for (actual_row, expected_row) in actual.iter().zip(expected.iter()) {
        assert_eq!(actual_row.len(), expected_row.len());
        for (&a, &e) in actual_row.iter().zip(expected_row.iter()) {
            assert!((a - e).abs() <= tol, "expected {e}, got {a}");
        }
    }
}

#[tokio::test]
async fn test_network_public_api_adaptive_cpu_baseline() {
    let explicit_result: GpuNetworkResult<()> = Ok(());
    assert!(explicit_result.is_ok());
    let explicit_error = GpuNetworkError::InvalidSize(0);
    assert!(explicit_error.to_string().contains('0'));

    assert!(!GpuGeometricNetwork::should_use_gpu(10));
    assert!(GpuGeometricNetwork::should_use_gpu(100));
    assert!(GpuGeometricNetwork::supports_gpu_distance::<3, 0, 0>());
    assert!(!GpuGeometricNetwork::supports_gpu_distance::<3, 1, 0>());

    let network = sample_network();
    let adaptive = AdaptiveNetworkCompute::new().await;
    let distances = adaptive
        .compute_all_pairwise_distances(&network)
        .await
        .unwrap();

    let expected = vec![
        vec![0.0, 5.0, 12.0],
        vec![5.0, 0.0, 13.0],
        vec![12.0, 13.0, 0.0],
    ];
    assert_matrix_close(&distances, &expected, 1e-10);

    // This API returns geometric distances, not graph shortest-path edge weights.
    let graph_shortest = network.compute_all_pairs_shortest_paths().unwrap();
    assert_eq!(graph_shortest[0][2], 3.0);
    assert_eq!(distances[0][2], 12.0);

    let centrality = adaptive
        .compute_geometric_centrality(&network)
        .await
        .unwrap();
    assert_eq!(centrality.len(), 3);
    assert!((centrality[0] - 2.0 / 17.0).abs() < 1e-10);
    assert!((centrality[1] - 2.0 / 18.0).abs() < 1e-10);
    assert!((centrality[2] - 2.0 / 25.0).abs() < 1e-10);
}

#[tokio::test]
async fn test_network_direct_gpu_paths_when_available() {
    let gpu = match GpuGeometricNetwork::new().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    let network = sample_network();
    let distances = gpu.compute_all_pairwise_distances(&network).await.unwrap();
    let expected = vec![
        vec![0.0, 5.0, 12.0],
        vec![5.0, 0.0, 13.0],
        vec![12.0, 13.0, 0.0],
    ];
    assert_matrix_close(&distances, &expected, 1e-5);

    let centrality = gpu.compute_geometric_centrality(&network).await.unwrap();
    assert_eq!(centrality.len(), 3);
    assert!((centrality[0] - 2.0 / 17.0).abs() < 1e-5);

    let communities = gpu.geometric_clustering(&network, 2, 8).await.unwrap();
    assert!(!communities.is_empty());
    assert!(communities.iter().all(|c| c.cohesion_score > 0.0));

    assert!(matches!(
        gpu.geometric_clustering(&network, 0, 8).await,
        Err(GpuNetworkError::InvalidSize(0))
    ));
    assert!(matches!(
        gpu.geometric_clustering(&network, 2, 0).await,
        Err(GpuNetworkError::InvalidSize(0))
    ));
}

#[tokio::test]
async fn test_network_direct_gpu_rejects_unsupported_embeddings() {
    let gpu = match GpuGeometricNetwork::new().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    let mut unsupported_signature = GeometricNetwork::<2, 1, 0>::new();
    unsupported_signature.add_node(Vector::from_components(0.0, 0.0, 0.0).mv);
    unsupported_signature.add_node(Vector::from_components(1.0, 0.0, 0.0).mv);
    assert!(matches!(
        gpu.compute_all_pairwise_distances(&unsupported_signature)
            .await,
        Err(GpuNetworkError::UnsupportedEmbedding(_))
    ));

    let mut non_vector = GeometricNetwork::<3, 0, 0>::new();
    let mut mv = Multivector::<3, 0, 0>::zero();
    mv.set(0, 1.0);
    non_vector.add_node(mv);
    non_vector.add_node(Vector::from_components(1.0, 0.0, 0.0).mv);
    assert!(matches!(
        gpu.compute_all_pairwise_distances(&non_vector).await,
        Err(GpuNetworkError::UnsupportedEmbedding(_))
    ));
}
