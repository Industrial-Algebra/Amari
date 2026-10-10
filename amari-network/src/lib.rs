//! # Geometric Network Analysis
//!
//! This crate provides graph/network analysis tools where nodes are embedded
//! in Clifford algebra (geometric algebra) space. This enables:
//!
//! - Geometric distance metrics between nodes
//! - Community detection via geometric clustering
//! - Information diffusion using geometric products
//! - Fast path-finding with tropical algebra
//!
//! ## Mathematical Foundation
//!
//! Nodes are represented as multivectors in Cl(P,Q,R), the Clifford algebra
//! with signature (P,Q,R). The geometric distance between nodes uses the
//! natural norm in this space: `||a - b||` where `a` and `b` are node positions.
//!
//! ## Basic Usage
//!
//! ```rust
//! use amari_network::GeometricNetwork;
//! use amari_core::Vector;
//!
//! // Create a network in 3D Euclidean space (signature 3,0,0)
//! let mut network = GeometricNetwork::<3, 0, 0>::new();
//!
//! // Add nodes at specific geometric positions
//! let node1 = network.add_node(Vector::from_components(1.0, 0.0, 0.0).mv);
//! let node2 = network.add_node(Vector::from_components(0.0, 1.0, 0.0).mv);
//!
//! // Connect nodes with weighted edges
//! network.add_edge(node1, node2, 1.0).unwrap();
//!
//! // Compute geometric properties
//! let distance = network.geometric_distance(node1, node2);
//! let centrality = network.compute_geometric_centrality();
//! ```
//!
//! ## Features
//!
//! - **Geometric Embedding**: Nodes as multivectors in Clifford algebra space
//! - **Tropical Optimization**: Fast path-finding using tropical (max-plus) algebra
//! - **Community Detection**: Spectral and geometric clustering methods
//! - **Diffusion Modeling**: Information propagation via geometric products
//! - **Centrality Measures**: Geometric, betweenness, and eigenvector centrality
//!
//! ## Application Areas
//!
//! - Social network analysis with semantic embeddings
//! - Citation networks with geometric document representations
//! - Graph neural networks with geometric features
//! - Epistemic network analysis
//! - Belief/information propagation modeling

use amari_core::Multivector;
use std::collections::HashMap;

pub mod error;
pub mod tropical;

// Formal verification modules (optional)
#[cfg(feature = "formal-verification")]
pub mod verified;
#[cfg(feature = "formal-verification")]
pub mod verified_contracts;

pub use error::{NetworkError, NetworkResult};

// Re-export formal verification types when feature is enabled
#[cfg(feature = "formal-verification")]
pub use verified::{NetworkProperty, NetworkSignature, VerifiedGeometricNetwork};
#[cfg(feature = "formal-verification")]
pub use verified_contracts::{
    GeometricProperties, GraphTheoreticProperties, TropicalProperties,
    VerifiedContractGeometricNetwork,
};

/// A network where nodes are embedded in geometric algebra space
///
/// Each node is represented as a multivector in Cl(P,Q,R), enabling
/// geometric operations like distance computation and geometric products
/// for information diffusion.
#[derive(Clone, Debug)]
pub struct GeometricNetwork<const P: usize, const Q: usize, const R: usize> {
    /// Node positions as multivectors in Cl(P,Q,R)
    nodes: Vec<Multivector<P, Q, R>>,
    /// Weighted directed edges
    edges: Vec<GeometricEdge>,
    /// Optional metadata for each node
    node_metadata: HashMap<usize, NodeMetadata>,
}

/// Edge with geometric properties
///
/// Represents a weighted directed edge between two nodes in the network.
/// The weight can represent strength of connection, similarity, or distance.
#[derive(Clone, Debug, PartialEq)]
pub struct GeometricEdge {
    /// Source node index
    pub source: usize,
    /// Target node index
    pub target: usize,
    /// Edge weight (strength, similarity, etc.)
    pub weight: f64,
}

/// Metadata associated with a network node
///
/// Allows attaching labels and properties to nodes for analysis
/// and visualization purposes.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeMetadata {
    /// Optional human-readable label
    pub label: Option<String>,
    /// Custom numerical properties
    pub properties: HashMap<String, f64>,
}

