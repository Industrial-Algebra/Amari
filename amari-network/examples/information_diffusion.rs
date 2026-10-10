//! Information diffusion simulation using geometric products
//!
//! This example demonstrates how information spreads through a network
//! using geometric algebra. The diffusion process uses geometric products
//! between node positions to determine transmission strength, creating
//! a physics-inspired model of information propagation.

use amari_core::Vector;
use amari_network::{GeometricNetwork, NodeMetadata};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("📡 Amari Geometric Network Analysis - Information Diffusion");
    println!("=========================================================\n");

    // Create a 2D network representing a social/communication network
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    println!("🏗️  Building a social communication network...");

    // Create nodes representing individuals with different "influence orientations"
    // in the 2D geometric space
    let individuals = vec![
        ("Alice", 0.0, 0.0, "news_source", 1.0), // Central news source
        ("Bob", 1.0, 0.5, "early_adopter", 0.9), // Early adopter
        ("Carol", -0.5, 1.0, "skeptic", 0.3),    // Skeptical individual
        ("David", 1.5, -0.5, "influencer", 0.8), // Social influencer
        ("Eve", 0.8, 1.2, "follower", 0.6),      // Follower
        ("Frank", -1.0, -0.5, "isolated", 0.2),  // Somewhat isolated
        ("Grace", 2.0, 1.0, "enthusiast", 0.9),  // Enthusiastic spreader
        ("Henry", -0.2, -1.0, "conservative", 0.4), // Conservative
        ("Iris", 1.8, 0.2, "connector", 0.7),    // Network connector
        ("Jack", 0.3, -1.5, "peripheral", 0.5),  // Peripheral member
    ];

    let mut node_indices = Vec::new();
    for (name, x, y, personality, receptivity) in individuals {
        let node = network.add_node_with_metadata(
            Vector::from_components(x, y, 0.0).mv,
            NodeMetadata::with_label(name)
                .with_property("receptivity", receptivity)
                .with_property("x_position", x)
                .with_property("y_position", y),
        );
        node_indices.push((node, name, personality));
    }

    println!(
        "✅ Created {} individuals in the network",
        network.num_nodes()
    );

    // Create connections based on social relationships
    println!("\n🤝 Establishing social connections...");

    // Alice (news source) connects to early adopters and influencers
    network.add_edge(0, 1, 0.8)?; // Alice -> Bob (early adopter)
    network.add_edge(0, 3, 0.7)?; // Alice -> David (influencer)
    network.add_edge(0, 8, 0.6)?; // Alice -> Iris (connector)

    // Bob connects to multiple people (early adopter behavior)
    network.add_edge(1, 4, 0.9)?; // Bob -> Eve
    network.add_edge(1, 6, 0.8)?; // Bob -> Grace
    network.add_edge(1, 8, 0.7)?; // Bob -> Iris

    // David (influencer) has strong connections
    network.add_edge(3, 6, 0.9)?; // David -> Grace
    network.add_edge(3, 8, 0.8)?; // David -> Iris
    network.add_edge(3, 9, 0.6)?; // David -> Jack

    // Create some bidirectional relationships
    network.add_undirected_edge(4, 6, 0.7)?; // Eve <-> Grace
    network.add_undirected_edge(2, 7, 0.5)?; // Carol <-> Henry (skeptics)
    network.add_undirected_edge(8, 9, 0.6)?; // Iris <-> Jack

    // Some weaker connections
    network.add_edge(5, 7, 0.4)?; // Frank -> Henry
    network.add_edge(7, 9, 0.5)?; // Henry -> Jack
    network.add_edge(2, 4, 0.3)?; // Carol -> Eve (skeptic to follower)

    println!("✅ Created {} connections", network.num_edges());

    // Display network structure
    println!("\n👥 Network structure:");
    for (node_idx, name, personality) in &node_indices {
        let _neighbors = network.neighbors(*node_idx);
        let degree = network.degree(*node_idx);

        if let Some(metadata) = network.get_metadata(*node_idx) {
            let receptivity = metadata.properties.get("receptivity").unwrap_or(&0.0);
            println!(
                "  • {} ({}): {} connections, receptivity: {:.1}",
                name, personality, degree, receptivity
            );
        }
    }

    // Simulate information diffusion starting from Alice (news source)
    println!("\n📢 Simulating information diffusion from Alice...");

    let diffusion_analysis = network.simulate_diffusion(
        &[0], // Start from Alice (node 0)
        15,   // Maximum 15 time steps
        0.4,  // 40% diffusion rate per step
    )?;

    println!("✅ Diffusion simulation completed");
    println!(
        "  • Converged in {} steps",
        diffusion_analysis.convergence_time
    );
    println!(
        "  • Total coverage points: {}",
        diffusion_analysis.coverage.len()
    );

    // Analyze diffusion progression
    println!("\n📈 Diffusion progression over time:");
    for (step, &coverage) in diffusion_analysis.coverage.iter().enumerate() {
        let coverage_percent = (coverage as f64 / network.num_nodes() as f64) * 100.0;
        println!(
            "  Step {}: {} individuals reached ({:.1}%)",
            step, coverage, coverage_percent
        );
    }

    // Analyze individual influence scores
    println!("\n🌟 Individual influence analysis:");
    let mut influence_data: Vec<(f64, &str, &str)> = diffusion_analysis
        .influence_scores
        .iter()
        .enumerate()
        .map(|(i, &score)| {
            let (_, name, personality) = &node_indices[i];
            (score, *name, *personality)
        })
        .collect();

    // Sort by influence score (descending)
    influence_data.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    for (i, (influence, name, personality)) in influence_data.iter().enumerate() {
        if *influence > 0.001 {
            // Only show significant influencers
            println!(
                "  {}. {} ({}): influence score {:.4}",
                i + 1,
                name,
                personality,
                influence
            );
        }
    }

    // Analyze geometric similarity effects
    println!("\n🔬 Geometric similarity analysis:");
    println!("Computing similarity between key pairs using geometric products...");

    let alice_idx = 0;
    let key_pairs = vec![
        (alice_idx, 1, "Alice", "Bob"),
        (alice_idx, 2, "Alice", "Carol"),
        (alice_idx, 3, "Alice", "David"),
        (1, 6, "Bob", "Grace"),
        (2, 7, "Carol", "Henry"),
    ];

    for (node1, node2, name1, name2) in key_pairs {
        // Get positions
        let pos1 = network.get_node(node1).unwrap();
        let pos2 = network.get_node(node2).unwrap();

        // Compute geometric similarity (using the private method indirectly)
        let product = pos1.geometric_product(pos2);
        let scalar_part = product.scalar_part();
        let norm1 = pos1.coefficient_norm();
        let norm2 = pos2.coefficient_norm();

        let similarity = if norm1 > 0.0 && norm2 > 0.0 {
            (scalar_part / (norm1 * norm2)).abs()
        } else {
            0.0
        };

        println!(
            "  • {} ↔ {}: geometric similarity {:.4}",
            name1, name2, similarity
        );
    }

    // Analyze network properties that affect diffusion
    println!("\n📊 Network diffusion properties:");

    let geometric_centrality = network.compute_geometric_centrality()?;
    let betweenness_centrality = network.compute_betweenness_centrality()?;

    println!("Correlation between centrality and influence:");
    for (i, &influence) in diffusion_analysis.influence_scores.iter().enumerate() {
        if influence > 0.001 {
            let geo_centrality = geometric_centrality[i];
            let between_centrality = betweenness_centrality[i];
            let (_, name, _) = &node_indices[i];

            println!(
                "  • {}: influence {:.4}, geo-centrality {:.4}, betweenness {:.4}",
                name, influence, geo_centrality, between_centrality
            );
        }
    }

    // Simulate different diffusion scenarios
    println!("\n🧪 Testing different diffusion scenarios:");

    // Scenario 1: High diffusion rate
    let high_rate_analysis = network.simulate_diffusion(&[0], 10, 0.8)?;
    println!(
        "  High rate (80%): converged in {} steps, max coverage {}",
        high_rate_analysis.convergence_time,
        high_rate_analysis.coverage.iter().max().unwrap_or(&0)
    );

    // Scenario 2: Low diffusion rate
    let low_rate_analysis = network.simulate_diffusion(&[0], 20, 0.2)?;
    println!(
        "  Low rate (20%): converged in {} steps, max coverage {}",
        low_rate_analysis.convergence_time,
        low_rate_analysis.coverage.iter().max().unwrap_or(&0)
    );

    // Scenario 3: Multiple sources
    let multi_source_analysis = network.simulate_diffusion(&[0, 3, 6], 10, 0.5)?;
    println!(
        "  Multiple sources: converged in {} steps, max coverage {}",
        multi_source_analysis.convergence_time,
        multi_source_analysis.coverage.iter().max().unwrap_or(&0)
    );

    println!("\n🎉 Information diffusion analysis complete!");
    println!("This example demonstrated:");
    println!("  ✓ Creating a realistic social network with personality traits");
    println!("  ✓ Simulating information spread using geometric products");
    println!("  ✓ Analyzing influence patterns and convergence");
    println!("  ✓ Correlating geometric similarity with transmission strength");
    println!("  ✓ Comparing different diffusion scenarios");
    println!("  ✓ Understanding the role of network topology in information flow");

    Ok(())
}
