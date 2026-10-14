//! Tropical algebra path finding example
//!
//! This example demonstrates efficient shortest path computation using
//! tropical (max-plus) algebra. Tropical arithmetic provides an elegant
//! mathematical framework for optimization problems, where addition
//! becomes max and multiplication becomes addition.

use amari_core::Vector;
use amari_network::GeometricNetwork;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌴 Amari Geometric Network Analysis - Tropical Path Finding");
    println!("========================================================\n");

    // Create a transportation network in 2D space
    let mut network = GeometricNetwork::<2, 0, 0>::new();

    println!("🚗 Building a transportation network...");

    // Create nodes representing cities/locations
    let cities = vec![
        ("New York", 0.0, 0.0),
        ("Philadelphia", 1.5, -0.8),
        ("Boston", 2.0, 3.0),
        ("Washington DC", 0.5, -2.0),
        ("Baltimore", 1.0, -1.5),
        ("Pittsburgh", -2.0, -1.0),
        ("Buffalo", -1.0, 2.5),
        ("Albany", 1.8, 1.5),
        ("Hartford", 2.5, 1.0),
        ("Providence", 3.0, 2.0),
    ];

    let mut city_indices = Vec::new();
    for (i, (name, x, y)) in cities.iter().enumerate() {
        let node = network.add_node(Vector::from_components(*x, *y, 0.0).into_multivector());
        city_indices.push((node, *name));
        println!("  🏙️  {}: City {} at ({:.1}, {:.1})", i, name, x, y);
    }

    println!("\n🛣️  Adding transportation routes with travel times...");

    // Add routes with travel times (in hours) as edge weights
    let routes = vec![
        (0, 1, 1.5), // NY -> Philadelphia (1.5h)
        (0, 3, 4.5), // NY -> Washington DC (4.5h)
        (0, 7, 2.5), // NY -> Albany (2.5h)
        (1, 3, 3.0), // Philadelphia -> Washington DC (3h)
        (1, 4, 2.0), // Philadelphia -> Baltimore (2h)
        (2, 7, 3.5), // Boston -> Albany (3.5h)
        (2, 8, 1.5), // Boston -> Hartford (1.5h)
        (2, 9, 1.0), // Boston -> Providence (1h)
        (3, 4, 0.5), // Washington DC -> Baltimore (0.5h)
        (3, 5, 5.0), // Washington DC -> Pittsburgh (5h)
        (4, 5, 4.5), // Baltimore -> Pittsburgh (4.5h)
        (5, 6, 3.5), // Pittsburgh -> Buffalo (3.5h)
        (6, 7, 2.0), // Buffalo -> Albany (2h)
        (7, 8, 1.8), // Albany -> Hartford (1.8h)
        (8, 9, 1.2), // Hartford -> Providence (1.2h)
        (0, 6, 6.0), // NY -> Buffalo (direct, 6h)
        (1, 8, 4.0), // Philadelphia -> Hartford (direct, 4h)
        (4, 7, 3.8), // Baltimore -> Albany (direct, 3.8h)
    ];

    for (from, to, time) in routes {
        network.add_edge(from, to, time)?;
        let from_name = city_indices[from].1;
        let to_name = city_indices[to].1;
        println!("    {} → {} ({:.1}h)", from_name, to_name, time);
    }

    // Also add some bidirectional routes
    let bidirectional_routes = vec![
        (0, 2, 4.0), // NY ↔ Boston (4h)
        (1, 5, 5.5), // Philadelphia ↔ Pittsburgh (5.5h)
        (6, 8, 4.2), // Buffalo ↔ Hartford (4.2h)
    ];

    for (city1, city2, time) in bidirectional_routes {
        network.add_undirected_edge(city1, city2, time)?;
        let name1 = city_indices[city1].1;
        let name2 = city_indices[city2].1;
        println!("    {} ↔ {} ({:.1}h)", name1, name2, time);
    }

    println!("✅ Created {} routes", network.num_edges());

    // Convert to tropical network for efficient path computation
    println!("\n🌴 Converting to tropical network representation...");

    let tropical_network = network.to_tropical_network()?;
    println!(
        "✅ Tropical network created with {} nodes",
        tropical_network.size()
    );

    println!("\nIn tropical algebra:");
    println!("  • Addition (⊕) becomes max operation");
    println!("  • Multiplication (⊗) becomes addition");
    println!("  • This transforms shortest path to tropical matrix operations");

    // Find shortest paths using tropical algebra
    println!("\n🎯 Finding shortest paths using tropical algebra:");

    let test_routes = vec![
        (0, 2, "New York", "Boston"),
        (0, 5, "New York", "Pittsburgh"),
        (1, 6, "Philadelphia", "Buffalo"),
        (3, 9, "Washington DC", "Providence"),
        (5, 9, "Pittsburgh", "Providence"),
    ];

    for (start, end, start_name, end_name) in test_routes {
        println!("\n  🗺️  Route: {} → {}", start_name, end_name);

        // Use tropical network for path finding
        let tropical_result = tropical_network.shortest_path_tropical(start, end)?;
        match &tropical_result {
            Some((path, distance)) => {
                print!("    Path: ");
                for (i, &node_idx) in path.iter().enumerate() {
                    if i > 0 {
                        print!(" → ");
                    }
                    print!("{}", city_indices[node_idx].1);
                }
                println!();
                println!("    Total time: {:.1} hours", distance);

                // Show step-by-step breakdown
                if path.len() > 1 {
                    println!("    Breakdown:");
                    for window in path.windows(2) {
                        let from_idx = window[0];
                        let to_idx = window[1];
                        if let Ok(edge_weight) = tropical_network.get_edge(from_idx, to_idx) {
                            if !edge_weight.is_zero() {
                                println!(
                                    "      {} → {}: {:.1}h",
                                    city_indices[from_idx].1,
                                    city_indices[to_idx].1,
                                    edge_weight.value()
                                );
                            }
                        }
                    }
                }
            }
            None => {
                println!("    ❌ No path found");
            }
        }

        // Compare with standard geometric network shortest path
        match network.shortest_path(start, end)? {
            Some((std_path, std_distance)) => {
                let tropical_path = tropical_result
                    .as_ref()
                    .map(|(path, _)| path.clone())
                    .unwrap_or_default();
                if std_path != tropical_path {
                    println!(
                        "    🔄 Standard algorithm found different path: {:?} ({:.1}h)",
                        std_path
                            .iter()
                            .map(|&i| city_indices[i].1)
                            .collect::<Vec<_>>(),
                        std_distance
                    );
                }
            }
            None => {
                println!("    🔄 Standard algorithm also found no path");
            }
        }
    }

    // Compute all-pairs shortest paths using tropical algebra
    println!("\n📊 Computing all-pairs shortest paths with tropical algebra...");

    let all_pairs_tropical = tropical_network.all_pairs_shortest_paths()?;
    println!(
        "✅ Computed shortest paths between all {} city pairs",
        cities.len()
    );

    // Create a distance matrix display
    println!("\n📋 Travel time matrix (in hours):");
    print!("        ");
    for (_, name) in &city_indices {
        print!("{:>8}", &name[..4.min(name.len())]);
    }
    println!();

    for (i, (_, from_name)) in city_indices.iter().enumerate() {
        print!("{:>8}", &from_name[..4.min(from_name.len())]);
        #[allow(clippy::needless_range_loop)]
        for j in 0..cities.len() {
            let distance = all_pairs_tropical[i][j];
            if distance.is_zero() {
                print!("     ∞  "); // No path
            } else if i == j {
                print!("     0  "); // Self
            } else {
                print!("{:>8.1}", distance.value());
            }
        }
        println!();
    }

    // Analyze network connectivity and efficiency
    println!("\n🔍 Network connectivity analysis:");

    let mut reachable_pairs = 0;
    let mut total_distance = 0.0;
    let mut max_distance = 0.0;
    let mut max_distance_pair = (0, 0);

    for (i, row) in all_pairs_tropical.iter().enumerate().take(cities.len()) {
        for (j, distance) in row.iter().enumerate().take(cities.len()) {
            if i != j && !distance.is_zero() {
                reachable_pairs += 1;
                let dist_value = distance.value();
                total_distance += dist_value;

                if dist_value > max_distance {
                    max_distance = dist_value;
                    max_distance_pair = (i, j);
                }
            }
        }
    }

    let total_pairs = cities.len() * (cities.len() - 1);
    let connectivity = (reachable_pairs as f64 / total_pairs as f64) * 100.0;
    let average_distance = if reachable_pairs > 0 {
        total_distance / reachable_pairs as f64
    } else {
        0.0
    };

    println!(
        "  • Network connectivity: {:.1}% ({}/{} pairs reachable)",
        connectivity, reachable_pairs, total_pairs
    );
    println!("  • Average travel time: {:.1} hours", average_distance);
    println!(
        "  • Longest route: {} → {} ({:.1} hours)",
        city_indices[max_distance_pair.0].1, city_indices[max_distance_pair.1].1, max_distance
    );

    // Demonstrate tropical betweenness centrality
    println!("\n🌟 Computing tropical betweenness centrality...");

    let tropical_betweenness = tropical_network.tropical_betweenness()?;
    println!("✅ Centrality analysis complete");

    let mut centrality_ranking: Vec<(f64, &str)> = tropical_betweenness
        .iter()
        .enumerate()
        .map(|(i, &centrality)| (centrality, city_indices[i].1))
        .collect();

    centrality_ranking.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    println!("\n🏆 Cities ranked by tropical betweenness centrality:");
    for (i, (centrality, city_name)) in centrality_ranking.iter().enumerate() {
        if *centrality > 0.0 {
            println!("  {}. {}: {:.4}", i + 1, city_name, centrality);
        }
    }

    println!("\n🎉 Tropical path finding analysis complete!");
    println!("This example demonstrated:");
    println!("  ✓ Converting geometric networks to tropical representation");
    println!("  ✓ Efficient shortest path computation using max-plus algebra");
    println!("  ✓ All-pairs shortest path computation");
    println!("  ✓ Network connectivity and efficiency analysis");
    println!("  ✓ Tropical betweenness centrality calculation");
    println!("  ✓ Practical applications in transportation networks");

    Ok(())
}
