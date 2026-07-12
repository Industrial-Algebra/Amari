// SPDX-License-Identifier: MIT OR Apache-2.0

//! Progressive capability discovery from the embedded catalog.
//!
//! Every handler returns a typed [`Envelope`] payload. Rendering is
//! centralized in the render module. Pure catalog queries carry
//! provenance that identifies the embedded catalog and reports
//! non-project, non-replay constraints.

use std::collections::HashSet;

use serde::Serialize;

use crate::{
    catalog::{CapabilityRecord, Catalog, CrateRecord},
    error::{DiscoveryError, DiscoveryResult},
    protocol::{CapabilityId, Provenance, SCHEMA_V1},
    CatalogIdentity, Compatibility, CostHint, Envelope, ReplayMetadata, StabilityTier,
};

// ---------------------------------------------------------------------------
// Search types
// ---------------------------------------------------------------------------

/// A compact search result — deliberately lighter than a full capability record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SearchResultItem {
    /// Stable capability identifier.
    pub id: CapabilityId,
    /// Concise display name.
    pub name: String,
    /// Human-readable purpose summary.
    pub description: String,
    /// Alternative names for the capability.
    pub aliases: Vec<String>,
    /// Mathematical and software concepts associated with the capability.
    pub concepts: Vec<String>,
    /// API stability tier.
    pub stability: StabilityTier,
    /// Expected relative runtime or integration cost.
    pub cost: CostHint,
}

/// Wrapper carrying raw query metadata alongside ranked results.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SearchResults {
    /// The raw query string as received from the CLI.
    pub query: String,
    /// Deterministically ranked matching capability summaries.
    pub results: Vec<SearchResultItem>,
}

// ---------------------------------------------------------------------------
// Graph types
// ---------------------------------------------------------------------------

/// One directed relationship between curated capabilities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GraphRelationItem {
    /// Source capability.
    pub from: CapabilityId,
    /// Target capability.
    pub to: CapabilityId,
    /// Stable relationship kind such as `composes_with` or `supports`.
    pub kind: String,
}

/// The relationship neighbourhood of a single capability.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GraphResult {
    /// The capability whose neighbourhood was queried.
    pub capability_id: CapabilityId,
    /// Human-readable name of the capability.
    pub capability_name: String,
    /// Inbound and outbound relationships for this capability.
    pub relations: Vec<GraphRelationItem>,
}

// ---------------------------------------------------------------------------
// Example types
// ---------------------------------------------------------------------------

/// One checked-in example resolved from a capability reference.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiscoveredExample {
    /// Crate containing the example.
    pub crate_name: String,
    /// Cargo example target name.
    pub example_name: String,
    /// Workspace-relative source path.
    pub path: String,
}

/// Examples referenced by a single curated capability.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExampleResult {
    /// The capability that references the examples.
    pub capability_id: CapabilityId,
    /// Human-readable name of the capability.
    pub capability_name: String,
    /// Checked-in examples referenced by the capability.
    pub examples: Vec<DiscoveredExample>,
}

// ---------------------------------------------------------------------------
// Provenance helper
// ---------------------------------------------------------------------------

fn catalog_provenance(catalog: &Catalog) -> Provenance {
    Provenance {
        tool_version: env!("CARGO_PKG_VERSION").to_owned(),
        catalog: CatalogIdentity {
            version: catalog.version().to_owned(),
            hash: catalog.content_hash().to_owned(),
        },
        compatibility: Compatibility {
            status: "compatible".into(),
            reasons: vec![],
        },
        replay: ReplayMetadata {
            replayable: false,
            required_hashes: vec![],
            reasons: vec!["pure catalog queries are non-replayable without project input".into()],
        },
        project_hash: None,
        input_hash: None,
        seed: None,
    }
}

fn catalog_envelope<T: Serialize>(catalog: &Catalog, data: T) -> Envelope<T> {
    Envelope {
        schema_version: SCHEMA_V1.to_owned(),
        provenance: catalog_provenance(catalog),
        warnings: Vec::new(),
        data,
    }
}

// ---------------------------------------------------------------------------
// Identifier resolution
// ---------------------------------------------------------------------------

