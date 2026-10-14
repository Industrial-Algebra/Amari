//! Tests for phantom types and formal verification features
//!
//! These tests verify that the phantom type system correctly enforces
//! compile-time constraints and that the verified contracts work as expected.

use amari_core::Vector;
use amari_network::GeometricNetwork;
use approx::assert_relative_eq;

// Only compile these tests when formal verification is enabled
#[cfg(feature = "formal-verification")]
mod verified_tests {
    use super::*;
    use amari_network::{
        GeometricProperties, GraphTheoreticProperties, TropicalProperties,
        VerifiedContractGeometricNetwork, VerifiedGeometricNetwork,
    };

    #[test]
    fn test_verified_network_creation() {
        // Test that verified networks can be created with different signatures
        let _euclidean: VerifiedGeometricNetwork<3, 0, 0> = VerifiedGeometricNetwork::new();
        let _minkowski: VerifiedGeometricNetwork<1, 3, 0> = VerifiedGeometricNetwork::new();
        let _projective: VerifiedGeometricNetwork<2, 0, 1> = VerifiedGeometricNetwork::new();

        // These should all be empty initially
        assert_eq!(_euclidean.num_nodes(), 0);
        assert_eq!(_minkowski.num_nodes(), 0);
        assert_eq!(_projective.num_nodes(), 0);
    }

    #[test]
    fn test_verified_contract_network() {
        let mut network: VerifiedContractGeometricNetwork<3, 0, 0> =
            VerifiedContractGeometricNetwork::new();

        // Test basic contract properties
        assert!(network.is_empty());
        assert_eq!(network.num_nodes(), 0);
        assert_eq!(network.num_edges(), 0);

        // Add some nodes
        let node1 = network.add_node(Vector::from_components(1.0, 0.0, 0.0).into_multivector());
        let node2 = network.add_node(Vector::from_components(0.0, 1.0, 0.0).into_multivector());

        assert_eq!(network.num_nodes(), 2);
        assert!(!network.is_empty());

        // Add an edge
        network
            .add_edge(node1, node2, 1.0)
            .expect("Should be able to add edge");
        assert_eq!(network.num_edges(), 1);
        assert!(network.has_edge(node1, node2));
    }

    #[test]
    fn test_graph_theoretic_properties() {
        let mut network: VerifiedContractGeometricNetwork<3, 0, 0> =
            VerifiedContractGeometricNetwork::new();

        // Add nodes and edges
        let node1 = network.add_node(Vector::from_components(1.0, 0.0, 0.0).into_multivector());
        let node2 = network.add_node(Vector::from_components(0.0, 1.0, 0.0).into_multivector());
        let node3 = network.add_node(Vector::from_components(0.0, 0.0, 1.0).into_multivector());

        network.add_edge(node1, node2, 1.0).unwrap();
        network.add_edge(node2, node3, 1.0).unwrap();

        // Test graph invariants
        assert!(network.graph_invariants());
        assert!(network.connectivity_invariants());
        assert!(network.degree_sequence_invariants());
    }

    #[test]
    fn test_geometric_properties() {
        let mut network: VerifiedContractGeometricNetwork<3, 0, 0> =
            VerifiedContractGeometricNetwork::new();

        let node1 = network.add_node(Vector::from_components(0.0, 0.0, 0.0).into_multivector());
        let node2 = network.add_node(Vector::from_components(1.0, 0.0, 0.0).into_multivector());
        let node3 = network.add_node(Vector::from_components(0.0, 1.0, 0.0).into_multivector());

        // Test metric properties
        let d12 = network.geometric_distance_verified(node1, node2);
        let d13 = network.geometric_distance_verified(node1, node3);
        let d23 = network.geometric_distance_verified(node2, node3);

        // Non-negativity
        assert!(d12 >= 0.0);
        assert!(d13 >= 0.0);
        assert!(d23 >= 0.0);

        // Identity: distance from node to itself should be 0
        assert_relative_eq!(
            network.geometric_distance_verified(node1, node1),
            0.0,
            epsilon = 1e-10
        );

        // Test signature consistency
        assert!(network.signature_consistency());
    }

