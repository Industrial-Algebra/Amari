// SPDX-License-Identifier: MIT OR Apache-2.0

//! Embedded structural and semantic capability catalog.

pub mod generator;
mod model;

use std::collections::{HashMap, HashSet};

use sha2::{Digest, Sha256};

use crate::{CapabilityId, DiscoveryError, DiscoveryResult};

pub use model::{
    AssociatedItemRecord, CapabilityRecord, CapabilityRelation, CfgGateRecord, CostHint,
    CrateRecord, DependencyEdgeRecord, DependencyRecord, ExampleRecord, FeatureRecord, FieldRecord,
    ItemRecord, ItemShape, ItemVariantRecord, MacroCatalogRecord, ProbeDescriptor, ProbeLimits,
    ProbeManifest, RelationshipEndpointRecord, SemanticCatalog, SideEffectPolicy, StabilityTier,
    StructuralCatalog, SuperTraitConstraintRecord, TargetRecord, TraitDefinitionRecord,
    TraitImplementationRecord, TraitItemRecord, VariantDataRecord, VariantFieldRecord,
    VariantRecord, WasmCapabilityMappingRef, WasmSurfaceRef,
};

const GENERATED: &str = include_str!("../../catalog/generated.json");
const SEMANTIC: &str = include_str!("../../catalog/semantic/core.toml");
const PROBES: &str = include_str!("../../catalog/probes.toml");

/// Validated structural, semantic, and probe catalog data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Catalog {
    structural: StructuralCatalog,
    semantic: SemanticCatalog,
    probes: ProbeManifest,
    content_hash: String,
}

impl Catalog {
    /// Loads and validates the catalog embedded in this crate.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::CatalogCorruption`] when embedded JSON/TOML
    /// cannot be parsed or any structural/semantic invariant is violated.
    pub fn embedded() -> DiscoveryResult<Self> {
        Self::from_sources(GENERATED, SEMANTIC, PROBES)
    }

