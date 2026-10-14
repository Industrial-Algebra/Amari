//! Basic network construction and analysis example
//!
//! This example demonstrates the fundamental operations of the amari-network crate:
//! - Creating a geometric network with nodes embedded in Clifford algebra space
//! - Adding nodes and edges
//! - Computing geometric distances and centrality measures
//! - Finding shortest paths

use amari_core::Vector;
use amari_network::{GeometricNetwork, NodeMetadata};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🕸️  Amari Geometric Network Analysis - Basic Example");
    println!("================================================\n");

    // Create a network in 3D Euclidean space (signature 3,0,0)
    let mut network = GeometricNetwork::<3, 0, 0>::new();

    println!("📊 Creating nodes in 3D geometric space...");

    // Add nodes at specific geometric positions with metadata
    let origin = network.add_node_with_metadata(
        Vector::from_components(0.0, 0.0, 0.0).into_multivector(),
        NodeMetadata::with_label("Origin").with_property("importance", 1.0),
    );

    let node_x = network.add_node_with_metadata(
        Vector::from_components(3.0, 0.0, 0.0).into_multivector(),
        NodeMetadata::with_label("X-Axis").with_property("importance", 0.8),
    );

    let node_y = network.add_node_with_metadata(
        Vector::from_components(0.0, 4.0, 0.0).into_multivector(),
        NodeMetadata::with_label("Y-Axis").with_property("importance", 0.8),
    );

    let node_z = network.add_node_with_metadata(
        Vector::from_components(0.0, 0.0, 5.0).into_multivector(),
        NodeMetadata::with_label("Z-Axis").with_property("importance", 0.7),
    );

    let center = network.add_node_with_metadata(
        Vector::from_components(1.0, 1.0, 1.0).into_multivector(),
        NodeMetadata::with_label("Center").with_property("importance", 0.9),
    );

    println!("✅ Created {} nodes", network.num_nodes());

    // Add edges to create connections
    println!("\n🔗 Adding edges between nodes...");

    // Connect origin to axis nodes
    network.add_edge(origin, node_x, 1.0)?;
    network.add_edge(origin, node_y, 1.0)?;
    network.add_edge(origin, node_z, 1.0)?;

    // Connect center to all other nodes
    network.add_edge(center, origin, 0.5)?;
    network.add_edge(center, node_x, 0.7)?;
    network.add_edge(center, node_y, 0.6)?;
    network.add_edge(center, node_z, 0.8)?;

    // Create some interconnections
    network.add_undirected_edge(node_x, node_y, 2.0)?;
    network.add_undirected_edge(node_y, node_z, 1.5)?;

    println!("✅ Created {} edges", network.num_edges());

    // Compute geometric distances
    println!("\n📏 Computing geometric distances:");

    let distance_xy = network.geometric_distance(node_x, node_y)?;
    let distance_oz = network.geometric_distance(origin, node_z)?;
    let distance_center_origin = network.geometric_distance(center, origin)?;

    println!("  • Distance from X-axis to Y-axis: {:.2}", distance_xy);
    println!("  • Distance from Origin to Z-axis: {:.2}", distance_oz);
    println!(
        "  • Distance from Center to Origin: {:.2}",
        distance_center_origin
    );

    // Compute centrality measures
    println!("\n🎯 Computing centrality measures:");

    let geometric_centrality = network.compute_geometric_centrality()?;
    let betweenness_centrality = network.compute_betweenness_centrality()?;

    println!("  Geometric Centrality:");
    for (i, centrality) in geometric_centrality.iter().enumerate() {
        if let Some(metadata) = network.get_metadata(i) {
            if let Some(label) = &metadata.label {
                println!("    {} ({}): {:.4}", i, label, centrality);
            }
        } else {
            println!("    {}: {:.4}", i, centrality);
        }
    }

    println!("  Betweenness Centrality:");
    for (i, centrality) in betweenness_centrality.iter().enumerate() {
        if let Some(metadata) = network.get_metadata(i) {
            if let Some(label) = &metadata.label {
                println!("    {} ({}): {:.4}", i, label, centrality);
            }
        } else {
            println!("    {}: {:.4}", i, centrality);
        }
    }

    // Find shortest paths
    println!("\n🛤️  Finding shortest paths:");

    if let Some((path, distance)) = network.shortest_path(origin, node_z)? {
        println!(
            "  • Path from Origin to Z-axis: {:?} (distance: {:.2})",
            path, distance
        );
    } else {
        println!("  • No path found from Origin to Z-axis");
    }

    if let Some((path, distance)) = network.shortest_geometric_path(origin, center)? {
        println!(
            "  • Geometric path from Origin to Center: {:?} (distance: {:.2})",
            path, distance
        );
    } else {
        println!("  • No geometric path found from Origin to Center");
    }

    // Convert to tropical network for efficient path operations
    println!("\n🌴 Converting to tropical network for advanced path analysis...");

    let tropical_network = network.to_tropical_network()?;
    println!(
        "✅ Tropical network created with {} nodes",
        tropical_network.size()
    );

    // Demonstrate neighbor analysis
    println!("\n👥 Analyzing node neighborhoods:");
    for i in 0..network.num_nodes() {
        let neighbors = network.neighbors(i);
        let degree = network.degree(i);

        if let Some(metadata) = network.get_metadata(i) {
            if let Some(label) = &metadata.label {
                println!(
                    "  • {} ({}): {} neighbors, degree {}",
                    i,
                    label,
                    neighbors.len(),
                    degree
                );
                if !neighbors.is_empty() {
                    println!("    Connected to: {:?}", neighbors);
                }
            }
        }
    }

    println!("\n🎉 Basic network analysis complete!");
    println!("This example demonstrated:");
    println!("  ✓ Creating nodes in 3D geometric space");
    println!("  ✓ Adding metadata to nodes");
    println!("  ✓ Computing geometric distances using Clifford algebra");
    println!("  ✓ Calculating centrality measures");
    println!("  ✓ Finding shortest paths");
    println!("  ✓ Converting to tropical representation");

    Ok(())
}
