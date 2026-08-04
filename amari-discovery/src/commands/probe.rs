// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed handlers for discovering, dry-running, and executing bounded probes.

use std::{path::Path, time::Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::recommend::read_bounded_input;
use crate::{
    CandidatePlan, Capabilities, Catalog, CatalogIdentity, Compatibility, DiscoveryError,
    DiscoveryResult, Envelope, PlanStep, ProbeBackend, ProbeDescriptor, ProbeEngineLimits, ProbeId,
    ProbeIsolation, ProbeResult, ProbeSchemaBinding, Provenance, ReplayMetadata, ResourceLimits,
    WireSchemaRole, SCHEMA_V1,
};

const MAX_PROBE_INPUT_BYTES: u64 = 1_048_576;
const MAX_PLAN_BYTES: u64 = 2 * 1_048_576;

/// Runtime state for one declarative probe descriptor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProbeListItem {
    /// Stable probe identifier.
    pub id: ProbeId,
    /// Whether the embedded catalog recognizes this descriptor.
    pub known: bool,
    /// Whether required Cargo features are compiled.
    pub available: bool,
    /// Whether a matching adapter is registered in this binary.
    pub executable: bool,
    /// Qualification for the runtime state.
    pub reason: Option<String>,
}

/// Deterministically ordered known probe states.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProbeList {
    /// Catalog-backed probe states.
    pub probes: Vec<ProbeListItem>,
}

/// Complete declarative and runtime contract for one known probe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProbeDescription {
    /// Declarative catalog descriptor.
    pub descriptor: ProbeDescriptor,
    /// Whether the descriptor is known.
    pub known: bool,
    /// Whether its required features are compiled.
    pub available: bool,
    /// Whether executable adapter code is registered.
    pub executable: bool,
    /// Isolation used by explicit CLI execution.
    pub isolation: ProbeIsolation,
    /// Whether the supervisor enforces a wall-clock deadline.
    pub hard_timeout: bool,
    /// Whether worker crashes are isolated from the CLI process.
    pub crash_isolation: bool,
    /// Compact resolved or declared wire schema identities and hashes.
    pub schema_hashes: ProbeSchemaBinding,
    /// Qualification for the runtime state.
    pub reason: Option<String>,
}

/// Complete exported wire schema plus its canonical identity hash.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProbeSchemaResolution {
    /// Exported JSON Schema document including Amari metadata.
    pub document: Value,
    /// Lowercase SHA-256 hash of the canonical exported document bytes.
    pub hash: String,
}

/// Compatibility-only result for a probe step in a saved plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProbeDryRun {
    /// Requested registered probe.
    pub probe_id: ProbeId,
    /// Whether the plan and current catalog are compatible.
    pub compatible: bool,
    /// Whether adapter code exists in this binary.
    pub executable: bool,
    /// Dry-runs never start a worker.
    pub would_execute: bool,
    /// Isolation that explicit input execution would use.
    pub planned_isolation: ProbeIsolation,
    /// Whether explicit execution would have a hard deadline.
    pub hard_timeout: bool,
    /// Whether explicit execution would isolate crashes.
    pub crash_isolation: bool,
    /// Hash of the checked saved plan.
    pub plan_hash: String,
}

/// Isolated CLI execution result plus truthful process guarantees.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProbeRunResult {
    /// Task 2 probe result suitable for saved evidence.
    pub result: ProbeResult,
    /// Versioned input schema validated by the adapter.
    pub input_schema: String,
    /// Versioned output schema emitted by the adapter.
    pub output_schema: String,
    /// Isolation provided by this execution path.
    pub isolation: ProbeIsolation,
    /// Whether identical validated inputs produce identical mathematical output.
    pub deterministic: bool,
    /// Whether the supervisor enforced a hard deadline.
    pub hard_timeout: bool,
    /// Whether worker crashes were isolated.
    pub crash_isolation: bool,
    /// Supervisor wall-clock limit in milliseconds.
    pub timeout_millis: u64,
}

/// Returns every catalog probe with dynamically derived runtime state.
///
/// # Errors
///
/// Returns a catalog or registry validation error.
pub fn list_envelope(catalog: &Catalog) -> DiscoveryResult<Envelope<ProbeList>> {
    let states = Capabilities::current()?.known_probes;
    let probes = catalog
        .probes()
        .iter()
        .zip(states)
        .map(|(descriptor, state)| ProbeListItem {
            id: descriptor.id.clone(),
            known: state.known,
            available: state.available,
            executable: state.executable,
            reason: state.reason,
        })
        .collect();
    Ok(catalog_envelope(catalog, ProbeList { probes }))
}

