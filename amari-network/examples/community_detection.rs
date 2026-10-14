//! Community detection using geometric clustering
//!
//! This example demonstrates how to detect communities in networks using
//! geometric clustering methods provided by the amari-network crate.
//! Communities are identified based on geometric proximity in the
//! multivector space and graph structure.

use amari_core::Vector;
use amari_network::{GeometricNetwork, NodeMetadata};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Amari Geometric Network Analysis - Community Detection");
    println!("=======================================================\n");

    // Create a 2D network for visualization simplicity
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    println!("🏘️  Creating a network with distinct communities...");

    // Create Community 1: Research Group (around origin)
    let research_nodes = vec![
        ("Alice", 0.0, 0.0),
        ("Bob", 0.2, 0.1),
        ("Carol", -0.1, 0.2),
        ("David", 0.1, -0.1),
        ("Eve", -0.2, -0.1),
    ];

    let mut community1_indices = Vec::new();
    for (name, x, y) in research_nodes {
        let node = network.add_node_with_metadata(
            Vector::from_components(x, y, 0.0).into_multivector(),
            NodeMetadata::with_label(format!("Research-{}", name))
                .with_property("group", 1.0)
                .with_property("activity", 0.8),
        );
        community1_indices.push(node);
    }

    // Create Community 2: Engineering Group (around point (3, 3))
    let engineering_nodes = vec![
        ("Frank", 3.0, 3.0),
        ("Grace", 3.1, 3.2),
        ("Henry", 2.9, 3.1),
        ("Iris", 3.2, 2.8),
        ("Jack", 2.8, 2.9),
    ];

    let mut community2_indices = Vec::new();
    for (name, x, y) in engineering_nodes {
        let node = network.add_node_with_metadata(
            Vector::from_components(x, y, 0.0).into_multivector(),
            NodeMetadata::with_label(format!("Engineering-{}", name))
                .with_property("group", 2.0)
                .with_property("activity", 0.9),
        );
        community2_indices.push(node);
    }

    // Create Community 3: Design Group (around point (-2, 4))
    let design_nodes = vec![
        ("Kate", -2.0, 4.0),
        ("Liam", -1.8, 4.1),
        ("Mia", -2.2, 3.9),
        ("Noah", -1.9, 3.8),
    ];

    let mut community3_indices = Vec::new();
    for (name, x, y) in design_nodes {
        let node = network.add_node_with_metadata(
            Vector::from_components(x, y, 0.0).into_multivector(),
            NodeMetadata::with_label(format!("Design-{}", name))
                .with_property("group", 3.0)
                .with_property("activity", 0.7),
        );
        community3_indices.push(node);
    }

    println!(
        "✅ Created {} nodes in 3 expected communities",
        network.num_nodes()
    );

    // Add intra-community edges (strong connections within groups)
    println!("\n🔗 Adding intra-community connections...");

    // Research group connections
    for i in 0..community1_indices.len() {
        for j in (i + 1)..community1_indices.len() {
            network.add_undirected_edge(community1_indices[i], community1_indices[j], 1.0)?;
        }
    }

    // Engineering group connections
    for i in 0..community2_indices.len() {
        for j in (i + 1)..community2_indices.len() {
            network.add_undirected_edge(community2_indices[i], community2_indices[j], 1.2)?;
        }
    }

    // Design group connections
    for i in 0..community3_indices.len() {
        for j in (i + 1)..community3_indices.len() {
            network.add_undirected_edge(community3_indices[i], community3_indices[j], 0.8)?;
        }
    }

    // Add some inter-community edges (weaker connections between groups)
    println!("🌉 Adding inter-community bridges...");
    network.add_edge(community1_indices[0], community2_indices[0], 0.3)?; // Alice -> Frank
    network.add_edge(community2_indices[1], community3_indices[0], 0.2)?; // Grace -> Kate
    network.add_edge(community3_indices[1], community1_indices[1], 0.4)?; // Liam -> Bob

    println!("✅ Created {} edges total", network.num_edges());

    // Perform geometric community detection
    println!("\n🎯 Detecting communities using geometric clustering...");

    let communities = network.find_communities(3)?;
    println!("✅ Found {} communities", communities.len());

    println!("\nCommunity Analysis:");
    for (i, community) in communities.iter().enumerate() {
        println!(
            "  Community {}: {} members, cohesion score: {:.4}",
            i + 1,
            community.nodes.len(),
            community.cohesion_score
        );

        print!("    Members: ");
        for &node_idx in &community.nodes {
            if let Some(metadata) = network.get_metadata(node_idx) {
                if let Some(label) = &metadata.label {
                    print!("{} ", label);
                }
            }
        }
        println!();

        // Show geometric centroid
        println!(
            "    Geometric centroid: [{:.2}, {:.2}]",
            community.geometric_centroid.get(1), // e1 component
            community.geometric_centroid.get(2)
        ); // e2 component
    }

    // Perform spectral clustering for comparison
    println!("\n🌟 Comparing with spectral clustering...");

    let spectral_clusters = network.spectral_clustering(3)?;
    println!(
        "✅ Spectral clustering found {} clusters",
        spectral_clusters.len()
    );

    for (i, cluster) in spectral_clusters.iter().enumerate() {
        println!("  Spectral Cluster {}: {} members", i + 1, cluster.len());
        print!("    Members: ");
        for &node_idx in cluster {
            if let Some(metadata) = network.get_metadata(node_idx) {
                if let Some(label) = &metadata.label {
                    print!("{} ", label);
                }
            }
        }
        println!();
    }

    // Analyze community quality
    println!("\n📊 Community Quality Analysis:");

    // Calculate modularity-like measure using geometric distances
    let mut intra_community_distances = Vec::new();
    let mut inter_community_distances = Vec::new();

    for (comm_idx, community) in communities.iter().enumerate() {
        // Intra-community distances
        for i in 0..community.nodes.len() {
            for j in (i + 1)..community.nodes.len() {
                let distance =
                    network.geometric_distance(community.nodes[i], community.nodes[j])?;
                intra_community_distances.push(distance);
            }
        }

        // Inter-community distances (to other communities)
        for other_comm in communities.iter().skip(comm_idx + 1) {
            for &node1 in &community.nodes {
                for &node2 in &other_comm.nodes {
                    let distance = network.geometric_distance(node1, node2)?;
                    inter_community_distances.push(distance);
                }
            }
        }
    }

    if !intra_community_distances.is_empty() && !inter_community_distances.is_empty() {
        let avg_intra_distance: f64 =
            intra_community_distances.iter().sum::<f64>() / intra_community_distances.len() as f64;
        let avg_inter_distance: f64 =
            inter_community_distances.iter().sum::<f64>() / inter_community_distances.len() as f64;

        println!(
            "  • Average intra-community distance: {:.4}",
            avg_intra_distance
        );
        println!(
            "  • Average inter-community distance: {:.4}",
            avg_inter_distance
        );
        println!(
            "  • Separation ratio: {:.4}",
            avg_inter_distance / avg_intra_distance
        );

        if avg_inter_distance / avg_intra_distance > 2.0 {
            println!("  ✅ Well-separated communities detected!");
        } else {
            println!("  ⚠️  Communities may be overlapping");
        }
    }

    // Show expected vs detected communities
    println!("\n🎭 Ground Truth vs Detection:");
    println!("Expected communities:");
    println!("  • Research Group: Alice, Bob, Carol, David, Eve");
    println!("  • Engineering Group: Frank, Grace, Henry, Iris, Jack");
    println!("  • Design Group: Kate, Liam, Mia, Noah");

    println!("\n🎉 Community detection analysis complete!");
    println!("This example demonstrated:");
    println!("  ✓ Creating networks with known community structure");
    println!("  ✓ Geometric clustering based on multivector positions");
    println!("  ✓ Spectral clustering using graph Laplacian");
    println!("  ✓ Community quality assessment");
    println!("  ✓ Comparing different clustering methods");

    Ok(())
}
