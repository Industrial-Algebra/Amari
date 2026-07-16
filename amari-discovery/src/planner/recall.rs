// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic holographic recall over semantic catalog capabilities.

use std::collections::{BTreeMap, BTreeSet};

use amari_holographic::{BindingAlgebra, MAPAlgebra};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    CapabilityId, CapabilityRecord, Catalog, DiscoveryError, DiscoveryResult, ProjectSnapshot,
};

const RECALL_DIMENSIONS: usize = 512;
type RecallVector = MAPAlgebra<RECALL_DIMENSIONS>;

/// Bounded configuration for deterministic candidate recall.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecallConfig {
    /// Seed used to derive deterministic bipolar vectors from normalized tokens.
    pub seed: u64,
    /// Maximum number of candidates returned.
    pub max_candidates: usize,
    /// Minimum leading holographic score before lexical fallback is used.
    pub minimum_holographic_score: f64,
}

impl Default for RecallConfig {
    fn default() -> Self {
        Self {
            seed: 0xA6A7_1D15_C0A1_2024,
            max_candidates: 8,
            minimum_holographic_score: 0.1,
        }
    }
}

/// The candidate-generation path selected for a retrieved capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalSource {
    /// Deterministic MAP-algebra recall met the configured confidence threshold.
    Holographic,
    /// Token overlap was used because holographic confidence was too low.
    LexicalFallback,
}

/// One catalog-backed capability candidate with transparent recall evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RetrievedCandidate {
    /// Stable capability ID from the supplied catalog.
    pub capability_id: CapabilityId,
    /// Deterministic score used to order this retrieval result.
    pub retrieval_score: f64,
    /// Cosine similarity between normalized holographic query and capability vectors.
    pub holographic_score: f64,
    /// Weighted normalized token overlap between query and capability evidence.
    pub lexical_score: f64,
    /// Normalized query tokens also present in the capability record.
    pub matched_evidence: Vec<String>,
    /// Whether holographic recall or lexical fallback produced this ordering.
    pub source: RetrievalSource,
}

/// Deterministic catalog candidate retriever.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CandidateRetriever {
    config: RecallConfig,
}

impl CandidateRetriever {
    /// Creates a retriever with explicit deterministic limits and seed.
    pub const fn new(config: RecallConfig) -> Self {
        Self { config }
    }

    /// Returns bounded capability candidates for a project snapshot and goal.
    ///
    /// The snapshot contributes only typed, already-sanitized evidence. Raw
    /// source text, absolute paths, and project code are never read here.
    /// Capability and query vectors are accumulated with additive
    /// [`BindingAlgebra::superpose`] and normalized only after accumulation.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::InvalidInput`] for an invalid configuration or
    /// when neither the goal nor snapshot contains usable evidence. Internal
    /// algebra reconstruction failures are returned as typed internal errors.
    pub fn retrieve(
        &self,
        catalog: &Catalog,
        snapshot: &ProjectSnapshot,
        goal: &str,
    ) -> DiscoveryResult<Vec<RetrievedCandidate>> {
        self.validate_config()?;
        if self.config.max_candidates == 0 || catalog.capabilities().is_empty() {
            return Ok(Vec::new());
        }

        let query_tokens = query_tokens(snapshot, goal);
        if query_tokens.is_empty() {
            return Err(DiscoveryError::InvalidInput(
                "candidate recall requires goal or project evidence".to_owned(),
            ));
        }
        let query_vector = self.encode(&query_tokens)?;

        let mut candidates = catalog
            .capabilities()
            .iter()
            .map(|capability| self.score_capability(capability, &query_tokens, &query_vector))
            .collect::<DiscoveryResult<Vec<_>>>()?;

        let leading_holographic = candidates
            .iter()
            .map(|candidate| candidate.holographic_score)
            .max_by(f64::total_cmp)
            .unwrap_or(f64::NEG_INFINITY);
        let source = if leading_holographic >= self.config.minimum_holographic_score {
            RetrievalSource::Holographic
        } else {
            RetrievalSource::LexicalFallback
        };

        for candidate in &mut candidates {
            candidate.source = source;
            candidate.retrieval_score = match source {
                RetrievalSource::Holographic => {
                    0.7 * candidate.holographic_score.max(0.0) + 0.3 * candidate.lexical_score
                }
                RetrievalSource::LexicalFallback => candidate.lexical_score,
            };
        }
        if source == RetrievalSource::LexicalFallback {
            candidates.retain(|candidate| candidate.lexical_score > 0.0);
        }

        candidates.sort_by(|left, right| {
            right
                .retrieval_score
                .total_cmp(&left.retrieval_score)
                .then_with(|| right.lexical_score.total_cmp(&left.lexical_score))
                .then_with(|| left.capability_id.cmp(&right.capability_id))
        });
        candidates.truncate(self.config.max_candidates);
        Ok(candidates)
    }

    fn validate_config(&self) -> DiscoveryResult<()> {
        if !self.config.minimum_holographic_score.is_finite() {
            return Err(DiscoveryError::InvalidInput(
                "minimum holographic score must be finite".to_owned(),
            ));
        }
        Ok(())
    }

