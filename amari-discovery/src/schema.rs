// SPDX-License-Identifier: MIT OR Apache-2.0

//! Curated JSON Schemas for the stable discovery protocol.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{DiscoveryError, DiscoveryResult, SCHEMA_V1};

const REQUEST_V1: &str = include_str!("../schemas/request-v1.json");
const RESPONSE_V1: &str = include_str!("../schemas/response-v1.json");
const GOAL_V1: &str = include_str!("../schemas/goal-v1.json");
const PLAN_V1: &str = include_str!("../schemas/plan-v1.json");
const PROBE_V1: &str = include_str!("../schemas/probe-v1.json");

/// One of the five stable v1 protocol schema families.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaKind {
    /// Typed one-shot command request.
    Request,
    /// Versioned response envelope.
    Response,
    /// Normalized planner goal.
    Goal,
    /// Replayable candidate plan.
    Plan,
    /// Saved bounded probe result.
    Probe,
}

impl SchemaKind {
    /// Every schema family in deterministic CLI order.
    pub const ALL: [Self; 5] = [
        Self::Request,
        Self::Response,
        Self::Goal,
        Self::Plan,
        Self::Probe,
    ];

    /// Returns the stable lowercase schema family name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Response => "response",
            Self::Goal => "goal",
            Self::Plan => "plan",
            Self::Probe => "probe",
        }
    }

    const fn source(self) -> &'static str {
        match self {
            Self::Request => REQUEST_V1,
            Self::Response => RESPONSE_V1,
            Self::Goal => GOAL_V1,
            Self::Plan => PLAN_V1,
            Self::Probe => PROBE_V1,
        }
    }
}

impl fmt::Display for SchemaKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One parsed curated protocol schema and its stable identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProtocolSchema {
    /// Schema family.
    pub kind: SchemaKind,
    /// Stable JSON Schema `$id`.
    pub id: String,
    /// Discovery protocol version described by this schema.
    pub protocol_version: String,
    /// Complete JSON Schema document.
    pub document: Value,
}

impl ProtocolSchema {
    /// Serializes the schema document as canonical pretty JSON with a newline.
    ///
    /// # Errors
    ///
    /// Returns a serialization error when the in-memory JSON cannot be encoded.
    pub fn canonical_json(&self) -> DiscoveryResult<Vec<u8>> {
        let mut bytes = serde_json::to_vec_pretty(&self.document)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

/// Loads and validates one embedded curated protocol schema.
///
/// # Errors
///
/// Returns catalog corruption when the checked-in schema is malformed or its
/// `$id`/protocol marker is missing.
pub fn protocol_schema(kind: SchemaKind) -> DiscoveryResult<ProtocolSchema> {
    let document: Value = serde_json::from_str(kind.source()).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!(
            "embedded {} schema is malformed: {error}",
            kind.as_str()
        ))
    })?;
    let id = document
        .get("$id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            DiscoveryError::CatalogCorruption(format!(
                "embedded {} schema has no `$id`",
                kind.as_str()
            ))
        })?
        .to_owned();
    let protocol_version = document
        .get("x-amari-protocol-version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            DiscoveryError::CatalogCorruption(format!(
                "embedded {} schema has no protocol marker",
                kind.as_str()
            ))
        })?
        .to_owned();
    if protocol_version != SCHEMA_V1 {
        return Err(DiscoveryError::CatalogCorruption(format!(
            "embedded {} schema protocol `{protocol_version}` is unsupported",
            kind.as_str()
        )));
    }
    Ok(ProtocolSchema {
        kind,
        id,
        protocol_version,
        document,
    })
}
