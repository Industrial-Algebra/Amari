// SPDX-License-Identifier: MIT OR Apache-2.0

//! Probe wire schema registration and validation.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::wire::{ProbeSchemaDocument, ProbeSchemaSummary, WireSchemaRole};
use crate::{Catalog, DiscoveryError, DiscoveryResult, ProbeDescriptor, ProbeId};

/// Resolution state for a known probe's declarative schema descriptors.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeSchemaContractState {
    /// Both directions resolve to compiled DTO contracts.
    Resolved,
    /// The descriptor is known but this build has no executable DTO contract.
    Declared,
}

/// One compiled DTO contract registered for a probe adapter.
#[derive(Clone, Debug)]
pub struct ProbeSchemaRegistration {
    probe_id: ProbeId,
    document: ProbeSchemaDocument,
}

impl ProbeSchemaRegistration {
    /// Creates a registration binding a compiled document to an adapter probe.
    pub const fn new(probe_id: ProbeId, document: ProbeSchemaDocument) -> Self {
        Self { probe_id, document }
    }

    /// Returns the owning probe ID.
    pub const fn probe_id(&self) -> &ProbeId {
        &self.probe_id
    }

    /// Returns the complete schema document.
    pub const fn document(&self) -> &ProbeSchemaDocument {
        &self.document
    }
}

/// Compact input/output schema state for one known probe.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProbeSchemaBinding {
    probe_id: ProbeId,
    state: ProbeSchemaContractState,
    input_summary: Option<ProbeSchemaSummary>,
    output_summary: Option<ProbeSchemaSummary>,
}

impl ProbeSchemaBinding {
    /// Returns the known probe ID.
    pub const fn probe_id(&self) -> &ProbeId {
        &self.probe_id
    }

    /// Returns whether both directions resolve to compiled contracts.
    pub const fn state(&self) -> ProbeSchemaContractState {
        self.state
    }

    /// Returns the input schema summary when resolvable.
    pub const fn input_summary(&self) -> Option<&ProbeSchemaSummary> {
        self.input_summary.as_ref()
    }

    /// Returns the output schema summary when resolvable.
    pub const fn output_summary(&self) -> Option<&ProbeSchemaSummary> {
        self.output_summary.as_ref()
    }
}

/// Validated registry of complete wire schema documents.
#[derive(Debug)]
pub struct ProbeWireSchemaRegistry {
    bindings: BTreeMap<ProbeId, ProbeSchemaBinding>,
    documents: BTreeMap<String, ProbeSchemaDocument>,
}