    fn score_capability(
        &self,
        capability: &CapabilityRecord,
        query_tokens: &BTreeMap<String, f64>,
        query_vector: &RecallVector,
    ) -> DiscoveryResult<RetrievedCandidate> {
        let capability_tokens = capability_tokens(capability);
        let capability_vector = self.encode(&capability_tokens)?;
        let holographic_score = query_vector.similarity(&capability_vector);
        let lexical_score = lexical_score(query_tokens, &capability_tokens);
        let matched_evidence = query_tokens
            .keys()
            .filter(|token| capability_tokens.contains_key(*token))
            .cloned()
            .collect();

        Ok(RetrievedCandidate {
            capability_id: capability.id.clone(),
            retrieval_score: 0.0,
            holographic_score,
            lexical_score,
            matched_evidence,
            source: RetrievalSource::Holographic,
        })
    }

    fn encode(&self, tokens: &BTreeMap<String, f64>) -> DiscoveryResult<RecallVector> {
        let mut accumulated = RecallVector::map_zero();
        for (token, weight) in tokens {
            let token_vector = RecallVector::from_seed(token_seed(self.config.seed, token));
            let weighted = <RecallVector as BindingAlgebra>::scale(&token_vector, *weight)
                .map_err(algebra_error)?;
            accumulated = <RecallVector as BindingAlgebra>::superpose(&accumulated, &weighted)
                .map_err(algebra_error)?;
        }
        <RecallVector as BindingAlgebra>::normalize(&accumulated).map_err(algebra_error)
    }
}

fn algebra_error(error: amari_holographic::AlgebraError) -> DiscoveryError {
    DiscoveryError::Internal(format!(
        "holographic recall algebra invariant failed: {error}"
    ))
}

fn capability_tokens(capability: &CapabilityRecord) -> BTreeMap<String, f64> {
    let mut tokens = BTreeMap::new();
    add_tokens(&mut tokens, &capability.name, 4.0);
    add_tokens(&mut tokens, &capability.description, 1.0);
    for alias in &capability.aliases {
        add_tokens(&mut tokens, alias, 3.0);
    }
    for concept in &capability.concepts {
        add_tokens(&mut tokens, concept, 3.0);
    }
    for crate_ref in &capability.crate_refs {
        add_tokens(&mut tokens, crate_ref, 1.0);
    }
    for symbol_ref in &capability.symbol_refs {
        add_tokens(&mut tokens, symbol_ref, 1.0);
    }
    tokens
}

fn query_tokens(snapshot: &ProjectSnapshot, goal: &str) -> BTreeMap<String, f64> {
    let mut tokens = BTreeMap::new();
    add_tokens(&mut tokens, goal, 3.0);

    if let Some(rust) = &snapshot.rust {
        for evidence in &rust.vocabulary {
            add_tokens(&mut tokens, &evidence.term, 2.0);
        }
        for usage in &rust.usages {
            add_tokens(&mut tokens, &usage.crate_name, 1.0);
            for segment in &usage.path_segments {
                add_tokens(&mut tokens, segment, 1.0);
            }
        }
    }
    if let Some(cargo) = &snapshot.cargo {
        for package in std::iter::once(&cargo.root_package).chain(&cargo.workspace_members) {
            for dependency in &package.dependencies {
                add_tokens(&mut tokens, &dependency.package_name, 1.0);
                for feature in &dependency.features {
                    add_tokens(&mut tokens, feature, 1.0);
                }
            }
        }
    }
    if let Some(npm) = &snapshot.npm {
        for dependency in &npm.package.dependencies {
            add_tokens(&mut tokens, &dependency.package_name, 1.0);
        }
    }
    if let Some(typescript) = &snapshot.typescript {
        for evidence in &typescript.vocabulary {
            add_tokens(&mut tokens, &evidence.term, 2.0);
        }
        for import in &typescript.imports {
            add_tokens(&mut tokens, &import.package_name, 1.0);
            if let Some(name) = &import.imported_name {
                add_tokens(&mut tokens, name, 1.0);
            }
        }
        for evidence in &typescript.capabilities {
            add_tokens(&mut tokens, &evidence.capability_id.to_string(), 1.0);
            add_tokens(&mut tokens, &evidence.wasm_path, 1.0);
        }
    }
    tokens
}

fn add_tokens(tokens: &mut BTreeMap<String, f64>, text: &str, weight: f64) {
    for token in tokenize(text) {
        tokens
            .entry(token)
            .and_modify(|existing| *existing = existing.max(weight))
            .or_insert(weight);
    }
}

fn tokenize(text: &str) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    for raw in text.split(|character: char| !character.is_ascii_alphanumeric()) {
        let token = raw.to_ascii_lowercase();
        if token.len() < 2 {
            continue;
        }
        tokens.insert(token.clone());
        if token.len() > 4 && token.ends_with('s') && !token.ends_with("ss") {
            tokens.insert(token[..token.len() - 1].to_owned());
        }
    }
    tokens
}

fn lexical_score(query: &BTreeMap<String, f64>, capability: &BTreeMap<String, f64>) -> f64 {
    let total: f64 = query.values().sum();
    if total == 0.0 {
        return 0.0;
    }
    let matched: f64 = query
        .iter()
        .filter_map(|(token, query_weight)| {
            capability
                .get(token)
                .map(|capability_weight| query_weight.min(*capability_weight))
        })
        .sum();
    matched / total
}

fn token_seed(seed: u64, token: &str) -> u64 {
    let mut digest = Sha256::new();
    digest.update(seed.to_le_bytes());
    digest.update((token.len() as u64).to_le_bytes());
    digest.update(token.as_bytes());
    let bytes: [u8; 8] = digest.finalize()[..8]
        .try_into()
        .expect("SHA-256 prefix always has eight bytes");
    u64::from_le_bytes(bytes)
}