/// Resolves a user-supplied identifier to a capability ID from a slice of records.
///
/// This is the pure, testable core of capability resolution — it operates on
/// a raw slice of [`CapabilityRecord`]s without requiring a full [`Catalog`].
///
/// Resolution is deterministic and rejects ambiguity:
///
/// 1. If the string is a valid [`CapabilityId`] and matches a record, return
///    it directly (ID lookups always take priority).
/// 2. If the string is a valid [`CapabilityId`] but no record has that ID,
///    return [`DiscoveryError::InvalidId`].
/// 3. Otherwise, collect **every** record that matches by name (case-insensitive
///    exact), alias (case-insensitive exact), or exact symbol_ref, deduplicate
///    by [`CapabilityId`], and:
///    - Exactly one match → return it.
///    - Zero matches → return [`DiscoveryError::InvalidId`].
///    - Multiple matches → return [`DiscoveryError::InvalidInput`] listing the
///      ambiguous candidate IDs sorted by [`CapabilityId`].
pub(crate) fn resolve_capability_id_from_records(
    records: &[CapabilityRecord],
    identifier: &str,
) -> DiscoveryResult<CapabilityId> {
    // 1. Direct CapabilityId lookup — always takes priority.
    if let Ok(cap_id) = identifier.parse::<CapabilityId>() {
        if records.iter().any(|c| c.id == cap_id) {
            return Ok(cap_id);
        }
        return Err(DiscoveryError::invalid_id(
            identifier,
            "capability ID not found in the embedded catalog",
        ));
    }

    // 2. Union name/alias (case-insensitive exact) + symbol_ref (exact case-sensitive),
    //    deduplicated by CapabilityId.
    let identifier_lower = identifier.to_lowercase();
    let mut seen = HashSet::new();
    let mut matches: Vec<&CapabilityRecord> = Vec::new();

    for cap in records {
        let is_match = cap.name.to_lowercase() == identifier_lower
            || cap
                .aliases
                .iter()
                .any(|a| a.to_lowercase() == identifier_lower)
            || cap.symbol_refs.iter().any(|s| s == identifier);
        if is_match && seen.insert(cap.id.clone()) {
            matches.push(cap);
        }
    }

    match matches.len() {
        0 => Err(DiscoveryError::invalid_id(
            identifier,
            "not a recognized capability ID, name, alias, or symbol",
        )),
        1 => Ok(matches[0].id.clone()),
        _ => {
            let mut ids: Vec<String> = matches.iter().map(|c| c.id.to_string()).collect();
            ids.sort();
            Err(DiscoveryError::InvalidInput(format!(
                "ambiguous identifier '{}' matches multiple capabilities: {}",
                identifier,
                ids.join(", "),
            )))
        }
    }
}

/// Resolves a user-supplied identifier string to a catalog capability ID.
///
/// Delegates to [`resolve_capability_id_from_records`].
fn resolve_capability(catalog: &Catalog, identifier: &str) -> DiscoveryResult<CapabilityId> {
    resolve_capability_id_from_records(catalog.capabilities(), identifier)
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// Search the embedded catalog for capabilities matching `query`.
///
/// Matching is performed against capability names, aliases, concepts,
/// descriptions, crate references, module paths, and symbol references.
/// Results are ranked deterministically:
///
/// 1. Exact match on capability ID (highest).
/// 2. Exact match on capability name.
/// 3. Prefix match on name or ID.
/// 4. Case-insensitive substring match on name, aliases, concepts,
///    or description.
/// 5. Match on crate, module, or symbol name (lowest).
///
/// Within each rank, ties are broken by [`CapabilityId`] ordering.
pub fn search(catalog: &Catalog, query: &str) -> SearchResults {
    let query_lower = query.to_lowercase();

    // Assign a rank to each capability that matches the query
    struct Candidate {
        cap: CapabilityRecord,
        rank: u8,
    }

    let mut candidates: Vec<Candidate> = Vec::new();

    for cap in catalog.capabilities() {
        let cap_id_str = cap.id.to_string();
        let rank = if cap_id_str == query {
            0 // Exact ID match
        } else if cap.name.to_lowercase() == query_lower {
            1 // Exact name match (case-insensitive)
        } else if cap_id_str.starts_with(query) || cap.name.to_lowercase().starts_with(&query_lower)
        {
            2 // Prefix match (exact on ID, case-insensitive on name)
        } else if cap.name.to_lowercase().contains(&query_lower)
            || cap
                .aliases
                .iter()
                .any(|a| a.to_lowercase().contains(&query_lower))
            || cap
                .concepts
                .iter()
                .any(|c| c.to_lowercase().contains(&query_lower))
            || cap.description.to_lowercase().contains(&query_lower)
        {
            3 // Substring match in name, aliases, concepts, description
        } else if cap
            .crate_refs
            .iter()
            .any(|r| r.to_lowercase().contains(&query_lower))
            || cap
                .symbol_refs
                .iter()
                .any(|s| s.to_lowercase().contains(&query_lower))
        {
            4 // Match in crate/module/symbol name
        } else {
            continue; // No match
        };

        candidates.push(Candidate {
            cap: cap.clone(),
            rank,
        });
    }

    // Stable sort by rank, then by capability ID (deterministic tie-break)
    candidates.sort_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.cap.id.cmp(&b.cap.id)));
    // Deduplicate by ID (a capability might match in multiple ways)
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|c| seen.insert(c.cap.id.clone()));

    let results = candidates
        .into_iter()
        .map(|c| SearchResultItem {
            id: c.cap.id,
            name: c.cap.name,
            description: c.cap.description,
            aliases: c.cap.aliases,
            concepts: c.cap.concepts,
            stability: c.cap.stability,
            cost: c.cap.cost,
        })
        .collect();

    SearchResults {
        query: query.to_owned(),
        results,
    }
}