impl ProbeWireSchemaRegistry {
    /// Builds a registry from known descriptors, executable probe IDs, and
    /// compiled DTO contract registrations.
    ///
    /// Non-executable known probes remain declarative. Every executable probe
    /// must have exactly one input and one output document whose IDs, roles,
    /// versions, and owning descriptors match the catalog.
    pub fn build(
        catalog: &Catalog,
        executable_ids: impl IntoIterator<Item = ProbeId>,
        registrations: impl IntoIterator<Item = ProbeSchemaRegistration>,
    ) -> DiscoveryResult<Self> {
        let descriptors = catalog
            .probes()
            .iter()
            .map(|descriptor| (descriptor.id.clone(), descriptor))
            .collect::<BTreeMap<_, _>>();
        let schema_owners = schema_owners(catalog.probes());
        let executable_ids = executable_ids.into_iter().collect::<BTreeSet<_>>();

        for probe_id in &executable_ids {
            if !descriptors.contains_key(probe_id) {
                return Err(DiscoveryError::CatalogCorruption(format!(
                    "executable probe `{probe_id}` references an unknown descriptor"
                )));
            }
        }

        let mut documents = BTreeMap::new();
        let mut executable_documents: BTreeMap<ProbeId, ExecutableDocuments> = BTreeMap::new();
        for registration in registrations {
            let document = registration.document;
            if let Err(error) = document.validate() {
                return Err(DiscoveryError::CatalogCorruption(format!(
                    "invalid registered wire schema `{}`: {error}",
                    document.id()
                )));
            }
            let &(owner_id, role) = schema_owners.get(document.id()).ok_or_else(|| {
                DiscoveryError::CatalogCorruption(format!(
                    "wire schema `{}` does not match any probe descriptor",
                    document.id()
                ))
            })?;
            if role != document.role() {
                return Err(DiscoveryError::CatalogCorruption(format!(
                    "wire schema `{}` role does not match its descriptor direction",
                    document.id()
                )));
            }
            if *owner_id != registration.probe_id {
                return Err(DiscoveryError::CatalogCorruption(format!(
                    "wire schema `{}` is registered for `{}` but belongs to `{owner_id}`",
                    document.id(),
                    registration.probe_id
                )));
            }
            if !executable_ids.contains(owner_id) {
                return Err(DiscoveryError::CatalogCorruption(format!(
                    "wire schema `{}` belongs to non-executable probe `{owner_id}`",
                    document.id()
                )));
            }
            validate_schema_version(owner_id, document.id())?;
            if documents
                .insert(document.id().to_owned(), document.clone())
                .is_some()
            {
                return Err(DiscoveryError::CatalogCorruption(format!(
                    "duplicate wire schema `{}`",
                    document.id()
                )));
            }

            let entry = executable_documents.entry(owner_id.clone()).or_default();
            let slot = match role {
                WireSchemaRole::Input => &mut entry.input,
                WireSchemaRole::Output => &mut entry.output,
            };
            if slot.replace(document).is_some() {
                return Err(DiscoveryError::CatalogCorruption(format!(
                    "duplicate {} wire schema for probe `{owner_id}`",
                    role.as_str()
                )));
            }
        }

        let mut bindings = BTreeMap::new();
        for descriptor in catalog.probes() {
            let binding = if executable_ids.contains(&descriptor.id) {
                let documents = executable_documents.remove(&descriptor.id).ok_or_else(|| {
                    DiscoveryError::CatalogCorruption(format!(
                        "executable probe `{}` has no registered wire schemas",
                        descriptor.id
                    ))
                })?;
                let input = documents.input.ok_or_else(|| {
                    DiscoveryError::CatalogCorruption(format!(
                        "executable probe `{}` is missing its input wire schema",
                        descriptor.id
                    ))
                })?;
                let output = documents.output.ok_or_else(|| {
                    DiscoveryError::CatalogCorruption(format!(
                        "executable probe `{}` is missing its output wire schema",
                        descriptor.id
                    ))
                })?;
                ProbeSchemaBinding {
                    probe_id: descriptor.id.clone(),
                    state: ProbeSchemaContractState::Resolved,
                    input_summary: Some(input.summary()?),
                    output_summary: Some(output.summary()?),
                }
            } else {
                ProbeSchemaBinding {
                    probe_id: descriptor.id.clone(),
                    state: ProbeSchemaContractState::Declared,
                    input_summary: None,
                    output_summary: None,
                }
            };
            bindings.insert(descriptor.id.clone(), binding);
        }

        if let Some((probe_id, _)) = executable_documents.into_iter().next() {
            return Err(DiscoveryError::CatalogCorruption(format!(
                "wire schemas were registered for unknown executable probe `{probe_id}`"
            )));
        }

        Ok(Self {
            bindings,
            documents,
        })
    }

    /// Returns the compact schema state for a known probe.
    pub fn binding(&self, probe_id: &ProbeId) -> Option<&ProbeSchemaBinding> {
        self.bindings.get(probe_id)
    }

    /// Returns a complete schema document by stable schema ID.
    pub fn document(&self, schema_id: &str) -> Option<&ProbeSchemaDocument> {
        self.documents.get(schema_id)
    }

    /// Returns all bindings in deterministic catalog order.
    pub fn bindings(&self) -> impl Iterator<Item = &ProbeSchemaBinding> {
        self.bindings.values()
    }
}

#[derive(Default)]
struct ExecutableDocuments {
    input: Option<ProbeSchemaDocument>,
    output: Option<ProbeSchemaDocument>,
}

fn schema_owners(descriptors: &[ProbeDescriptor]) -> BTreeMap<&str, (&ProbeId, WireSchemaRole)> {
    let mut owners = BTreeMap::new();
    for descriptor in descriptors {
        owners.insert(
            descriptor.input_schema.as_str(),
            (&descriptor.id, WireSchemaRole::Input),
        );
        owners.insert(
            descriptor.output_schema.as_str(),
            (&descriptor.id, WireSchemaRole::Output),
        );
    }
    owners
}

fn validate_schema_version(probe_id: &ProbeId, schema_id: &str) -> DiscoveryResult<()> {
    let probe_text = probe_id.to_string();
    let probe_version = probe_text.rsplit(':').next().unwrap_or_default();
    let schema_version = schema_id.rsplit('/').next().unwrap_or_default();
    if probe_version != schema_version {
        return Err(DiscoveryError::CatalogCorruption(format!(
            "wire schema `{schema_id}` version `{schema_version}` does not match probe `{probe_id}` version `{probe_version}`"
        )));
    }
    Ok(())
}
