// SPDX-License-Identifier: MIT OR Apache-2.0

//! Registered bounded in-process capability probes.
//!
//! The public engine executes only fixed adapters validated against the
//! embedded catalog. Limits are cooperative: adapters report and enforce
//! deterministic operation, node, iteration, and byte ceilings, but this
//! in-process API does not provide crash or wall-clock isolation.

mod cgt;
mod core;
mod dual;
mod holographic;
mod network;
mod optimization;
mod registry;
mod rewrite;
mod supervisor;
mod surreal;
mod tropical;
pub(crate) mod worker;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use cgt::{CgtNimSumOutput, CgtNimSumRequest};
pub use core::{Cl3ProductOutput, Cl3ProductRequest};
pub use dual::{PolynomialDerivativeOutput, PolynomialDerivativeRequest};
pub use holographic::{
    HolographicAttribution, HolographicCapacity, HolographicEntry, HolographicRecallOutput,
    HolographicRecallRequest, HolographicSuperpositionOutput, HolographicSuperpositionRequest,
};
pub use network::{NetworkPath, NetworkShortestPathOutput, NetworkShortestPathRequest};
pub use optimization::{ObjectiveDirection, ParetoFrontOutput, ParetoFrontRequest, ParetoPoint};
use registry::{AdapterRegistration, EffectiveProbeLimits, ProbeRegistry};
pub use rewrite::{
    RewriteExample, RewriteInferRuleOutput, RewriteInferRuleRequest, RewriteNormalizeOutput,
    RewriteNormalizeRequest, RewritePredecessorsOutput, RewritePredecessorsRequest, RewriteRule,
    RewriteTerm,
};
pub use surreal::{
    DecimalRational, DecimalSurcomplex, RationalSurcomplexDivisionOutput,
    RationalSurcomplexDivisionRequest, RationalSurrealArithmeticOutput,
    RationalSurrealArithmeticRequest,
};
pub use tropical::{TropicalViterbiOutput, TropicalViterbiRequest};

use crate::{
    Catalog, DiscoveryError, DiscoveryResult, ProbeBackend, ProbeId, ResourceObservations,
};

/// Caller-selected ceilings for cooperative in-process probe execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeEngineLimits {
    /// Maximum canonical request JSON bytes.
    pub max_input_bytes: u64,
    /// Maximum canonical result JSON bytes.
    pub max_output_bytes: u64,
    /// Maximum reported domain operations.
    pub max_operations: u64,
    /// Maximum reported domain nodes.
    pub max_nodes: u64,
    /// Maximum reported domain iterations.
    pub max_iterations: u64,
}

impl Default for ProbeEngineLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 1_048_576,
            max_output_bytes: 1_048_576,
            max_operations: 1_000_000,
            max_nodes: 1_000_000,
            max_iterations: 1_000_000,
        }
    }
}

/// Isolation guarantee attached to a probe execution result.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeIsolation {
    /// In-process execution with cooperative limits and no crash/timeout boundary.
    Cooperative,
    /// Out-of-process execution with supervisor-enforced isolation.
    Process,
}

/// Deterministic mathematical output and resource accounting from one probe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeExecution {
    /// Stable executed probe ID.
    pub probe_id: ProbeId,
    /// Versioned request schema validated by the adapter.
    pub input_schema: String,
    /// Versioned response schema emitted by the adapter.
    pub output_schema: String,
    /// Execution backend.
    pub backend: ProbeBackend,
    /// Available isolation boundary.
    pub isolation: ProbeIsolation,
    /// Whether identical validated inputs produce identical mathematical output.
    pub deterministic: bool,
    /// Cooperative resource observations.
    pub resources: ResourceObservations,
    /// Typed probe-specific result encoded as JSON.
    pub output: Value,
}

/// Public cooperative executor over the fixed private probe registry.
pub struct ProbeEngine {
    registry: ProbeRegistry,
    limits: ProbeEngineLimits,
}

impl ProbeEngine {
    /// Builds an engine with default cooperative ceilings.
    ///
    /// # Errors
    ///
    /// Returns a catalog-corruption error when a compiled adapter does not
    /// match its embedded declarative descriptor.
    pub fn new() -> DiscoveryResult<Self> {
        Self::with_limits(ProbeEngineLimits::default())
    }

    /// Builds an engine with caller-tightened cooperative ceilings.
    ///
    /// Descriptor limits always remain authoritative; caller limits can only
    /// reduce the effective ceiling.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::InvalidInput`] when any limit is zero or a
    /// catalog-corruption error for an invalid compiled adapter registry.
    pub fn with_limits(limits: ProbeEngineLimits) -> DiscoveryResult<Self> {
        if limits.max_input_bytes == 0
            || limits.max_output_bytes == 0
            || limits.max_operations == 0
            || limits.max_nodes == 0
            || limits.max_iterations == 0
        {
            return Err(DiscoveryError::InvalidInput(
                "cooperative probe limits must be greater than zero".to_owned(),
            ));
        }
        let catalog = Catalog::embedded()?;
        Self::with_catalog(&catalog, limits)
    }