// ---------------------------------------------------------------------------
// Detail
// ---------------------------------------------------------------------------

/// Return the complete curated [`CapabilityRecord`] for a capability
/// or symbol identifier.
pub fn detail(catalog: &Catalog, identifier: &str) -> DiscoveryResult<CapabilityRecord> {
    let cap_id = resolve_capability(catalog, identifier)?;
    catalog
        .capabilities()
        .iter()
        .find(|c| c.id == cap_id)
        .cloned()
        .ok_or_else(|| {
            DiscoveryError::invalid_id(identifier, "resolved capability not found in catalog")
        })
}

// ---------------------------------------------------------------------------
// Graph
// ---------------------------------------------------------------------------

/// Return the relationship neighbourhood of a capability.
pub fn graph(catalog: &Catalog, identifier: &str) -> DiscoveryResult<GraphResult> {
    let cap_id = resolve_capability(catalog, identifier)?;
    let cap = catalog
        .capabilities()
        .iter()
        .find(|c| c.id == cap_id)
        .ok_or_else(|| {
            DiscoveryError::invalid_id(identifier, "resolved capability not found in catalog")
        })?;

    let mut relations: Vec<GraphRelationItem> = catalog
        .relations()
        .iter()
        .filter(|r| r.from == cap_id || r.to == cap_id)
        .map(|r| GraphRelationItem {
            from: r.from.clone(),
            to: r.to.clone(),
            kind: r.kind.clone(),
        })
        .collect();
    // Deterministic: sort by from, then to, then kind
    relations.sort_by(|a, b| {
        a.from
            .cmp(&b.from)
            .then_with(|| a.to.cmp(&b.to))
            .then_with(|| a.kind.cmp(&b.kind))
    });
    relations.dedup();

    Ok(GraphResult {
        capability_id: cap.id.clone(),
        capability_name: cap.name.clone(),
        relations,
    })
}

// ---------------------------------------------------------------------------
// Example
// ---------------------------------------------------------------------------

/// Resolves example references against structural crate records.
///
/// Each reference must be in `crate:example` form. Every referenced crate
/// and example must exist in the supplied structural records. A missing
/// reference after successful catalog validation signals catalog corruption.
///
/// # Errors
///
/// Returns [`DiscoveryError::CatalogCorruption`] when an example reference
/// is malformed or references a crate or example not present in the records.
pub(crate) fn resolve_example_refs(
    example_refs: &[String],
    crates: &[CrateRecord],
) -> DiscoveryResult<Vec<DiscoveredExample>> {
    let mut examples = Vec::with_capacity(example_refs.len());
    for example_ref in example_refs {
        let (crate_name, example_name) = example_ref.split_once(':').ok_or_else(|| {
            DiscoveryError::CatalogCorruption(format!(
                "malformed example_ref '{example_ref}': expected 'crate:example'"
            ))
        })?;
        let crate_record = crates
            .iter()
            .find(|c| c.name == crate_name)
            .ok_or_else(|| {
                DiscoveryError::CatalogCorruption(format!(
                    "example_ref '{example_ref}' references unknown crate '{crate_name}'"
                ))
            })?;
        let ex = crate_record
            .examples
            .iter()
            .find(|e| e.name == example_name)
            .ok_or_else(|| {
                DiscoveryError::CatalogCorruption(format!(
                    "example_ref '{example_ref}': example '{example_name}' not found in crate '{crate_name}'"
                ))
            })?;
        examples.push(DiscoveredExample {
            crate_name: crate_name.to_owned(),
            example_name: example_name.to_owned(),
            path: ex.path.clone(),
        });
    }
    Ok(examples)
}