/// Returns one complete catalog probe contract and dynamic runtime state.
///
/// # Errors
///
/// Returns invalid input for an unknown probe or a registry validation error.
pub fn describe_envelope(
    catalog: &Catalog,
    probe_id: &ProbeId,
) -> DiscoveryResult<Envelope<ProbeDescription>> {
    let descriptor = descriptor(catalog, probe_id)?.clone();
    let state = runtime_state(probe_id)?;
    let schema_registry = crate::probes::wire_schema_registry(catalog)?;
    let schema_hashes = schema_registry
        .binding(probe_id)
        .ok_or_else(|| {
            DiscoveryError::CatalogCorruption(format!(
                "probe `{probe_id}` has no wire schema binding"
            ))
        })?
        .clone();
    Ok(catalog_envelope(
        catalog,
        ProbeDescription {
            descriptor,
            known: state.known,
            available: state.available,
            executable: state.executable,
            isolation: ProbeIsolation::Process,
            hard_timeout: true,
            crash_isolation: true,
            schema_hashes,
            reason: state.reason,
        },
    ))
}

/// Returns the complete DTO-derived wire schema for one probe direction.
///
/// # Errors
///
/// Returns invalid input for an unknown probe or a known probe whose contract
/// is only declarative in this build, or a catalog/registry corruption error.
pub fn schema_envelope(
    catalog: &Catalog,
    probe_id: &ProbeId,
    role: WireSchemaRole,
) -> DiscoveryResult<Envelope<ProbeSchemaResolution>> {
    let descriptor = descriptor(catalog, probe_id)?;
    let schema_id = match role {
        WireSchemaRole::Input => descriptor.input_schema.as_str(),
        WireSchemaRole::Output => descriptor.output_schema.as_str(),
    };
    let schema_registry = crate::probes::wire_schema_registry(catalog)?;
    let document = schema_registry.document(schema_id).ok_or_else(|| {
        DiscoveryError::InvalidInput(format!(
            "probe `{probe_id}` has only a declared {role} wire schema in this build",
            role = role.as_str()
        ))
    })?;
    Ok(catalog_envelope(
        catalog,
        ProbeSchemaResolution {
            document: document.exported_value()?,
            hash: document.canonical_hash()?,
        },
    ))
}

/// Validates a saved plan's catalog-backed probe step without execution.
///
/// # Errors
///
/// Returns a typed file, serialization, catalog-drift, or missing-step error.
pub fn dry_run_plan_envelope(
    catalog: &Catalog,
    probe_id: &ProbeId,
    path: &Path,
) -> DiscoveryResult<Envelope<ProbeDryRun>> {
    let descriptor = descriptor(catalog, probe_id)?;
    let bytes = read_bounded_input(path, MAX_PLAN_BYTES, "probe plan")?;
    let artifact: Envelope<CandidatePlan> = serde_json::from_slice(&bytes)?;
    if artifact.schema_version != SCHEMA_V1 {
        return Err(DiscoveryError::InvalidInput(format!(
            "probe plan schema `{}` is unsupported",
            artifact.schema_version
        )));
    }
    if artifact.data.compatibility.catalog.version != catalog.version()
        || artifact.data.compatibility.catalog.hash != catalog.content_hash()
    {
        return Err(DiscoveryError::ReplayDrift {
            field: "catalog_hash".to_owned(),
            expected: artifact.data.compatibility.catalog.hash,
            actual: catalog.content_hash().to_owned(),
        });
    }
    let matching_step = artifact.data.steps.iter().any(|step| {
        matches!(step, PlanStep::Probe { capability_id, probe_id: planned }
            if planned == probe_id && capability_id == &descriptor.capability_id)
    });
    if !matching_step {
        return Err(DiscoveryError::InvalidInput(format!(
            "saved plan contains no probe step for `{probe_id}`"
        )));
    }
    let state = runtime_state(probe_id)?;
    let mut envelope = catalog_envelope(
        catalog,
        ProbeDryRun {
            probe_id: probe_id.clone(),
            compatible: true,
            executable: state.executable,
            would_execute: false,
            planned_isolation: ProbeIsolation::Process,
            hard_timeout: true,
            crash_isolation: true,
            plan_hash: artifact.data.plan_hash,
        },
    );
    envelope.provenance.project_hash = Some(artifact.data.compatibility.project_hash);
    envelope.provenance.input_hash = Some(artifact.data.compatibility.input_hash);
    envelope.provenance.replay = artifact.provenance.replay;
    Ok(envelope)
}