/// Result of community detection analysis
///
/// Represents a detected community with its member nodes,
/// geometric centroid, and cohesion score.
#[derive(Clone, Debug)]
pub struct Community<const P: usize, const Q: usize, const R: usize> {
    /// Node indices belonging to this community
    pub nodes: Vec<usize>,
    /// Geometric centroid of community nodes
    pub geometric_centroid: Multivector<P, Q, R>,
    /// Cohesion score (higher = more cohesive)
    pub cohesion_score: f64,
}

/// Result of information propagation analysis
///
/// Tracks how information spreads through the network over time,
/// measuring coverage, influence, and convergence properties.
#[derive(Clone, Debug)]
pub struct PropagationAnalysis {
    /// Number of nodes reached at each timestep
    pub coverage: Vec<usize>,
    /// Influence score for each node (total information transmitted)
    pub influence_scores: Vec<f64>,
    /// Number of steps until convergence
    pub convergence_time: usize,
}

impl NodeMetadata {
    /// Create new empty metadata
    pub fn new() -> Self {
        Self {
            label: None,
            properties: HashMap::new(),
        }
    }

    /// Create metadata with label
    pub fn with_label(label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
            properties: HashMap::new(),
        }
    }

    /// Add a numerical property
    pub fn with_property(mut self, key: impl Into<String>, value: f64) -> Self {
        self.properties.insert(key.into(), value);
        self
    }
}

impl Default for NodeMetadata {
    fn default() -> Self {
        Self::new()
    }
}