/// Return the checked-in examples referenced by a capability.
///
/// # Errors
///
/// Returns [`DiscoveryError::InvalidInput`] when the capability has no
/// example references.
/// Returns [`DiscoveryError::CatalogCorruption`] when an example reference
/// cannot be resolved against the structural catalog.
pub fn example(catalog: &Catalog, identifier: &str) -> DiscoveryResult<ExampleResult> {
    let cap_id = resolve_capability(catalog, identifier)?;
    let cap = catalog
        .capabilities()
        .iter()
        .find(|c| c.id == cap_id)
        .ok_or_else(|| {
            DiscoveryError::invalid_id(identifier, "resolved capability not found in catalog")
        })?;

    if cap.example_refs.is_empty() {
        return Err(DiscoveryError::InvalidInput(format!(
            "capability '{}' has no example references",
            cap.id
        )));
    }

    let examples = resolve_example_refs(&cap.example_refs, catalog.crates())?;

    Ok(ExampleResult {
        capability_id: cap.id.clone(),
        capability_name: cap.name.clone(),
        examples,
    })
}

// ---------------------------------------------------------------------------
// Envelope helpers
// ---------------------------------------------------------------------------

/// Wraps search results in the shared versioned envelope.
pub fn search_envelope(catalog: &Catalog, query: &str) -> Envelope<SearchResults> {
    catalog_envelope(catalog, search(catalog, query))
}

/// Wraps a detail record in the shared versioned envelope.
///
/// # Errors
///
/// Returns a discovery error when the identifier cannot be resolved.
pub fn detail_envelope(
    catalog: &Catalog,
    identifier: &str,
) -> DiscoveryResult<Envelope<CapabilityRecord>> {
    Ok(catalog_envelope(catalog, detail(catalog, identifier)?))
}

/// Wraps a graph result in the shared versioned envelope.
///
/// # Errors
///
/// Returns a discovery error when the identifier cannot be resolved.
pub fn graph_envelope(
    catalog: &Catalog,
    identifier: &str,
) -> DiscoveryResult<Envelope<GraphResult>> {
    Ok(catalog_envelope(catalog, graph(catalog, identifier)?))
}

/// Wraps an example result in the shared versioned envelope.
///
/// # Errors
///
/// Returns a discovery error when the identifier cannot be resolved or
/// the capability has no example references.
pub fn example_envelope(
    catalog: &Catalog,
    identifier: &str,
) -> DiscoveryResult<Envelope<ExampleResult>> {
    Ok(catalog_envelope(catalog, example(catalog, identifier)?))
}

