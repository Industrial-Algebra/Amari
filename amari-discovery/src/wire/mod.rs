// SPDX-License-Identifier: MIT OR Apache-2.0

//! Wire schema contracts for executable discovery probes.
//!
//! The Rust DTO remains the implementation authority. This module contains
//! the extraction-ready data model used to export that authority as stable
//! JSON Schema documents with Amari semantic metadata and canonical hashes.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{DiscoveryError, DiscoveryResult};

/// Direction of a probe wire schema.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WireSchemaRole {
    /// Schema accepted by a probe invocation.
    Input,
    /// Schema emitted by a successful probe invocation.
    Output,
}

impl WireSchemaRole {
    /// Returns the stable lowercase wire representation.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

/// Compatibility meaning of a schema change.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WireCompatibility {
    /// Existing valid payloads remain valid under the checked-in contract.
    AdditivePatch,
    /// The contract intentionally changes meaning and requires a new version.
    VersionedChange,
}

/// A semantic rule exported beside the structural JSON Schema.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WireSemanticConstraint {
    id: String,
    description: String,
}

impl WireSemanticConstraint {
    /// Creates a semantic constraint with a stable machine-readable ID.
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
        }
    }

    /// Returns the stable constraint ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the agent-readable constraint description.
    pub fn description(&self) -> &str {
        &self.description
    }
}

/// An illustrative payload for a wire schema.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WireExample {
    label: String,
    value: serde_json::Value,
}

impl WireExample {
    /// Creates a labeled wire-schema example.
    pub fn new(label: impl Into<String>, value: serde_json::Value) -> Self {
        Self {
            label: label.into(),
            value,
        }
    }

    /// Returns the stable example label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the example payload.
    pub fn value(&self) -> &serde_json::Value {
        &self.value
    }
}

/// Compact schema identity exposed in list or describe surfaces.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProbeSchemaSummary {
    id: String,
    role: WireSchemaRole,
    compatibility: WireCompatibility,
    hash: String,
}

impl ProbeSchemaSummary {
    /// Creates and validates a compact schema identity.
    pub fn new(
        id: impl Into<String>,
        role: WireSchemaRole,
        compatibility: WireCompatibility,
        hash: impl Into<String>,
    ) -> DiscoveryResult<Self> {
        let id = id.into();
        validate_schema_id(&id, role)?;
        let hash = hash.into();
        validate_schema_hash(&hash)?;
        Ok(Self {
            id,
            role,
            compatibility,
            hash,
        })
    }

    /// Returns the stable schema ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the schema role.
    pub const fn role(&self) -> WireSchemaRole {
        self.role
    }

    /// Returns the declared compatibility class.
    pub const fn compatibility(&self) -> WireCompatibility {
        self.compatibility
    }

    /// Returns the canonical SHA-256 hash.
    pub fn hash(&self) -> &str {
        &self.hash
    }
}

/// Complete exported schema document for one probe direction.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProbeSchemaDocument {
    id: String,
    role: WireSchemaRole,
    protocol_version: String,
    structural_schema: serde_json::Value,
    semantic_constraints: Vec<WireSemanticConstraint>,
    examples: Vec<WireExample>,
    compatibility: WireCompatibility,
}

impl ProbeSchemaDocument {
    /// Creates and validates a complete hybrid schema document.
    pub fn new(
        id: impl Into<String>,
        role: WireSchemaRole,
        protocol_version: impl Into<String>,
        structural_schema: serde_json::Value,
        semantic_constraints: Vec<WireSemanticConstraint>,
        examples: Vec<WireExample>,
        compatibility: WireCompatibility,
    ) -> DiscoveryResult<Self> {
        let id = id.into();
        validate_schema_id(&id, role)?;
        let protocol_version = protocol_version.into();
        if protocol_version.trim().is_empty() {
            return Err(DiscoveryError::InvalidInput(
                "wire schema protocol version must be nonempty".to_owned(),
            ));
        }
        if !structural_schema.is_object() {
            return Err(DiscoveryError::InvalidInput(
                "wire structural schema must be a JSON object".to_owned(),
            ));
        }
        Ok(Self {
            id,
            role,
            protocol_version,
            structural_schema,
            semantic_constraints,
            examples,
            compatibility,
        })
    }

    /// Returns the stable schema ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the schema role.
    pub const fn role(&self) -> WireSchemaRole {
        self.role
    }