impl<const P: usize, const Q: usize, const R: usize> GeometricNetwork<P, Q, R> {
    /// Create a new empty geometric network
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            node_metadata: HashMap::new(),
        }
    }

    /// Create a network with pre-allocated capacity
    pub fn with_capacity(num_nodes: usize, num_edges: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(num_nodes),
            edges: Vec::with_capacity(num_edges),
            node_metadata: HashMap::with_capacity(num_nodes),
        }
    }

    /// Add a node at the specified geometric position
    ///
    /// Returns the index of the newly added node.
    pub fn add_node(&mut self, position: Multivector<P, Q, R>) -> usize {
        let index = self.nodes.len();
        self.nodes.push(position);
        index
    }

    /// Add a node with associated metadata
    ///
    /// Returns the index of the newly added node.
    pub fn add_node_with_metadata(
        &mut self,
        position: Multivector<P, Q, R>,
        metadata: NodeMetadata,
    ) -> usize {
        let index = self.add_node(position);
        self.node_metadata.insert(index, metadata);
        index
    }

    /// Add a directed edge between two nodes
    pub fn add_edge(&mut self, source: usize, target: usize, weight: f64) -> NetworkResult<()> {
        if source >= self.nodes.len() {
            return Err(NetworkError::NodeIndexOutOfBounds(source));
        }
        if target >= self.nodes.len() {
            return Err(NetworkError::NodeIndexOutOfBounds(target));
        }

        self.edges.push(GeometricEdge {
            source,
            target,
            weight,
        });
        Ok(())
    }

    /// Add an undirected edge (creates two directed edges)
    pub fn add_undirected_edge(&mut self, a: usize, b: usize, weight: f64) -> NetworkResult<()> {
        self.add_edge(a, b, weight)?;
        self.add_edge(b, a, weight)?;
        Ok(())
    }

    /// Get the number of nodes in the network
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of edges in the network
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    /// Get a reference to a node's position
    pub fn get_node(&self, index: usize) -> Option<&Multivector<P, Q, R>> {
        self.nodes.get(index)
    }

    /// Get metadata for a node
    pub fn get_metadata(&self, index: usize) -> Option<&NodeMetadata> {
        self.node_metadata.get(&index)
    }

    /// Get all neighbors of a node
    pub fn neighbors(&self, node: usize) -> Vec<usize> {
        self.edges
            .iter()
            .filter(|edge| edge.source == node)
            .map(|edge| edge.target)
            .collect()
    }

    /// Get the degree (number of outgoing edges) of a node
    pub fn degree(&self, node: usize) -> usize {
        self.edges.iter().filter(|edge| edge.source == node).count()
    }

    /// Get all edges in the network
    pub fn edges(&self) -> &[GeometricEdge] {
        &self.edges
    }

    /// Compute geometric distance between two nodes
    ///
    /// Uses the natural norm in Clifford algebra space: ||a - b||
    /// where a and b are the multivector positions of the nodes.
    pub fn geometric_distance(&self, node1: usize, node2: usize) -> NetworkResult<f64> {
        if node1 >= self.nodes.len() {
            return Err(NetworkError::NodeIndexOutOfBounds(node1));
        }
        if node2 >= self.nodes.len() {
            return Err(NetworkError::NodeIndexOutOfBounds(node2));
        }

        let diff = self.nodes[node1].clone() - self.nodes[node2].clone();
        Ok(diff.coefficient_norm())
    }

    /// Compute geometric centrality for all nodes
    ///
    /// Geometric centrality is based on the inverse of the sum of geometric
    /// distances to all other nodes. Nodes closer to the geometric center
    /// of the network have higher centrality.
    pub fn compute_geometric_centrality(&self) -> NetworkResult<Vec<f64>> {
        if self.nodes.is_empty() {
            return Ok(Vec::new());
        }

        let mut centrality = vec![0.0; self.nodes.len()];

        for (i, centrality_value) in centrality.iter_mut().enumerate().take(self.nodes.len()) {
            let mut total_distance = 0.0;
            let mut reachable_count = 0;

            for j in 0..self.nodes.len() {
                if i != j {
                    let distance = self.geometric_distance(i, j)?;
                    if distance > 0.0 {
                        total_distance += distance;
                        reachable_count += 1;
                    }
                }
            }

            // Centrality is inverse of average distance (higher = more central)
            *centrality_value = if total_distance > 0.0 && reachable_count > 0 {
                reachable_count as f64 / total_distance
            } else {
                0.0
            };
        }

        Ok(centrality)
    }

    /// Compute betweenness centrality for all nodes
    ///
    /// Measures how often each node lies on shortest paths between
    /// other pairs of nodes using graph-theoretic shortest paths.
    pub fn compute_betweenness_centrality(&self) -> NetworkResult<Vec<f64>> {
        if self.nodes.is_empty() {
            return Ok(Vec::new());
        }

        let mut betweenness = vec![0.0; self.nodes.len()];
        let distances = self.compute_all_pairs_shortest_paths()?;

        for s in 0..self.nodes.len() {
            for t in 0..self.nodes.len() {
                if s == t {
                    continue;
                }

                if distances[s][t].is_infinite() {
                    continue; // No path from s to t
                }

                // Count nodes on shortest paths from s to t
                for v in 0..self.nodes.len() {
                    if v == s || v == t {
                        continue;
                    }

                    if !distances[s][v].is_infinite() && !distances[v][t].is_infinite() {
                        let path_through_v = distances[s][v] + distances[v][t];

                        // Check if path through v is a shortest path (within tolerance)
                        if (path_through_v - distances[s][t]).abs() < 1e-10 {
                            betweenness[v] += 1.0;
                        }
                    }
                }
            }
        }

        // Normalize by number of node pairs
        let normalization = ((self.nodes.len() - 1) * (self.nodes.len() - 2)) as f64;
        if normalization > 0.0 {
            for value in &mut betweenness {
                *value /= normalization;
            }
        }

        Ok(betweenness)
    }

    /// Compute all-pairs shortest paths using Floyd-Warshall algorithm
    ///
    /// Returns a matrix where element [i][j] is the shortest path distance
    /// from node i to node j using edge weights.
    pub fn compute_all_pairs_shortest_paths(&self) -> NetworkResult<Vec<Vec<f64>>> {
        let n = self.nodes.len();
        let mut distances = vec![vec![f64::INFINITY; n]; n];

        // Initialize distances
        for (i, distance_row) in distances.iter_mut().enumerate().take(n) {
            distance_row[i] = 0.0; // Distance to self is 0
        }

        // Set edge distances
        for edge in &self.edges {
            distances[edge.source][edge.target] = edge.weight;
        }

        // Floyd-Warshall algorithm
        for k in 0..n {
            for i in 0..n {
                for j in 0..n {
                    if distances[i][k] != f64::INFINITY && distances[k][j] != f64::INFINITY {
                        let new_distance = distances[i][k] + distances[k][j];
                        if new_distance < distances[i][j] {
                            distances[i][j] = new_distance;
                        }
                    }
                }
            }
        }

        Ok(distances)
    }

    /// Find shortest path between two nodes using Dijkstra's algorithm
    ///
    /// Returns the path as a vector of node indices and the total distance.
    /// Uses edge weights for distance computation.
    pub fn shortest_path(
        &self,
        source: usize,
        target: usize,
    ) -> NetworkResult<Option<(Vec<usize>, f64)>> {
        if source >= self.nodes.len() {
            return Err(NetworkError::NodeIndexOutOfBounds(source));
        }
        if target >= self.nodes.len() {
            return Err(NetworkError::NodeIndexOutOfBounds(target));
        }

        if source == target {
            return Ok(Some((vec![source], 0.0)));
        }

        let n = self.nodes.len();
        let mut distances = vec![f64::INFINITY; n];
        let mut previous = vec![None; n];
        let mut visited = vec![false; n];

        distances[source] = 0.0;

        // Build adjacency list for efficiency
        let mut adj_list: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        for edge in &self.edges {
            adj_list[edge.source].push((edge.target, edge.weight));
        }

        for _ in 0..n {
            // Find unvisited node with minimum distance
            let mut current = None;
            let mut min_distance = f64::INFINITY;

            for v in 0..n {
                if !visited[v] && distances[v] < min_distance {
                    min_distance = distances[v];
                    current = Some(v);
                }
            }

            let current = match current {
                Some(v) => v,
                None => break, // All remaining nodes are unreachable
            };

            if current == target {
                break; // Found shortest path to target
            }

            visited[current] = true;

            // Update distances to neighbors
            for &(neighbor, weight) in &adj_list[current] {
                if !visited[neighbor] {
                    let new_distance = distances[current] + weight;
                    if new_distance < distances[neighbor] {
                        distances[neighbor] = new_distance;
                        previous[neighbor] = Some(current);
                    }
                }
            }
        }

        // Check if target is reachable
        if distances[target].is_infinite() {
            return Ok(None);
        }

        // Reconstruct path
        let mut path = Vec::new();
        let mut current = target;

        while let Some(prev) = previous[current] {
            path.push(current);
            current = prev;
        }
        path.push(source);
        path.reverse();

        Ok(Some((path, distances[target])))
    }

    /// Find shortest path using geometric distances
    ///
    /// Uses geometric distances between multivector positions rather than edge weights.
    /// This provides a direct path in the geometric algebra space.
    pub fn shortest_geometric_path(
        &self,
        source: usize,
        target: usize,
    ) -> NetworkResult<Option<(Vec<usize>, f64)>> {
        if source >= self.nodes.len() {
            return Err(NetworkError::NodeIndexOutOfBounds(source));
        }
        if target >= self.nodes.len() {
            return Err(NetworkError::NodeIndexOutOfBounds(target));
        }

        if source == target {
            return Ok(Some((vec![source], 0.0)));
        }

        let n = self.nodes.len();
        let mut distances = vec![f64::INFINITY; n];
        let mut previous = vec![None; n];
        let mut visited = vec![false; n];

        distances[source] = 0.0;

        for _ in 0..n {
            // Find unvisited node with minimum distance
            let mut current = None;
            let mut min_distance = f64::INFINITY;

            for v in 0..n {
                if !visited[v] && distances[v] < min_distance {
                    min_distance = distances[v];
                    current = Some(v);
                }
            }

            let current = match current {
                Some(v) => v,
                None => break,
            };

            if current == target {
                break;
            }

            visited[current] = true;

            // Check all nodes that are connected by edges
            for edge in &self.edges {
                if edge.source == current {
                    let neighbor = edge.target;
                    if !visited[neighbor] {
                        let geometric_distance = self.geometric_distance(current, neighbor)?;
                        let new_distance = distances[current] + geometric_distance;

                        if new_distance < distances[neighbor] {
                            distances[neighbor] = new_distance;
                            previous[neighbor] = Some(current);
                        }
                    }
                }
            }
        }

        if distances[target].is_infinite() {
            return Ok(None);
        }

        // Reconstruct path
        let mut path = Vec::new();
        let mut current = target;

        while let Some(prev) = previous[current] {
            path.push(current);
            current = prev;
        }
        path.push(source);
        path.reverse();

        Ok(Some((path, distances[target])))
    }

    /// Convert to tropical network for efficient path operations
    ///
    /// Creates a TropicalNetwork representation using edge weights,
    /// enabling fast shortest path computations via tropical algebra.
    pub fn to_tropical_network(&self) -> NetworkResult<crate::tropical::TropicalNetwork> {
        let n = self.nodes.len();
        let mut weights = vec![vec![f64::INFINITY; n]; n];

        // Set diagonal to 0 (distance to self)
        for (i, weight_row) in weights.iter_mut().enumerate().take(n) {
            weight_row[i] = 0.0;
        }

        // Set edge weights
        for edge in &self.edges {
            weights[edge.source][edge.target] = edge.weight;
        }

        crate::tropical::TropicalNetwork::from_weights(&weights)
    }

    /// Detect communities using geometric clustering
    ///
    /// Groups nodes based on their geometric proximity in the multivector space.
    /// Uses k-means style clustering with geometric distance metric.
    pub fn find_communities(
        &self,
        num_communities: usize,
    ) -> NetworkResult<Vec<Community<P, Q, R>>> {
        if self.nodes.is_empty() {
            return Ok(Vec::new());
        }

        if num_communities == 0 {
            return Err(NetworkError::invalid_param(
                "num_communities",
                0,
                "greater than 0",
            ));
        }

        if num_communities >= self.nodes.len() {
            // Each node is its own community
            let communities = self
                .nodes
                .iter()
                .enumerate()
                .map(|(i, node)| Community {
                    nodes: vec![i],
                    geometric_centroid: node.clone(),
                    cohesion_score: 1.0,
                })
                .collect();
            return Ok(communities);
        }

        // Initialize centroids using k-means++ style selection
        let mut centroids: Vec<Multivector<P, Q, R>> = Vec::with_capacity(num_communities);
        centroids.push(self.nodes[0].clone());

        for _ in 1..num_communities {
            let mut max_distance = 0.0;
            let mut farthest_node = 0;

            for (i, node) in self.nodes.iter().enumerate() {
                let mut min_distance_to_centroid = f64::INFINITY;

                for centroid in &centroids {
                    let diff = node.clone() - centroid.clone();
                    let distance = diff.coefficient_norm();
                    if distance < min_distance_to_centroid {
                        min_distance_to_centroid = distance;
                    }
                }

                if min_distance_to_centroid > max_distance {
                    max_distance = min_distance_to_centroid;
                    farthest_node = i;
                }
            }

            centroids.push(self.nodes[farthest_node].clone());
        }

        // Iterative clustering
        let mut assignments = vec![0; self.nodes.len()];
        let max_iterations = 100;

        for _ in 0..max_iterations {
            let mut changed = false;

            // Assign nodes to nearest centroid
            for (node_idx, node) in self.nodes.iter().enumerate() {
                let mut best_cluster = 0;
                let mut min_distance = f64::INFINITY;

                for (cluster_idx, centroid) in centroids.iter().enumerate() {
                    let diff = node.clone() - centroid.clone();
                    let distance = diff.coefficient_norm();

                    if distance < min_distance {
                        min_distance = distance;
                        best_cluster = cluster_idx;
                    }
                }

                if assignments[node_idx] != best_cluster {
                    assignments[node_idx] = best_cluster;
                    changed = true;
                }
            }

            if !changed {
                break;
            }

            // Update centroids
            for (cluster_idx, centroid) in centroids.iter_mut().enumerate().take(num_communities) {
                let cluster_nodes: Vec<&Multivector<P, Q, R>> = self
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| assignments[*i] == cluster_idx)
                    .map(|(_, node)| node)
                    .collect();

                if !cluster_nodes.is_empty() {
                    // Compute centroid as average
                    let mut sum = cluster_nodes[0].clone();
                    for node in cluster_nodes.iter().skip(1) {
                        sum = sum + (*node).clone();
                    }
                    *centroid = sum * (1.0 / cluster_nodes.len() as f64);
                }
            }
        }

        // Build communities and compute cohesion scores
        let mut communities = Vec::new();

        for (cluster_idx, centroid) in centroids.iter().enumerate().take(num_communities) {
            let cluster_nodes: Vec<usize> = assignments
                .iter()
                .enumerate()
                .filter(|(_, &assignment)| assignment == cluster_idx)
                .map(|(i, _)| i)
                .collect();

            if cluster_nodes.is_empty() {
                continue;
            }

            // Compute cohesion as inverse of average intra-cluster distance
            let mut total_distance = 0.0;
            let mut pair_count = 0;

            for &i in &cluster_nodes {
                for &j in &cluster_nodes {
                    if i < j {
                        let distance = self.geometric_distance(i, j)?;
                        total_distance += distance;
                        pair_count += 1;
                    }
                }
            }

            let cohesion_score = if pair_count > 0 && total_distance > 0.0 {
                pair_count as f64 / total_distance
            } else {
                1.0
            };

            communities.push(Community {
                nodes: cluster_nodes,
                geometric_centroid: centroid.clone(),
                cohesion_score,
            });
        }

        Ok(communities)
    }

    /// Simulate information diffusion through the network
    ///
    /// Models how information spreads using geometric products between node positions.
    /// Initial information is placed at source nodes and propagates based on edge weights
    /// and geometric similarity.
    pub fn simulate_diffusion(
        &self,
        initial_sources: &[usize],
        max_steps: usize,
        diffusion_rate: f64,
    ) -> NetworkResult<PropagationAnalysis> {
        if initial_sources.is_empty() {
            return Err(NetworkError::insufficient_data(
                "No initial sources provided",
            ));
        }

        for &source in initial_sources {
            if source >= self.nodes.len() {
                return Err(NetworkError::NodeIndexOutOfBounds(source));
            }
        }

        if diffusion_rate <= 0.0 || diffusion_rate > 1.0 {
            return Err(NetworkError::invalid_param(
                "diffusion_rate",
                diffusion_rate,
                "between 0 and 1",
            ));
        }

        let n = self.nodes.len();
        let mut information_levels = vec![0.0; n];
        let mut coverage = Vec::new();
        let mut influence_scores = vec![0.0; n];

        // Initialize information at source nodes
        for &source in initial_sources {
            information_levels[source] = 1.0;
        }

        let convergence_threshold = 1e-6;
        let mut converged_step = max_steps;

        for step in 0..max_steps {
            // Count nodes with significant information
            let current_coverage = information_levels
                .iter()
                .filter(|&&level| level > convergence_threshold)
                .count();
            coverage.push(current_coverage);

            let previous_levels = information_levels.clone();
            let mut new_levels = information_levels.clone();

            // Diffuse information along edges
            for edge in &self.edges {
                let source_level = information_levels[edge.source];
                if source_level > convergence_threshold {
                    // Compute geometric similarity for diffusion strength
                    let similarity = self.compute_geometric_similarity(edge.source, edge.target)?;

                    // Information transfer based on edge weight, diffusion rate, and geometric similarity
                    let transfer_amount = source_level * diffusion_rate * similarity * edge.weight;

                    new_levels[edge.target] += transfer_amount;
                    influence_scores[edge.source] += transfer_amount;
                }
            }

            // Apply decay to prevent infinite accumulation
            for level in &mut new_levels {
                *level *= 0.95; // 5% decay per step
            }

            information_levels = new_levels;

            // Check for convergence
            let total_change: f64 = information_levels
                .iter()
                .zip(previous_levels.iter())
                .map(|(new, old)| (new - old).abs())
                .sum();

            if total_change < convergence_threshold {
                converged_step = step + 1;
                break;
            }
        }

        Ok(PropagationAnalysis {
            coverage,
            influence_scores,
            convergence_time: converged_step,
        })
    }

    /// Compute geometric similarity between two nodes
    ///
    /// Uses the geometric product in Clifford algebra to measure similarity.
    /// Higher values indicate more similar geometric positions.
    fn compute_geometric_similarity(&self, node1: usize, node2: usize) -> NetworkResult<f64> {
        let pos1 = &self.nodes[node1];
        let pos2 = &self.nodes[node2];

        // Compute geometric product and extract scalar part as similarity measure
        let product = pos1.geometric_product(pos2);
        let scalar_part = product.scalar_part();

        // Normalize by the norms to get a similarity measure
        let norm1 = pos1.coefficient_norm();
        let norm2 = pos2.coefficient_norm();

        if norm1 > 0.0 && norm2 > 0.0 {
            Ok((scalar_part / (norm1 * norm2)).abs())
        } else {
            Ok(0.0)
        }
    }

    /// Perform spectral clustering using the graph Laplacian
    ///
    /// Uses eigenvalue decomposition of the normalized Laplacian matrix
    /// to identify community structure in the network.
    pub fn spectral_clustering(&self, num_clusters: usize) -> NetworkResult<Vec<Vec<usize>>> {
        use nalgebra::{DMatrix, SymmetricEigen};

        if self.nodes.is_empty() {
            return Ok(Vec::new());
        }

        if num_clusters == 0 {
            return Err(NetworkError::invalid_param(
                "num_clusters",
                0,
                "greater than 0",
            ));
        }

        let n = self.nodes.len();
        if num_clusters >= n {
            // Each node is its own cluster
            return Ok((0..n).map(|i| vec![i]).collect());
        }

        // Build adjacency matrix
        let mut adjacency = DMatrix::zeros(n, n);
        for edge in &self.edges {
            adjacency[(edge.source, edge.target)] = edge.weight;
            // For spectral clustering, typically use symmetric adjacency
            adjacency[(edge.target, edge.source)] = edge.weight;
        }

        // Compute degree matrix
        let mut degree = DMatrix::zeros(n, n);
        for i in 0..n {
            let node_degree: f64 = adjacency.row(i).sum();
            if node_degree > 0.0 {
                degree[(i, i)] = node_degree;
            }
        }

        // Compute normalized Laplacian: L = I - D^(-1/2) * A * D^(-1/2)
        let mut normalized_laplacian = DMatrix::identity(n, n);

        for i in 0..n {
            for j in 0..n {
                if i != j && degree[(i, i)] > 0.0 && degree[(j, j)] > 0.0 {
                    let value = adjacency[(i, j)] / (degree[(i, i)].sqrt() * degree[(j, j)].sqrt());
                    normalized_laplacian[(i, j)] = -value;
                }
            }
        }

        // Compute eigenvalues and eigenvectors
        let eigen = SymmetricEigen::new(normalized_laplacian);
        let eigenvalues = eigen.eigenvalues;
        let eigenvectors = eigen.eigenvectors;

        // Use smallest eigenvalues' eigenvectors for clustering
        let mut clusters = vec![Vec::new(); num_clusters];

        // Simple assignment based on eigenvector values
        for i in 0..n {
            let mut best_cluster = 0;
            let mut max_value = eigenvectors[(i, 0)];

            for k in 1..num_clusters.min(eigenvalues.len()) {
                if eigenvectors[(i, k)].abs() > max_value.abs() {
                    max_value = eigenvectors[(i, k)];
                    best_cluster = k;
                }
            }

            clusters[best_cluster].push(i);
        }

        // Remove empty clusters
        clusters.retain(|cluster| !cluster.is_empty());

        Ok(clusters)
    }
}

impl<const P: usize, const Q: usize, const R: usize> Default for GeometricNetwork<P, Q, R> {
    fn default() -> Self {
        Self::new()
    }
}