    #[test]
    fn test_tropical_properties() {
        let mut network: VerifiedContractGeometricNetwork<3, 0, 0> =
            VerifiedContractGeometricNetwork::new();

        let node1 = network.add_node(Vector::from_components(0.0, 0.0, 0.0).into_multivector());
        let node2 = network.add_node(Vector::from_components(1.0, 0.0, 0.0).into_multivector());

        network.add_edge(node1, node2, 1.0).unwrap();

        // Test tropical preservation properties
        assert_eq!(
            network.tropical_node_count(),
            network.geometric_node_count()
        );
        assert_eq!(
            network.has_edge_geometric(node1, node2),
            network.has_edge_tropical(node1, node2)
        );
        assert!(network.tropical_shortest_path_correctness());
    }

    #[test]
    fn test_conversion_between_types() {
        // Create a regular GeometricNetwork
        let mut regular_network = GeometricNetwork::<3, 0, 0>::new();
        let node1 =
            regular_network.add_node(Vector::from_components(1.0, 0.0, 0.0).into_multivector());
        let node2 =
            regular_network.add_node(Vector::from_components(0.0, 1.0, 0.0).into_multivector());
        regular_network.add_edge(node1, node2, 1.0).unwrap();

        // Convert to verified network
        let verified_network: VerifiedGeometricNetwork<3, 0, 0> = regular_network.clone().into();
        assert_eq!(verified_network.num_nodes(), 2);
        assert_eq!(verified_network.num_edges(), 1);

        // Convert to contract network
        let contract_network: VerifiedContractGeometricNetwork<3, 0, 0> = regular_network.into();
        assert_eq!(contract_network.num_nodes(), 2);
        assert_eq!(contract_network.num_edges(), 1);
    }
}

// Fallback tests when formal verification is not enabled
#[cfg(not(feature = "formal-verification"))]
mod regular_tests {
    use super::*;

    #[test]
    fn test_regular_network_without_verification() {
        let mut network = GeometricNetwork::<3, 0, 0>::new();

        let node1 = network.add_node(Vector::from_components(1.0, 0.0, 0.0).into_multivector());
        let node2 = network.add_node(Vector::from_components(0.0, 1.0, 0.0).into_multivector());

        network
            .add_edge(node1, node2, 1.0)
            .expect("Should be able to add edge");

        let distance = network.geometric_distance(node1, node2).unwrap();
        assert_relative_eq!(distance, std::f64::consts::SQRT_2, epsilon = 1e-10);
    }
}

#[test]
fn test_type_safety_compilation() {
    // These should compile fine - same signatures
    let mut network1 = GeometricNetwork::<3, 0, 0>::new();
    let mut network2 = GeometricNetwork::<3, 0, 0>::new();

    let node1 = network1.add_node(Vector::from_components(1.0, 0.0, 0.0).into_multivector());
    let _node2 = network2.add_node(Vector::from_components(0.0, 1.0, 0.0).into_multivector());

    // This is fine - within the same network
    network1.add_edge(node1, node1, 1.0).unwrap();

    // Note: We can't easily test compile-time failures in unit tests,
    // but the type system will prevent mixing incompatible signatures
}

#[test]
fn test_different_signatures() {
    // Test that networks with different signatures work independently
    let mut euclidean = GeometricNetwork::<3, 0, 0>::new();
    let mut minkowski = GeometricNetwork::<1, 3, 0>::new();

    let e_node = euclidean.add_node(Vector::from_components(1.0, 0.0, 0.0).into_multivector());
    let m_node = minkowski.add_node(Vector::from_components(1.0, 0.0, 0.0).into_multivector());

    // Each network maintains its own signature
    assert_eq!(euclidean.num_nodes(), 1);
    assert_eq!(minkowski.num_nodes(), 1);

    euclidean.add_edge(e_node, e_node, 1.0).unwrap();
    minkowski.add_edge(m_node, m_node, 1.0).unwrap();
}