    pub(crate) fn with_catalog(
        catalog: &Catalog,
        limits: ProbeEngineLimits,
    ) -> DiscoveryResult<Self> {
        if limits.max_input_bytes == 0
            || limits.max_output_bytes == 0
            || limits.max_operations == 0
            || limits.max_nodes == 0
            || limits.max_iterations == 0
        {
            return Err(DiscoveryError::InvalidInput(
                "cooperative probe limits must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            registry: ProbeRegistry::build(catalog, compiled_registrations()?)?,
            limits,
        })
    }

    /// Returns whether a known descriptor has executable code in this build.
    pub fn is_executable(&self, id: &ProbeId) -> bool {
        self.registry.is_executable(id)
    }

    /// Returns every executable probe ID in deterministic order.
    pub fn executable_probe_ids(&self) -> Vec<ProbeId> {
        self.registry.executable_ids()
    }

    /// Validates and executes one registered probe in-process.
    ///
    /// This method never invokes project code, a shell, an external executable,
    /// a provider, or the network. Isolation is explicitly cooperative.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error for an unknown descriptor, a
    /// probe-unavailable error when the adapter is not compiled, a limit error
    /// for request/result/resource ceilings, or a typed adapter failure.
    pub fn execute(&self, id: &ProbeId, input: &Value) -> DiscoveryResult<ProbeExecution> {
        if !self.registry.is_known(id) {
            return Err(DiscoveryError::InvalidInput(format!(
                "unknown probe `{id}`"
            )));
        }
        let registered = self.registry.descriptor(id).ok_or_else(|| {
            DiscoveryError::ProbeUnavailable(format!(
                "probe `{id}` has no executable adapter in this build"
            ))
        })?;
        // ProbeDescriptor v1 exposes one domain-work ceiling. Apply that
        // conservative ceiling independently to operations, nodes, and
        // iterations until the catalog protocol grows dedicated fields.
        let descriptor_work_limit = registered.limits.max_operations;
        let effective = EffectiveProbeLimits {
            max_input_bytes: registered
                .limits
                .max_input_bytes
                .min(self.limits.max_input_bytes),
            max_output_bytes: registered
                .limits
                .max_output_bytes
                .min(self.limits.max_output_bytes),
            max_operations: descriptor_work_limit.min(self.limits.max_operations),
            max_nodes: descriptor_work_limit.min(self.limits.max_nodes),
            max_iterations: descriptor_work_limit.min(self.limits.max_iterations),
        };
        let input_bytes = u64::try_from(serde_json::to_vec(input)?.len()).map_err(|_| {
            DiscoveryError::LimitExceeded("probe input byte count overflow".to_owned())
        })?;
        enforce_limit("input bytes", input_bytes, effective.max_input_bytes)?;

        let mut result = self.registry.execute(id, input, &effective)?;
        enforce_limit(
            "operations",
            result.resources.operations,
            effective.max_operations,
        )?;
        enforce_limit("nodes", result.resources.nodes, effective.max_nodes)?;
        enforce_limit(
            "iterations",
            result.resources.iterations,
            effective.max_iterations,
        )?;
        let output_bytes =
            u64::try_from(serde_json::to_vec(&result.output)?.len()).map_err(|_| {
                DiscoveryError::LimitExceeded("probe output byte count overflow".to_owned())
            })?;
        enforce_limit("output bytes", output_bytes, effective.max_output_bytes)?;
        result.resources.bytes = input_bytes.checked_add(output_bytes).ok_or_else(|| {
            DiscoveryError::LimitExceeded("probe total byte count overflow".to_owned())
        })?;

        Ok(ProbeExecution {
            probe_id: id.clone(),
            input_schema: registered.input_schema.clone(),
            output_schema: registered.output_schema.clone(),
            backend: ProbeBackend::Cpu,
            isolation: ProbeIsolation::Cooperative,
            deterministic: registered.deterministic,
            resources: result.resources,
            output: result.output,
        })
    }
}

pub(crate) fn execute_isolated(
    id: &ProbeId,
    input: &Value,
    limits: ProbeEngineLimits,
    provenance: crate::Provenance,
) -> DiscoveryResult<ProbeExecution> {
    supervisor::execute_isolated(id, input, limits, provenance)
}

fn enforce_limit(kind: &str, observed: u64, maximum: u64) -> DiscoveryResult<()> {
    if observed <= maximum {
        Ok(())
    } else {
        Err(DiscoveryError::LimitExceeded(format!(
            "probe {kind} {observed} exceeds limit {maximum}"
        )))
    }
}

#[cfg(feature = "standard-probes")]
fn compiled_registrations() -> DiscoveryResult<Vec<AdapterRegistration>> {
    Ok(vec![
        cgt::registration()?,
        core::registration()?,
        dual::registration()?,
        holographic::recall_registration()?,
        holographic::superposition_registration()?,
        network::registration()?,
        optimization::registration()?,
        rewrite::infer_rule_registration()?,
        rewrite::normalize_registration()?,
        rewrite::predecessors_registration()?,
        surreal::surcomplex_division_registration()?,
        surreal::rational_arithmetic_registration()?,
        tropical::registration()?,
    ])
}

#[cfg(not(feature = "standard-probes"))]
fn compiled_registrations() -> DiscoveryResult<Vec<AdapterRegistration>> {
    Ok(Vec::new())
}