/// Executes explicit typed input through the fixed private process supervisor.
///
/// # Errors
///
/// Returns a typed file, input, registry, limit, worker, or protocol error.
pub fn run_input_envelope(
    catalog: &Catalog,
    probe_id: &ProbeId,
    path: &Path,
) -> DiscoveryResult<Envelope<ProbeRunResult>> {
    descriptor(catalog, probe_id)?;
    let state = runtime_state(probe_id)?;
    if !state.executable {
        return Err(DiscoveryError::ProbeUnavailable(format!(
            "probe `{probe_id}` has no executable adapter in this build"
        )));
    }
    let bytes = read_bounded_input(path, MAX_PROBE_INPUT_BYTES, "probe input")?;
    let input: Value = serde_json::from_slice(&bytes)?;
    let input_hash = hash_json(&input)?;
    let limits = ProbeEngineLimits::default();
    let provenance = standalone_provenance(catalog, input_hash.clone());
    let started = Instant::now();
    let execution = crate::probes::execute_isolated(probe_id, &input, limits, provenance.clone())?;
    let duration_micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let result = ProbeResult {
        probe_id: probe_id.clone(),
        backend: ProbeBackend::Cpu,
        duration_micros,
        resources: execution.resources.clone(),
        seed: provenance.seed,
        project_hash: provenance.project_hash.clone(),
        catalog_hash: provenance.catalog.hash.clone(),
        input_hash: input_hash.clone(),
        validated_assumptions: Vec::new(),
        refuted_assumptions: Vec::new(),
        warnings: Vec::new(),
        output: execution.output,
    };
    let defaults = ResourceLimits::default();
    let mut envelope = catalog_envelope(
        catalog,
        ProbeRunResult {
            result,
            input_schema: execution.input_schema,
            output_schema: execution.output_schema,
            isolation: execution.isolation,
            deterministic: execution.deterministic,
            hard_timeout: true,
            crash_isolation: true,
            timeout_millis: defaults.probe_timeout_millis,
        },
    );
    envelope.provenance = provenance;
    Ok(envelope)
}

fn descriptor<'a>(
    catalog: &'a Catalog,
    probe_id: &ProbeId,
) -> DiscoveryResult<&'a ProbeDescriptor> {
    catalog
        .probes()
        .iter()
        .find(|descriptor| descriptor.id == *probe_id)
        .ok_or_else(|| DiscoveryError::InvalidInput(format!("unknown probe `{probe_id}`")))
}

fn runtime_state(probe_id: &ProbeId) -> DiscoveryResult<crate::RuntimeCapabilityState> {
    Capabilities::current()?
        .known_probes
        .into_iter()
        .find(|state| state.id == probe_id.to_string())
        .ok_or_else(|| DiscoveryError::InvalidInput(format!("unknown probe `{probe_id}`")))
}

fn catalog_envelope<T>(catalog: &Catalog, data: T) -> Envelope<T> {
    Envelope::new(
        data,
        CatalogIdentity {
            version: catalog.version().to_owned(),
            hash: catalog.content_hash().to_owned(),
        },
        Compatibility {
            status: "compatible".to_owned(),
            reasons: Vec::new(),
        },
        ReplayMetadata {
            replayable: false,
            required_hashes: Vec::new(),
            reasons: vec!["catalog-only probe response".to_owned()],
        },
    )
}

fn standalone_provenance(catalog: &Catalog, input_hash: String) -> Provenance {
    Provenance {
        tool_version: env!("CARGO_PKG_VERSION").to_owned(),
        catalog: CatalogIdentity {
            version: catalog.version().to_owned(),
            hash: catalog.content_hash().to_owned(),
        },
        compatibility: Compatibility {
            status: "compatible".to_owned(),
            reasons: Vec::new(),
        },
        replay: ReplayMetadata {
            replayable: true,
            required_hashes: vec!["catalog_hash".to_owned(), "input_hash".to_owned()],
            reasons: Vec::new(),
        },
        project_hash: None,
        input_hash: Some(input_hash),
        seed: None,
    }
}

fn hash_json(value: &Value) -> DiscoveryResult<String> {
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(value)?)))
}