    /// Returns the compatibility class.
    pub const fn compatibility(&self) -> WireCompatibility {
        self.compatibility
    }

    /// Returns the exported JSON value including Amari metadata.
    pub fn exported_value(&self) -> DiscoveryResult<serde_json::Value> {
        let mut exported = self
            .structural_schema
            .as_object()
            .expect("constructor validates structural schema object")
            .clone();
        exported.insert("$id".to_owned(), serde_json::Value::String(self.id.clone()));
        exported.insert(
            "$schema".to_owned(),
            serde_json::Value::String("https://json-schema.org/draft/2020-12/schema".to_owned()),
        );
        exported.insert(
            "x-amari-schema-role".to_owned(),
            serde_json::to_value(self.role)?,
        );
        exported.insert(
            "x-amari-protocol-version".to_owned(),
            serde_json::Value::String(self.protocol_version.clone()),
        );
        exported.insert(
            "x-amari-semantic-constraints".to_owned(),
            serde_json::to_value(&self.semantic_constraints)?,
        );
        exported.insert(
            "x-amari-examples".to_owned(),
            serde_json::to_value(&self.examples)?,
        );
        exported.insert(
            "x-amari-compatibility".to_owned(),
            serde_json::to_value(self.compatibility)?,
        );
        Ok(serde_json::Value::Object(exported))
    }

    /// Returns canonical pretty JSON bytes with a trailing newline.
    pub fn canonical_json(&self) -> DiscoveryResult<String> {
        let mut encoded = serde_json::to_string_pretty(&self.exported_value()?)?;
        encoded.push('\n');
        Ok(encoded)
    }

    /// Returns the lowercase SHA-256 hash of the canonical document.
    pub fn canonical_hash(&self) -> DiscoveryResult<String> {
        Ok(hex::encode(Sha256::digest(
            self.canonical_json()?.as_bytes(),
        )))
    }

    /// Returns the compact identity for this document.
    pub fn summary(&self) -> DiscoveryResult<ProbeSchemaSummary> {
        ProbeSchemaSummary::new(
            self.id.clone(),
            self.role,
            self.compatibility,
            self.canonical_hash()?,
        )
    }
}

/// Compile-time contract declared by a probe request or response DTO.
///
/// Implementations are generated by `amari-discovery-macros::WireContract`.
/// Associated functions are used so the registry can resolve a contract from
/// the DTO type without constructing a placeholder payload.
pub trait WireContract {
    /// Stable schema identifier shared with the catalog descriptor.
    const SCHEMA_ID: &'static str;
    /// Input or output direction.
    const ROLE: WireSchemaRole;
    /// Declared compatibility class.
    const COMPATIBILITY: WireCompatibility;

    /// Returns the DTO's `schemars`-derived structural schema.
    fn structural_schema() -> serde_json::Value;

    /// Returns authoritative semantic constraints not expressed structurally.
    fn semantic_constraints() -> Vec<WireSemanticConstraint>;

    /// Returns illustrative payloads for agent-facing schema documents.
    fn examples() -> Vec<WireExample>;
}

fn validate_schema_id(id: &str, role: WireSchemaRole) -> DiscoveryResult<()> {
    let segments: Vec<&str> = id.split('/').collect();
    let [namespace, probe, slug, direction, version] = segments.as_slice() else {
        return Err(DiscoveryError::InvalidInput(format!(
            "wire schema ID `{id}` must have namespace/probe/name/direction/version segments"
        )));
    };
    let valid = *namespace == "amari.discovery"
        && *probe == "probe"
        && !slug.is_empty()
        && slug.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
        && *direction == role.as_str()
        && version.starts_with('v')
        && version.len() > 1
        && version[1..]
            .chars()
            .all(|character| character.is_ascii_digit());
    if valid {
        Ok(())
    } else {
        Err(DiscoveryError::InvalidInput(format!(
            "wire schema ID `{id}` is malformed or does not match role `{}`",
            role.as_str()
        )))
    }
}

fn validate_schema_hash(hash: &str) -> DiscoveryResult<()> {
    if hash.len() == 64
        && hash
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
    {
        Ok(())
    } else {
        Err(DiscoveryError::InvalidInput(
            "wire schema hash must be 64 lowercase hexadecimal characters".to_owned(),
        ))
    }
}