    /// Parses and validates explicit catalog source documents.
    ///
    /// This is primarily useful for deterministic catalog generation and drift
    /// checks. It performs no filesystem or network access.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::CatalogCorruption`] for malformed documents,
    /// duplicate IDs, dangling references, or incomplete probe contracts.
    pub fn from_sources(
        structural_json: &str,
        semantic_toml: &str,
        probes_toml: &str,
    ) -> DiscoveryResult<Self> {
        let structural: StructuralCatalog =
            serde_json::from_str(structural_json).map_err(|error| {
                DiscoveryError::CatalogCorruption(format!("invalid structural JSON: {error}"))
            })?;
        let semantic = toml::from_str(semantic_toml).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!("invalid semantic TOML: {error}"))
        })?;
        let probes = toml::from_str(probes_toml).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!("invalid probe TOML: {error}"))
        })?;

        // For schema2: the supplied content_hash from the JSON is preserved
        // as-is. Catalog::validate() recomputes and verifies it, so tampered
        // checked-in files are caught at validation time.

        let mut hasher = Sha256::new();
        for (label, source) in [
            ("structural", structural_json),
            ("semantic", semantic_toml),
            ("probes", probes_toml),
        ] {
            hasher.update(label.as_bytes());
            hasher.update([0]);
            hasher.update(source.as_bytes());
            hasher.update([0]);
        }

        let catalog = Self {
            structural,
            semantic,
            probes,
            content_hash: hex::encode(hasher.finalize()),
        };
        catalog.validate()?;
        Ok(catalog)
    }

    /// Validates uniqueness, reference integrity, versions, and probe contracts.
    ///
    /// For schema version 2, additionally validates:
    /// - `content_hash` is present and matches canonical recomputation
    /// - `probe_descriptors` exactly equal the separately loaded ProbeManifest.probes
    /// - `wasm_surface` is Some with valid hash, counts, and capability mappings
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::CatalogCorruption`] when any catalog invariant
    /// is violated.
    pub fn validate(&self) -> DiscoveryResult<()> {
        if self.structural.schema_version != 1 && self.structural.schema_version != 2 {
            return catalog_error("structural schema_version must be 1 or 2");
        }
        if self.structural.version.is_empty()
            || self.structural.version != self.semantic.catalog_version
            || self.structural.version != self.probes.catalog_version
        {
            return catalog_error("structural, semantic, and probe versions must match");
        }
        if self.structural.crates.is_empty() {
            return catalog_error("structural catalog must contain at least one crate");
        }

        // Schema2: validate content_hash by canonical recomputation.
        if self.structural.schema_version == 2 {
            let hash = self.structural.content_hash.as_ref().ok_or_else(|| {
                DiscoveryError::CatalogCorruption(
                    "schema2 requires content_hash to be present".into(),
                )
            })?;
            if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
                return catalog_error("content_hash must be a 64-char hex string");
            }
            // Recompute hash and compare.
            // Serialize without the hash field for canonical recomputation.
            let mut hasher = Sha256::new();
            let mut for_hash = self.structural.clone();
            for_hash.content_hash = None;
            let json_without_hash = serde_json::to_vec_pretty(&for_hash).map_err(|error| {
                DiscoveryError::CatalogCorruption(format!(
                    "cannot serialize catalog for content_hash verification: {error}"
                ))
            })?;
            hasher.update(&json_without_hash);
            let recomputed = hex::encode(hasher.finalize());
            if hash != &recomputed {
                return catalog_error(format!(
                    "content_hash mismatch: expected {recomputed}, got {hash}"
                ));
            }

            // Schema2: require wasm_surface to be Some.
            let wasm = self.structural.wasm_surface.as_ref().ok_or_else(|| {
                DiscoveryError::CatalogCorruption(
                    "schema2 requires wasm_surface to be present".into(),
                )
            })?;
            if wasm.path.is_empty() {
                return catalog_error("wasm_surface.path must be nonempty");
            }
            if wasm.source_hash.len() != 64
                || !wasm.source_hash.chars().all(|c| c.is_ascii_hexdigit())
            {
                return catalog_error("wasm_surface.source_hash must be a 64-char hex string");
            }
            if wasm.class_count == 0
                && wasm.function_count == 0
                && wasm.enum_count == 0
                && wasm.interface_count == 0
            {
                return catalog_error("wasm_surface must have at least one export count");
            }

            // Schema2: WASM capability mapping integrity.
            // Every wasm_path must be nonempty and unique; every capability_id
            // must parse as a valid CapabilityId.
            {
                let mut wasm_paths = std::collections::HashSet::new();
                for (i, mapping) in wasm.capability_mappings.iter().enumerate() {
                    if mapping.wasm_path.is_empty() {
                        return catalog_error(format!(
                            "wasm_surface.capability_mappings[{i}]: wasm_path must be nonempty"
                        ));
                    }
                    if !wasm_paths.insert(&mapping.wasm_path) {
                        return catalog_error(format!(
                            "wasm_surface.capability_mappings[{i}]: duplicate wasm_path '{}'",
                            mapping.wasm_path
                        ));
                    }
                    // Validate the capability_id parses.
                    if mapping.capability_id.parse::<CapabilityId>().is_err() {
                        return catalog_error(format!(
                            "wasm_surface.capability_mappings[{i}]: invalid capability_id '{}'",
                            mapping.capability_id
                        ));
                    }
                }
            }

            // Schema2: probe_descriptors must exactly equal the separately loaded
            // ProbeManifest.probes. Full structural equality, not just count+IDs,
            // so that modified limits, features, or cost trigger corruption.
            if self.structural.probe_descriptors != self.probes.probes {
                return catalog_error(format!(
                    "structural probe_descriptors must match probes.toml exactly ({} vs {} probes)",
                    self.structural.probe_descriptors.len(),
                    self.probes.probes.len(),
                ));
            }
        }

        let mut crate_names = HashSet::new();
        let mut feature_refs = HashSet::new();
        let mut item_paths = HashSet::new();
        let mut example_refs = HashSet::new();
        for crate_record in &self.structural.crates {
            if crate_record.name.is_empty() || !crate_names.insert(crate_record.name.as_str()) {
                return catalog_error("crate names must be nonempty and unique");
            }
            if crate_record.version.is_empty() || crate_record.description.is_empty() {
                return catalog_error("crate version and description must be nonempty");
            }
            for feature in &crate_record.features {
                let key = format!("{}:{}", crate_record.name, feature.name);
                if feature.name.is_empty() || !feature_refs.insert(key) {
                    return catalog_error("feature references must be nonempty and unique");
                }
            }
            for item in &crate_record.items {
                if item.path.is_empty() || !item_paths.insert(item.path.as_str()) {
                    return catalog_error("item paths must be nonempty and globally unique");
                }
                // Single-variant items must have a non-empty kind; multi-variant
                // items omit the canonical summary kind but must have variants.
                if let Some(kind) = &item.kind {
                    if kind.is_empty() {
                        return catalog_error("item kind must be nonempty when present");
                    }
                } else if item.variants.len() < 2 {
                    return catalog_error(format!(
                        "item {}: kind absent but fewer than 2 variants ({})",
                        item.path,
                        item.variants.len()
                    ));
                }
                // Variants must be non-empty.
                if item.variants.is_empty() {
                    return catalog_error(format!(
                        "item {}: variants must not be empty",
                        item.path
                    ));
                }
            }
            for example in &crate_record.examples {
                let key = format!("{}:{}", crate_record.name, example.name);
                if example.name.is_empty() || example.path.is_empty() || !example_refs.insert(key) {
                    return catalog_error("example references must be complete and unique");
                }
            }
        }

        let mut capability_ids = HashSet::new();
        for capability in &self.semantic.capabilities {
            if !capability_ids.insert(capability.id.to_string()) {
                return catalog_error("capability IDs must be unique");
            }
            if capability.name.is_empty()
                || capability.description.is_empty()
                || capability.crate_refs.is_empty()
            {
                return catalog_error("capabilities require name, description, and crate refs");
            }
            if !capability
                .crate_refs
                .iter()
                .all(|reference| crate_names.contains(reference.as_str()))
            {
                return catalog_error("semantic capability references an unknown crate");
            }
            if !capability
                .feature_refs
                .iter()
                .all(|reference| feature_refs.contains(reference))
            {
                return catalog_error("semantic capability references an unknown feature");
            }
            if !capability
                .symbol_refs
                .iter()
                .all(|reference| item_paths.contains(reference.as_str()))
            {
                return catalog_error("semantic capability references an unknown symbol");
            }
            if !capability
                .example_refs
                .iter()
                .all(|reference| example_refs.contains(reference))
            {
                return catalog_error("semantic capability references an unknown example");
            }
        }

        // Schema-2 WASM mappings are part of the semantic discovery surface,
        // not merely syntactically valid identifiers. Reject mappings whose
        // capability is absent from the curated semantic catalog.
        if self.structural.schema_version == 2 {
            let wasm = self.structural.wasm_surface.as_ref().ok_or_else(|| {
                DiscoveryError::CatalogCorruption(
                    "schema2 requires wasm_surface to be present".into(),
                )
            })?;
            if wasm
                .capability_mappings
                .iter()
                .any(|mapping| !capability_ids.contains(&mapping.capability_id.to_string()))
            {
                return catalog_error(
                    "wasm_surface capability mapping references an unknown capability",
                );
            }
        }

        let mut probe_ids = HashSet::new();
        let mut probe_owners = HashMap::new();
        for probe in &self.probes.probes {
            let probe_id = probe.id.to_string();
            if !probe_ids.insert(probe_id.clone()) {
                return catalog_error("probe IDs must be unique");
            }
            let owner = probe.capability_id.to_string();
            if !capability_ids.contains(&owner) {
                return catalog_error("probe references an unknown capability");
            }
            probe_owners.insert(probe_id.clone(), owner);

            let Some(probe_version) = probe_id.rsplit(':').next() else {
                return catalog_error("probe ID has no version segment");
            };
            let Some(input_contract) =
                probe_schema_contract(&probe.input_schema, "input", probe_version)
            else {
                return catalog_error("probe input schema is malformed or version-mismatched");
            };
            let Some(output_contract) =
                probe_schema_contract(&probe.output_schema, "output", probe_version)
            else {
                return catalog_error("probe output schema is malformed or version-mismatched");
            };
            if input_contract != output_contract {
                return catalog_error("probe input/output schema contracts must match");
            }
            let mut required_features = HashSet::new();
            if !probe.required_features.iter().all(|feature| {
                matches!(feature.as_str(), "standard-probes" | "ai")
                    && required_features.insert(feature.as_str())
            }) {
                return catalog_error("probe required features must be known and unique");
            }
            if probe.limits.max_input_bytes == 0
                || probe.limits.max_output_bytes == 0
                || probe.limits.max_operations == 0
                || probe.limits.timeout_millis == 0
            {
                return catalog_error("probe limits must be greater than zero");
            }
        }

        let mut referenced_probes = HashSet::new();
        for capability in &self.semantic.capabilities {
            let capability_id = capability.id.to_string();
            for reference in &capability.probe_refs {
                let probe_id = reference.to_string();
                if probe_owners.get(&probe_id) != Some(&capability_id) {
                    return catalog_error(
                        "semantic capability references a missing or differently owned probe",
                    );
                }
                referenced_probes.insert(probe_id);
            }
        }
        if referenced_probes != probe_ids {
            return catalog_error("every probe must be referenced by its owning capability");
        }

        let mut relation_keys = HashSet::new();
        for relation in &self.semantic.relations {
            if !capability_ids.contains(&relation.from.to_string())
                || !capability_ids.contains(&relation.to.to_string())
                || relation.kind.is_empty()
            {
                return catalog_error("capability relation has a missing endpoint or kind");
            }
            let key = format!("{}:{}:{}", relation.from, relation.kind, relation.to);
            if !relation_keys.insert(key) {
                return catalog_error("capability relationships must be unique");
            }
        }

        Ok(())
    }

    /// Returns structural crate records.
    pub fn crates(&self) -> &[CrateRecord] {
        &self.structural.crates
    }

    /// Returns curated capability records.
    pub fn capabilities(&self) -> &[CapabilityRecord] {
        &self.semantic.capabilities
    }

    /// Returns curated capability relationships.
    pub fn relations(&self) -> &[CapabilityRelation] {
        &self.semantic.relations
    }

    /// Returns known declarative probe descriptors.
    pub fn probes(&self) -> &[ProbeDescriptor] {
        &self.probes.probes
    }

    /// Returns authoritative WASM export-to-capability mappings.
    ///
    /// Schema-2 validation guarantees unique WASM paths and semantic IDs that
    /// resolve within this catalog.
    pub fn wasm_capability_mappings(&self) -> &[WasmCapabilityMappingRef] {
        self.structural
            .wasm_surface
            .as_ref()
            .map_or(&[], |surface| surface.capability_mappings.as_slice())
    }

    /// Returns the Amari release version represented by the catalog.
    pub fn version(&self) -> &str {
        &self.structural.version
    }

    /// Returns the deterministic SHA-256 hash of all embedded source documents.
    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }
}

fn probe_schema_contract<'a>(
    schema: &'a str,
    expected_direction: &str,
    expected_version: &str,
) -> Option<&'a str> {
    let segments: Vec<_> = schema.split('/').collect();
    let [namespace, probe, contract, direction, version] = segments.as_slice() else {
        return None;
    };
    let canonical_contract = !contract.is_empty()
        && contract.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
        && contract
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && contract
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric);
    (*namespace == "amari.discovery"
        && *probe == "probe"
        && canonical_contract
        && *direction == expected_direction
        && *version == expected_version)
        .then_some(*contract)
}

fn catalog_error<T>(message: impl Into<String>) -> DiscoveryResult<T> {
    Err(DiscoveryError::CatalogCorruption(message.into()))
}
