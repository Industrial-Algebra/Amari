//! Advanced geometric analysis using Clifford algebra
//!
//! This example showcases the advanced geometric algebra capabilities
//! of the amari-network crate, including operations in different
//! Clifford algebra spaces and the mathematical foundations of
//! geometric network analysis.

use amari_core::Vector;
use amari_network::{GeometricNetwork, NodeMetadata};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧮 Amari Geometric Network Analysis - Advanced Geometric Analysis");
    println!("===============================================================\n");

    println!("🔬 Exploring different Clifford algebra signatures...");

    // Example 1: Euclidean space (3,0,0) - standard 3D space
    println!("\n📐 Example 1: Euclidean Space Cl(3,0,0)");
    let mut euclidean_network = GeometricNetwork::<3, 0, 0>::new();

    let euclidean_points = vec![
        ("Origin", 0.0, 0.0, 0.0),
        ("Unit-X", 1.0, 0.0, 0.0),
        ("Unit-Y", 0.0, 1.0, 0.0),
        ("Unit-Z", 0.0, 0.0, 1.0),
        ("Diagonal", 1.0, 1.0, 1.0),
    ];

    for (name, x, y, z) in euclidean_points {
        euclidean_network.add_node_with_metadata(
            Vector::from_components(x, y, z).into_multivector(),
            NodeMetadata::with_label(name)
                .with_property("x", x)
                .with_property("y", y)
                .with_property("z", z),
        );
    }

    // Add some edges
    euclidean_network.add_edge(0, 1, 1.0)?; // Origin -> Unit-X
    euclidean_network.add_edge(0, 2, 1.0)?; // Origin -> Unit-Y
    euclidean_network.add_edge(0, 3, 1.0)?; // Origin -> Unit-Z
    euclidean_network.add_edge(1, 4, 1.0)?; // Unit-X -> Diagonal
    euclidean_network.add_edge(2, 4, 1.0)?; // Unit-Y -> Diagonal
    euclidean_network.add_edge(3, 4, 1.0)?; // Unit-Z -> Diagonal

    println!(
        "✅ Created Euclidean network with {} nodes",
        euclidean_network.num_nodes()
    );

    // Compute geometric distances in Euclidean space
    println!("   Geometric distances:");
    let dist_origin_diagonal = euclidean_network.geometric_distance(0, 4)?;
    let dist_x_y = euclidean_network.geometric_distance(1, 2)?;
    let dist_x_z = euclidean_network.geometric_distance(1, 3)?;

    println!(
        "     • Origin to Diagonal: {:.4} (expected: √3 ≈ 1.732)",
        dist_origin_diagonal
    );
    println!(
        "     • Unit-X to Unit-Y: {:.4} (expected: √2 ≈ 1.414)",
        dist_x_y
    );
    println!(
        "     • Unit-X to Unit-Z: {:.4} (expected: √2 ≈ 1.414)",
        dist_x_z
    );

    // Example 2: Minkowski space (1,1,0) - spacetime-like signature
    println!("\n⚡ Example 2: Minkowski-like Space Cl(1,1,0)");
    let mut minkowski_network = GeometricNetwork::<1, 1, 0>::new();

    let spacetime_events = vec![
        ("Event-0", 0.0, 0.0, 0.0),   // Origin event
        ("Future", 1.0, 0.8, 0.0),    // Future-like separated
        ("Spacelike", 0.5, 1.2, 0.0), // Space-like separated
        ("Past", -0.8, -0.6, 0.0),    // Past-like separated
    ];

    for (name, t, x, _z) in spacetime_events {
        minkowski_network.add_node_with_metadata(
            Vector::from_components(t, x, 0.0).into_multivector(),
            NodeMetadata::with_label(name)
                .with_property("time", t)
                .with_property("space", x),
        );
    }

    // Connect events that could have causal relationships
    minkowski_network.add_edge(0, 1, 1.0)?; // Origin -> Future
    minkowski_network.add_edge(3, 0, 1.0)?; // Past -> Origin
    minkowski_network.add_edge(3, 1, 2.0)?; // Past -> Future

    println!(
        "✅ Created Minkowski-like network with {} nodes",
        minkowski_network.num_nodes()
    );

    // Note: In Minkowski space, distances can be complex due to the signature
    println!("   Geometric intervals (note: may include signature effects):");
    let interval_origin_future = minkowski_network.geometric_distance(0, 1)?;
    let interval_origin_spacelike = minkowski_network.geometric_distance(0, 2)?;

    println!(
        "     • Origin to Future event: {:.4}",
        interval_origin_future
    );
    println!(
        "     • Origin to Spacelike event: {:.4}",
        interval_origin_spacelike
    );

    // Example 3: Projective space (2,0,1) - with null dimensions
    println!("\n🌌 Example 3: Space with Null Signature Cl(2,0,1)");
    let mut projective_network = GeometricNetwork::<2, 0, 1>::new();

    let projective_points = vec![
        ("Point-A", 1.0, 0.0, 0.0),
        ("Point-B", 0.0, 1.0, 0.0),
        ("Point-C", 0.0, 0.0, 1.0), // Null direction
        ("Mixed", 1.0, 1.0, 0.5),
    ];

    for (name, x, y, n) in projective_points {
        projective_network.add_node_with_metadata(
            Vector::from_components(x, y, n).into_multivector(),
            NodeMetadata::with_label(name)
                .with_property("euclidean_x", x)
                .with_property("euclidean_y", y)
                .with_property("null_component", n),
        );
    }

    projective_network.add_edge(0, 1, 1.0)?;
    projective_network.add_edge(1, 2, 1.0)?;
    projective_network.add_edge(2, 3, 1.0)?;
    projective_network.add_edge(3, 0, 1.0)?;

    println!(
        "✅ Created projective-like network with {} nodes",
        projective_network.num_nodes()
    );

    // Advanced geometric analysis
    println!("\n🔬 Advanced geometric analysis across different spaces:");

    // Analyze centrality in different spaces
    let euclidean_centrality = euclidean_network.compute_geometric_centrality()?;
    let minkowski_centrality = minkowski_network.compute_geometric_centrality()?;
    let projective_centrality = projective_network.compute_geometric_centrality()?;

    println!("\n📊 Geometric centrality comparison:");
    println!("   Euclidean space (3,0,0):");
    for (i, &centrality) in euclidean_centrality.iter().enumerate() {
        if let Some(metadata) = euclidean_network.get_metadata(i) {
            if let Some(label) = &metadata.label {
                println!("     • {}: {:.4}", label, centrality);
            }
        }
    }

    println!("   Minkowski-like space (1,1,0):");
    for (i, &centrality) in minkowski_centrality.iter().enumerate() {
        if let Some(metadata) = minkowski_network.get_metadata(i) {
            if let Some(label) = &metadata.label {
                println!("     • {}: {:.4}", label, centrality);
            }
        }
    }

    println!("   Projective-like space (2,0,1):");
    for (i, &centrality) in projective_centrality.iter().enumerate() {
        if let Some(metadata) = projective_network.get_metadata(i) {
            if let Some(label) = &metadata.label {
                println!("     • {}: {:.4}", label, centrality);
            }
        }
    }

    // Demonstrate path finding in different geometries
    println!("\n🛤️  Path finding across geometric spaces:");

    // Euclidean shortest path
    if let Some((path, distance)) = euclidean_network.shortest_path(0, 4)? {
        println!(
            "   Euclidean path (Origin→Diagonal): {:?}, distance: {:.4}",
            path, distance
        );
    }

    // Minkowski-like shortest path
    if let Some((path, distance)) = minkowski_network.shortest_path(0, 1)? {
        println!(
            "   Minkowski path (Origin→Future): {:?}, distance: {:.4}",
            path, distance
        );
    }

    // Compare with geometric paths
    if let Some((geo_path, geo_distance)) = euclidean_network.shortest_geometric_path(0, 4)? {
        println!(
            "   Euclidean geometric path: {:?}, distance: {:.4}",
            geo_path, geo_distance
        );
    }

    // Demonstrate community detection in geometric space
    println!("\n🏘️  Community detection in Euclidean space:");

    // Add more nodes to create communities
    let community_points = [
        (2.0, 2.0, 0.0),
        (2.1, 2.1, 0.0),
        (1.9, 2.0, 0.0), // Cluster 1
        (-1.0, -1.0, 1.0),
        (-1.1, -0.9, 1.1),
        (-0.9, -1.1, 0.9), // Cluster 2
    ];

    for (i, (x, y, z)) in community_points.iter().enumerate() {
        euclidean_network.add_node_with_metadata(
            Vector::from_components(*x, *y, *z).into_multivector(),
            NodeMetadata::with_label(format!("Cluster-{}", i + 1))
                .with_property("cluster_id", if i < 3 { 1.0 } else { 2.0 }),
        );
    }

    // Connect within clusters
    for i in 0..3 {
        for j in (i + 1)..3 {
            euclidean_network.add_undirected_edge(5 + i, 5 + j, 0.5)?;
        }
    }
    for i in 3..6 {
        for j in (i + 1)..6 {
            euclidean_network.add_undirected_edge(5 + i, 5 + j, 0.5)?;
        }
    }

    let communities = euclidean_network.find_communities(3)?;
    println!(
        "   Found {} communities in extended Euclidean network:",
        communities.len()
    );

    for (i, community) in communities.iter().enumerate() {
        println!(
            "     Community {}: {} members, cohesion: {:.4}",
            i + 1,
            community.nodes.len(),
            community.cohesion_score
        );

        // Show geometric centroid
        let centroid = &community.geometric_centroid;
        println!(
            "       Centroid: [{:.2}, {:.2}, {:.2}]",
            centroid.get(1),
            centroid.get(2),
            centroid.get(4)
        ); // e1, e2, e3 components
    }

    // Mathematical foundation demonstration
    println!("\n🧮 Mathematical foundation insights:");

    println!("   Geometric algebra provides:");
    println!("     • Unified framework for different geometric spaces");
    println!("     • Natural distance metrics via multivector norms");
    println!("     • Rotations and reflections through geometric products");
    println!("     • Scale-invariant similarity measures");

    println!("   Tropical algebra enables:");
    println!("     • Efficient shortest path computation (O(n³) → matrix operations)");
    println!("     • Max-plus semiring structure for optimization");
    println!("     • Parallel computation opportunities");
    println!("     • Connection to convex geometry");

    println!("   Network analysis benefits:");
    println!("     • Geometric embedding reveals hidden structure");
    println!("     • Physics-inspired diffusion models");
    println!("     • Multi-scale community detection");
    println!("     • Robust centrality measures");

    // Performance comparison
    println!("\n⚡ Performance characteristics:");

    let start_time = std::time::Instant::now();
    let _all_pairs = euclidean_network.compute_all_pairs_shortest_paths()?;
    let standard_time = start_time.elapsed();

    let start_time = std::time::Instant::now();
    let tropical_net = euclidean_network.to_tropical_network()?;
    let _tropical_all_pairs = tropical_net.all_pairs_shortest_paths()?;
    let tropical_time = start_time.elapsed();

    println!("   All-pairs shortest paths:");
    println!("     • Standard Floyd-Warshall: {:?}", standard_time);
    println!("     • Tropical computation: {:?}", tropical_time);

    if tropical_time < standard_time {
        println!("     ✅ Tropical method is faster!");
    } else {
        println!("     ✅ Both methods efficient for this size");
    }

    println!("\n🎉 Advanced geometric analysis complete!");
    println!("This example demonstrated:");
    println!("  ✓ Networks in different Clifford algebra signatures");
    println!("  ✓ Geometric distance computation across spaces");
    println!("  ✓ Centrality analysis in various geometries");
    println!("  ✓ Community detection using geometric clustering");
    println!("  ✓ Mathematical foundations and performance characteristics");
    println!("  ✓ Tropical algebra optimization advantages");

    Ok(())
}
