//! Integration tests for amari-network crate
//!
//! Tests the full functionality of geometric network analysis including
//! construction, geometric operations, path finding, community detection,
//! and information diffusion.

use amari_core::{Multivector, Vector};
use amari_network::{GeometricNetwork, NetworkError, NodeMetadata};
use approx::assert_relative_eq;

// Helper function to create multivectors from components
fn mv2(x: f64, y: f64) -> Multivector<2, 0, 0> {
    Vector::from_components(x, y, 0.0).mv
}

fn mv3(x: f64, y: f64, z: f64) -> Multivector<3, 0, 0> {
    Vector::from_components(x, y, z).mv
}

#[test]
fn test_basic_network_construction() {
    let mut network = GeometricNetwork::<3, 0, 0>::new();

    // Add nodes in 3D Euclidean space
    let node1 = network.add_node(mv3(1.0, 0.0, 0.0));
    let node2 = network.add_node(mv3(0.0, 1.0, 0.0));
    let node3 = network.add_node(mv3(0.0, 0.0, 1.0));

    assert_eq!(network.num_nodes(), 3);
    assert_eq!(node1, 0);
    assert_eq!(node2, 1);
    assert_eq!(node3, 2);

    // Add edges
    network.add_edge(node1, node2, 1.0).unwrap();
    network.add_edge(node2, node3, 1.5).unwrap();
    network.add_undirected_edge(node1, node3, 2.0).unwrap();

    assert_eq!(network.num_edges(), 4); // 2 directed + 1 undirected (2 directed)
}

#[test]
fn test_node_metadata() {
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    let metadata = NodeMetadata::with_label("Test Node")
        .with_property("importance", 0.8)
        .with_property("category", 1.0);

    let node = network.add_node_with_metadata(mv2(1.0, 1.0), metadata);

    let retrieved_metadata = network.get_metadata(node).unwrap();
    assert_eq!(retrieved_metadata.label, Some("Test Node".to_string()));
    assert_eq!(retrieved_metadata.properties.get("importance"), Some(&0.8));
    assert_eq!(retrieved_metadata.properties.get("category"), Some(&1.0));
}

#[test]
fn test_geometric_distance() {
    let mut network = GeometricNetwork::<3, 0, 0>::new();

    let node1 = network.add_node(mv3(0.0, 0.0, 0.0));
    let node2 = network.add_node(mv3(3.0, 4.0, 0.0));

    let distance = network.geometric_distance(node1, node2).unwrap();
    assert_relative_eq!(distance, 5.0, epsilon = 1e-10); // 3-4-5 triangle
}

#[test]
fn test_geometric_centrality() {
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    // Create a star topology in geometric space
    let center = network.add_node(mv2(0.0, 0.0));
    let node1 = network.add_node(mv2(1.0, 0.0));
    let node2 = network.add_node(mv2(0.0, 1.0));
    let node3 = network.add_node(mv2(-1.0, 0.0));
    let node4 = network.add_node(mv2(0.0, -1.0));

    let centrality = network.compute_geometric_centrality().unwrap();

    // Center node should have highest centrality (equidistant from all others)
    assert!(centrality[center] > centrality[node1]);
    assert!(centrality[center] > centrality[node2]);
    assert!(centrality[center] > centrality[node3]);
    assert!(centrality[center] > centrality[node4]);
}

#[test]
fn test_shortest_path() {
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    let node1 = network.add_node(mv2(0.0, 0.0));
    let node2 = network.add_node(mv2(1.0, 0.0));
    let node3 = network.add_node(mv2(2.0, 0.0));

    // Create a linear chain
    network.add_edge(node1, node2, 1.0).unwrap();
    network.add_edge(node2, node3, 1.0).unwrap();

    let result = network.shortest_path(node1, node3).unwrap();
    assert!(result.is_some());

    let (path, distance) = result.unwrap();
    assert_eq!(path, vec![node1, node2, node3]);
    assert_relative_eq!(distance, 2.0, epsilon = 1e-10);
}