// ============================================================================
// Unit tests — pure resolution and example functions
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::ExampleRecord;

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    fn cap(id: &str, name: &str, aliases: Vec<&str>, symbol_refs: Vec<&str>) -> CapabilityRecord {
        CapabilityRecord {
            id: id.parse().unwrap(),
            name: name.to_owned(),
            description: "test".into(),
            aliases: aliases.into_iter().map(String::from).collect(),
            concepts: vec![],
            crate_refs: vec!["test-crate".into()],
            feature_refs: vec![],
            symbol_refs: symbol_refs.into_iter().map(String::from).collect(),
            example_refs: vec![],
            probe_refs: vec![],
            stability: StabilityTier::Stable,
            cost: CostHint::Low,
        }
    }

    /// Returns IDs from error message text (for assertion helpers).
    fn ids_in_error(err: &DiscoveryError) -> Vec<&str> {
        match err {
            DiscoveryError::InvalidInput(msg) => {
                // Extract the ID list after ": "
                let prefix = ": ";
                if let Some(pos) = msg.find(prefix) {
                    msg[pos + prefix.len()..].split(", ").collect()
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn crate_rec(name: &str) -> CrateRecord {
        CrateRecord {
            name: name.to_owned(),
            version: "0.1.0".into(),
            description: "test crate".into(),
            license: String::new(),
            edition: String::new(),
            manifest_path: String::new(),
            library_outputs: vec![],
            features: vec![],
            dependencies: vec![],
            targets: vec![],
            items: vec![],
            macros: vec![],
            trait_definitions: vec![],
            trait_implementations: vec![],
            cfg_gates: vec![],
            examples: vec![],
            modules: vec![],
            readme: None,
        }
    }

    // -------------------------------------------------------------------------
    // Resolver: direct CapabilityId matches take priority
    // -------------------------------------------------------------------------

    #[test]
    fn resolve_direct_id_match() {
        let records = [
            cap("amari:test:module:alpha", "Alpha", vec![], vec![]),
            cap("amari:test:module:beta", "Beta", vec![], vec![]),
        ];
        let result = resolve_capability_id_from_records(&records, "amari:test:module:alpha");
        assert_eq!(result.unwrap().to_string(), "amari:test:module:alpha");
    }

    #[test]
    fn resolve_direct_id_unknown_returns_invalid_id() {
        let records = [cap("amari:test:module:alpha", "Alpha", vec![], vec![])];
        let result = resolve_capability_id_from_records(&records, "amari:test:module:nonexistent");
        match result {
            Err(DiscoveryError::InvalidId { .. }) => {}
            other => panic!("expected InvalidId, got {other:?}"),
        }
    }

    #[test]
    fn resolve_direct_id_has_priority_over_name_match() {
        // A valid CapabilityId takes priority even if another capability
        // has it as a name.
        let records = [
            cap(
                "amari:test:module:target",
                "amari:test:module:shadow",
                vec![],
                vec![],
            ),
            cap("amari:test:module:shadow", "Shadow", vec![], vec![]),
        ];
        // "amari:test:module:target" is a valid CapabilityId → direct match
        let result = resolve_capability_id_from_records(&records, "amari:test:module:target");
        assert_eq!(result.unwrap().to_string(), "amari:test:module:target");
    }

    // -------------------------------------------------------------------------
    // Resolver: single match by name, alias, or symbol_ref
    // -------------------------------------------------------------------------

    #[test]
    fn resolve_single_name_match() {
        let records = [cap("amari:test:module:alpha", "Alpha", vec![], vec![])];
        let result = resolve_capability_id_from_records(&records, "Alpha");
        assert_eq!(result.unwrap().to_string(), "amari:test:module:alpha");
    }

    #[test]
    fn resolve_case_insensitive_name_match() {
        let records = [cap("amari:test:module:alpha", "Alpha", vec![], vec![])];
        let result = resolve_capability_id_from_records(&records, "alpha");
        assert_eq!(result.unwrap().to_string(), "amari:test:module:alpha");
    }

    #[test]
    fn resolve_single_alias_match() {
        let records = [cap(
            "amari:test:module:alpha",
            "Alpha",
            vec!["alias-one", "alias-two"],
            vec![],
        )];
        let result = resolve_capability_id_from_records(&records, "alias-one");
        assert_eq!(result.unwrap().to_string(), "amari:test:module:alpha");
    }

    #[test]
    fn resolve_alias_case_insensitive() {
        let records = [cap(
            "amari:test:module:alpha",
            "Alpha",
            vec!["Alias-One"],
            vec![],
        )];
        let result = resolve_capability_id_from_records(&records, "alias-one");
        assert_eq!(result.unwrap().to_string(), "amari:test:module:alpha");
    }

    #[test]
    fn resolve_single_symbol_ref_match() {
        let records = [cap(
            "amari:test:module:alpha",
            "Alpha",
            vec![],
            vec!["crate::Module::Alpha"],
        )];
        let result = resolve_capability_id_from_records(&records, "crate::Module::Alpha");
        assert_eq!(result.unwrap().to_string(), "amari:test:module:alpha");
    }

    #[test]
    fn resolve_no_match_returns_invalid_id() {
        let records = [cap("amari:test:module:alpha", "Alpha", vec![], vec![])];
        let result = resolve_capability_id_from_records(&records, "nonexistent");
        match result {
            Err(DiscoveryError::InvalidId { .. }) => {}
            other => panic!("expected InvalidId, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------------
    // Resolver: duplicate case-insensitive names/aliases → InvalidInput
    // -------------------------------------------------------------------------

    #[test]
    fn resolve_duplicate_case_insensitive_names_returns_invalid_input() {
        let records = [
            cap("amari:test:module:alpha", "MyCap", vec![], vec![]),
            cap("amari:test:module:beta", "mycap", vec![], vec![]),
        ];
        let result = resolve_capability_id_from_records(&records, "mycap");
        match &result {
            Err(DiscoveryError::InvalidInput(msg)) => {
                assert!(
                    msg.contains("amari:test:module:alpha"),
                    "error must list alpha: {msg}"
                );
                assert!(
                    msg.contains("amari:test:module:beta"),
                    "error must list beta: {msg}"
                );
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
        let ids = ids_in_error(result.as_ref().unwrap_err());
        assert_eq!(ids.len(), 2);
        // Must be sorted by CapabilityId
        assert_eq!(ids[0], "amari:test:module:alpha");
        assert_eq!(ids[1], "amari:test:module:beta");
    }

    #[test]
    fn resolve_duplicate_aliases_returns_invalid_input() {
        let records = [
            cap("amari:test:module:a", "CapA", vec!["shared-alias"], vec![]),
            cap("amari:test:module:b", "CapB", vec!["shared-alias"], vec![]),
        ];
        let result = resolve_capability_id_from_records(&records, "shared-alias");
        match result {
            Err(DiscoveryError::InvalidInput(_)) => {}
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------------
    // Resolver: duplicate symbol_refs → InvalidInput
    // -------------------------------------------------------------------------

    #[test]
    fn resolve_duplicate_symbol_refs_returns_invalid_input() {
        let records = [
            cap(
                "amari:test:module:a",
                "CapA",
                vec![],
                vec!["crate::SharedSymbol"],
            ),
            cap(
                "amari:test:module:b",
                "CapB",
                vec![],
                vec!["crate::SharedSymbol"],
            ),
        ];
        let result = resolve_capability_id_from_records(&records, "crate::SharedSymbol");
        match result {
            Err(DiscoveryError::InvalidInput(_)) => {}
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------------
    // Resolver: mixed collision (name on A + symbol_ref on B) → InvalidInput
    // -------------------------------------------------------------------------

    #[test]
    fn resolve_mixed_name_symbol_collision_returns_invalid_input() {
        // Capability A matches by name; capability B matches by symbol_ref
        // with the same query string. This must produce InvalidInput.
        let records = [
            cap("amari:test:module:a", "CollisionTarget", vec![], vec![]),
            cap(
                "amari:test:module:b",
                "CapB",
                vec![],
                vec!["CollisionTarget"],
            ),
        ];
        let result = resolve_capability_id_from_records(&records, "CollisionTarget");
        match &result {
            Err(DiscoveryError::InvalidInput(msg)) => {
                assert!(
                    msg.contains("amari:test:module:a"),
                    "must list capability A: {msg}"
                );
                assert!(
                    msg.contains("amari:test:module:b"),
                    "must list capability B: {msg}"
                );
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
        let ids = ids_in_error(result.as_ref().unwrap_err());
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], "amari:test:module:a");
        assert_eq!(ids[1], "amari:test:module:b");
    }

    #[test]
    fn resolve_mixed_alias_symbol_collision_returns_invalid_input() {
        // Capability A matches by alias; capability B matches by symbol_ref
        let records = [
            cap("amari:test:module:a", "CapA", vec!["collision-key"], vec![]),
            cap("amari:test:module:b", "CapB", vec![], vec!["collision-key"]),
        ];
        let result = resolve_capability_id_from_records(&records, "collision-key");
        match result {
            Err(DiscoveryError::InvalidInput(_)) => {}
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------------
    // Resolver: sorted unique candidate IDs in error message
    // -------------------------------------------------------------------------

    #[test]
    fn resolve_invalid_input_lists_candidates_sorted_by_capability_id() {
        let records = [
            cap("amari:test:module:c", "SharedName", vec![], vec![]),
            cap("amari:test:module:a", "SharedName", vec![], vec![]),
            cap("amari:test:module:b", "SharedName", vec![], vec![]),
        ];
        let result = resolve_capability_id_from_records(&records, "SharedName");
        let ids = ids_in_error(result.as_ref().unwrap_err());
        assert_eq!(ids.len(), 3);
        // Sorted by CapabilityId (lexicographic)
        assert_eq!(ids[0], "amari:test:module:a");
        assert_eq!(ids[1], "amari:test:module:b");
        assert_eq!(ids[2], "amari:test:module:c");
    }

    // -------------------------------------------------------------------------
    // Resolver: deduplication when same capability matches multiple ways
    // -------------------------------------------------------------------------

    #[test]
    fn resolve_deduplicates_by_capability_id() {
        // One capability matches by both name and symbol_ref
        let records = [cap(
            "amari:test:module:only",
            "Unique",
            vec![],
            vec!["Unique"],
        )];
        let result = resolve_capability_id_from_records(&records, "Unique");
        // Must resolve to the single capability, not report ambiguity with itself
        assert_eq!(result.unwrap().to_string(), "amari:test:module:only");
    }

    // -------------------------------------------------------------------------
    // Example: resolve_example_refs
    // -------------------------------------------------------------------------

    #[test]
    fn resolve_example_refs_all_valid() {
        let crates = [{
            let mut c = crate_rec("test-crate");
            c.examples = vec![ExampleRecord {
                name: "demo".into(),
                path: "examples/demo.rs".into(),
                required_features: vec![],
            }];
            c
        }];
        let refs = vec!["test-crate:demo".to_string()];
        let examples = resolve_example_refs(&refs, &crates).unwrap();
        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].crate_name, "test-crate");
        assert_eq!(examples[0].example_name, "demo");
        assert_eq!(examples[0].path, "examples/demo.rs");
    }

    #[test]
    fn resolve_example_refs_malformed_ref_returns_catalog_corruption() {
        let crates = [crate_rec("test-crate")];
        let refs = vec!["no-colon-here".to_string()];
        let result = resolve_example_refs(&refs, &crates);
        match result {
            Err(DiscoveryError::CatalogCorruption(msg)) => {
                assert!(
                    msg.contains("malformed example_ref"),
                    "must mention malformed: {msg}"
                );
                assert!(msg.contains("no-colon-here"), "must include the ref: {msg}");
            }
            other => panic!("expected CatalogCorruption, got {other:?}"),
        }
    }

    #[test]
    fn resolve_example_refs_unknown_crate_returns_catalog_corruption() {
        let crates = [crate_rec("test-crate")];
        let refs = vec!["unknown-crate:demo".to_string()];
        let result = resolve_example_refs(&refs, &crates);
        match result {
            Err(DiscoveryError::CatalogCorruption(msg)) => {
                assert!(
                    msg.contains("unknown crate"),
                    "must mention unknown crate: {msg}"
                );
                assert!(
                    msg.contains("unknown-crate"),
                    "must include the crate name: {msg}"
                );
            }
            other => panic!("expected CatalogCorruption, got {other:?}"),
        }
    }

    #[test]
    fn resolve_example_refs_unknown_example_in_known_crate_returns_catalog_corruption() {
        let crates = [{
            let mut c = crate_rec("test-crate");
            c.examples = vec![ExampleRecord {
                name: "existing".into(),
                path: "examples/existing.rs".into(),
                required_features: vec![],
            }];
            c
        }];
        let refs = vec!["test-crate:nonexistent".to_string()];
        let result = resolve_example_refs(&refs, &crates);
        match result {
            Err(DiscoveryError::CatalogCorruption(msg)) => {
                assert!(
                    msg.contains("not found in crate"),
                    "must mention not found: {msg}"
                );
                assert!(
                    msg.contains("nonexistent"),
                    "must include example name: {msg}"
                );
            }
            other => panic!("expected CatalogCorruption, got {other:?}"),
        }
    }

    #[test]
    fn resolve_example_refs_empty_is_ok() {
        let crates = [crate_rec("test-crate")];
        let examples = resolve_example_refs(&[], &crates).unwrap();
        assert!(examples.is_empty());
    }
}