#[test]
fn test_no_path_exists() {
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    let node1 = network.add_node(mv2(0.0, 0.0));
    let node2 = network.add_node(mv2(1.0, 0.0));
    let node3 = network.add_node(mv2(2.0, 0.0));

    // Only connect node1 to node2, leaving node3 isolated
    network.add_edge(node1, node2, 1.0).unwrap();

    let result = network.shortest_path(node1, node3).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_geometric_path_finding() {
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    let node1 = network.add_node(mv2(0.0, 0.0));
    let node2 = network.add_node(mv2(1.0, 1.0));
    let node3 = network.add_node(mv2(2.0, 0.0));

    // Create connections
    network.add_edge(node1, node2, 1.0).unwrap();
    network.add_edge(node2, node3, 1.0).unwrap();

    let result = network.shortest_geometric_path(node1, node3).unwrap();
    assert!(result.is_some());

    let (path, _distance) = result.unwrap();
    assert_eq!(path, vec![node1, node2, node3]);
}

#[test]
fn test_community_detection() {
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    // Create two well-separated clusters
    // Cluster 1: nodes around (0, 0)
    let _c1n1 = network.add_node(mv2(0.0, 0.0));
    let _c1n2 = network.add_node(mv2(0.1, 0.1));
    let _c1n3 = network.add_node(mv2(-0.1, 0.1));

    // Cluster 2: nodes around (5, 5)
    let _c2n1 = network.add_node(mv2(5.0, 5.0));
    let _c2n2 = network.add_node(mv2(5.1, 5.1));
    let _c2n3 = network.add_node(mv2(4.9, 5.1));

    let communities = network.find_communities(2).unwrap();
    assert_eq!(communities.len(), 2);

    // Check that each community contains nodes from the same cluster
    for community in &communities {
        assert!(!community.nodes.is_empty());
        assert!(community.cohesion_score > 0.0);

        // Verify nodes in same community are close together
        let first_node = community.nodes[0];
        let first_pos = network.get_node(first_node).unwrap();

        for &node_idx in &community.nodes[1..] {
            let node_pos = network.get_node(node_idx).unwrap();
            let distance = (first_pos.clone() - node_pos.clone()).coefficient_norm();
            assert!(distance < 1.0); // Nodes in same community should be close
        }
    }
}

#[test]
fn test_information_diffusion() {
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    // Create a simple linear network
    let node1 = network.add_node(mv2(0.0, 0.0));
    let node2 = network.add_node(mv2(1.0, 0.0));
    let node3 = network.add_node(mv2(2.0, 0.0));

    network.add_edge(node1, node2, 1.0).unwrap();
    network.add_edge(node2, node3, 1.0).unwrap();

    let analysis = network
        .simulate_diffusion(
            &[node1], // Start diffusion from node1
            10,       // Max 10 steps
            0.5,      // 50% diffusion rate
        )
        .unwrap();

    assert!(!analysis.coverage.is_empty());
    assert_eq!(analysis.influence_scores.len(), 3);
    assert!(analysis.convergence_time <= 10);

    // Node1 should have some influence (it's the source)
    assert!(analysis.influence_scores[node1] >= 0.0);
}

#[test]
fn test_betweenness_centrality() {
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    // Create a path network: 0 -> 1 -> 2
    let node1 = network.add_node(mv2(0.0, 0.0));
    let node2 = network.add_node(mv2(1.0, 0.0));
    let node3 = network.add_node(mv2(2.0, 0.0));

    network.add_edge(node1, node2, 1.0).unwrap();
    network.add_edge(node2, node3, 1.0).unwrap();

    let betweenness = network.compute_betweenness_centrality().unwrap();

    // Middle node should have highest betweenness centrality
    assert!(betweenness[node2] >= betweenness[node1]);
    assert!(betweenness[node2] >= betweenness[node3]);
}

// TODO: Fix hanging issue in tropical shortest path
#[test]
#[ignore]
fn test_tropical_network_conversion() {
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    let node1 = network.add_node(mv2(0.0, 0.0));
    let node2 = network.add_node(mv2(1.0, 0.0));
    let node3 = network.add_node(mv2(2.0, 0.0));

    network.add_edge(node1, node2, 1.0).unwrap();
    network.add_edge(node2, node3, 2.0).unwrap();

    let tropical_network = network.to_tropical_network().unwrap();
    assert_eq!(tropical_network.size(), 3);

    // Test tropical shortest path
    let result = tropical_network
        .shortest_path_tropical(node1, node3)
        .unwrap();
    assert!(result.is_some());
    let (_path, distance) = result.unwrap();
    assert_relative_eq!(distance, 3.0, epsilon = 1e-10); // 1.0 + 2.0
}

#[test]
fn test_error_handling() {
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    // Test accessing non-existent nodes
    assert!(matches!(
        network.geometric_distance(0, 1),
        Err(NetworkError::NodeIndexOutOfBounds(0))
    ));

    assert!(matches!(
        network.shortest_path(0, 1),
        Err(NetworkError::NodeIndexOutOfBounds(0))
    ));

    // Add a node to make network non-empty for community detection test
    network.add_node(mv2(0.0, 0.0));

    // Test invalid community detection parameters
    assert!(matches!(
        network.find_communities(0),
        Err(NetworkError::InvalidParameter { .. })
    ));
}

#[test]
fn test_spectral_clustering() {
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    // Create a simple network with clear structure
    let node1 = network.add_node(mv2(0.0, 0.0));
    let node2 = network.add_node(mv2(1.0, 0.0));
    let node3 = network.add_node(mv2(2.0, 0.0));
    let node4 = network.add_node(mv2(3.0, 0.0));

    // Create edges to form two clusters: (0,1) and (2,3)
    network.add_edge(node1, node2, 1.0).unwrap();
    network.add_edge(node2, node1, 1.0).unwrap();
    network.add_edge(node3, node4, 1.0).unwrap();
    network.add_edge(node4, node3, 1.0).unwrap();
    // Weak connection between clusters
    network.add_edge(node2, node3, 0.1).unwrap();

    let clusters = network.spectral_clustering(2).unwrap();
    assert!(clusters.len() <= 2);
    assert!(!clusters.is_empty());

    // Verify that nodes are assigned to clusters
    let total_nodes: usize = clusters.iter().map(|c| c.len()).sum();
    assert!(total_nodes <= 4);
}
