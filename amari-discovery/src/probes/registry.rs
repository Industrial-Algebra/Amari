// SPDX-License-Identifier: MIT OR Apache-2.0

//! Private descriptor-to-adapter registry validation.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::{
    CapabilityId, Catalog, DiscoveryError, DiscoveryResult, ProbeDescriptor, ProbeId, ProbeLimits,
    ResourceObservations, SideEffectPolicy,
};

pub(crate) type AdapterFn = fn(&Value, &EffectiveProbeLimits) -> DiscoveryResult<AdapterOutput>;

#[derive(Clone)]
pub(crate) struct AdapterRegistration {
    pub(crate) id: ProbeId,
    pub(crate) capability_id: CapabilityId,
    pub(crate) input_schema: String,
    pub(crate) output_schema: String,
    pub(crate) required_features: Vec<String>,
    pub(crate) limits: ProbeLimits,
    pub(crate) deterministic: bool,
    pub(crate) side_effects: SideEffectPolicy,
    pub(crate) network: bool,
    pub(crate) execute: AdapterFn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EffectiveProbeLimits {
    pub(crate) max_input_bytes: u64,
    pub(crate) max_output_bytes: u64,
    pub(crate) max_operations: u64,
    pub(crate) max_nodes: u64,
    pub(crate) max_iterations: u64,
}

pub(crate) struct AdapterOutput {
    pub(crate) resources: ResourceObservations,
    pub(crate) output: Value,
}

pub(crate) struct ProbeRegistry {
    known_ids: BTreeSet<ProbeId>,
    adapters: BTreeMap<ProbeId, RegisteredAdapter>,
}

struct RegisteredAdapter {
    descriptor: ProbeDescriptor,
    execute: AdapterFn,
}

impl ProbeRegistry {
    pub(crate) fn build(
        catalog: &Catalog,
        registrations: Vec<AdapterRegistration>,
    ) -> DiscoveryResult<Self> {
        let descriptors = catalog
            .probes()
            .iter()
            .map(|descriptor| (descriptor.id.clone(), descriptor))
            .collect::<BTreeMap<_, _>>();
        let known_ids = descriptors.keys().cloned().collect();
        let mut adapters = BTreeMap::new();
        for registration in registrations {
            if adapters.contains_key(&registration.id) {
                return Err(DiscoveryError::CatalogCorruption(format!(
                    "duplicate probe adapter `{}`",
                    registration.id
                )));
            }
            let descriptor = descriptors.get(&registration.id).ok_or_else(|| {
                DiscoveryError::CatalogCorruption(format!(
                    "probe adapter `{}` references an unknown descriptor",
                    registration.id
                ))
            })?;
            validate_registration(descriptor, &registration)?;
            adapters.insert(
                registration.id.clone(),
                RegisteredAdapter {
                    descriptor: (*descriptor).clone(),
                    execute: registration.execute,
                },
            );
        }
        Ok(Self {
            known_ids,
            adapters,
        })
    }

    pub(crate) fn is_known(&self, id: &ProbeId) -> bool {
        self.known_ids.contains(id)
    }

    pub(crate) fn is_executable(&self, id: &ProbeId) -> bool {
        self.adapters.contains_key(id)
    }

    pub(crate) fn executable_ids(&self) -> Vec<ProbeId> {
        self.adapters.keys().cloned().collect()
    }

    pub(crate) fn execute(
        &self,
        id: &ProbeId,
        input: &Value,
        limits: &EffectiveProbeLimits,
    ) -> DiscoveryResult<AdapterOutput> {
        let adapter = self.adapters.get(id).ok_or_else(|| {
            DiscoveryError::ProbeUnavailable(format!(
                "probe `{id}` has no executable adapter in this build"
            ))
        })?;
        (adapter.execute)(input, limits)
    }

    pub(crate) fn descriptor(&self, id: &ProbeId) -> Option<&ProbeDescriptor> {
        self.adapters.get(id).map(|adapter| &adapter.descriptor)
    }
}

fn validate_registration(
    descriptor: &ProbeDescriptor,
    registration: &AdapterRegistration,
) -> DiscoveryResult<()> {
    if descriptor.capability_id != registration.capability_id {
        return mismatch(&registration.id, "capability");
    }
    if descriptor.input_schema != registration.input_schema {
        return mismatch(&registration.id, "input schema");
    }
    if descriptor.output_schema != registration.output_schema {
        return mismatch(&registration.id, "output schema");
    }
    if descriptor.required_features != registration.required_features {
        return mismatch(&registration.id, "required features");
    }
    if descriptor.limits != registration.limits {
        return mismatch(&registration.id, "limits");
    }
    if descriptor.deterministic != registration.deterministic {
        return mismatch(&registration.id, "determinism");
    }
    if descriptor.side_effects != SideEffectPolicy::None
        || registration.side_effects != SideEffectPolicy::None
    {
        return mismatch(&registration.id, "side effects");
    }
    if registration.network {
        return mismatch(&registration.id, "network authority");
    }
    Ok(())
}

fn mismatch<T>(id: &ProbeId, field: &str) -> DiscoveryResult<T> {
    Err(DiscoveryError::CatalogCorruption(format!(
        "probe adapter `{id}` does not match descriptor {field}"
    )))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{Catalog, DiscoveryError};

    fn dummy_execute(
        _input: &serde_json::Value,
        _limits: &EffectiveProbeLimits,
    ) -> crate::DiscoveryResult<AdapterOutput> {
        Ok(AdapterOutput {
            resources: ResourceObservations::default(),
            output: json!({"ok": true}),
        })
    }

    fn registration() -> AdapterRegistration {
        let catalog = Catalog::embedded().unwrap();
        let descriptor = catalog
            .probes()
            .iter()
            .find(|probe| probe.id.to_string() == "amari-probe:tropical:viterbi:v1")
            .unwrap();
        AdapterRegistration {
            id: descriptor.id.clone(),
            capability_id: descriptor.capability_id.clone(),
            input_schema: descriptor.input_schema.clone(),
            output_schema: descriptor.output_schema.clone(),
            required_features: descriptor.required_features.clone(),
            limits: descriptor.limits.clone(),
            deterministic: descriptor.deterministic,
            side_effects: descriptor.side_effects,
            network: false,
            execute: dummy_execute,
        }
    }

    fn rejects(registration: AdapterRegistration, expected: &str) {
        let catalog = Catalog::embedded().unwrap();
        assert!(matches!(
            ProbeRegistry::build(&catalog, vec![registration]),
            Err(DiscoveryError::CatalogCorruption(message)) if message.contains(expected)
        ));
    }

    #[test]
    fn executable_adapter_maps_one_to_one_to_known_descriptor() {
        let catalog = Catalog::embedded().unwrap();
        let registry = ProbeRegistry::build(&catalog, vec![registration()]).unwrap();
        let id = "amari-probe:tropical:viterbi:v1".parse().unwrap();

        assert!(registry.is_executable(&id));
        assert_eq!(registry.executable_ids(), vec![id]);
    }

    #[cfg(feature = "standard-probes")]
    #[test]
    fn compiled_registry_contains_exactly_completed_probe_slices() {
        let catalog = Catalog::embedded().unwrap();
        let registry =
            ProbeRegistry::build(&catalog, super::super::compiled_registrations().unwrap())
                .unwrap();

        assert_eq!(
            registry.executable_ids(),
            vec![
                "amari-probe:cgt:nim-sum:v1".parse().unwrap(),
                "amari-probe:core:geometric-product:v1".parse().unwrap(),
                "amari-probe:dual:polynomial-derivative:v1".parse().unwrap(),
                "amari-probe:holographic:recall:v1".parse().unwrap(),
                "amari-probe:holographic:superposition:v1".parse().unwrap(),
                "amari-probe:network:shortest-path:v1".parse().unwrap(),
                "amari-probe:optimization:pareto-front:v1".parse().unwrap(),
                "amari-probe:rewrite:infer-rule:v1".parse().unwrap(),
                "amari-probe:rewrite:normalize:v1".parse().unwrap(),
                "amari-probe:rewrite:predecessors:v1".parse().unwrap(),
                "amari-probe:surcomplex:rational-division:v1"
                    .parse()
                    .unwrap(),
                "amari-probe:surreal:rational-arithmetic:v1"
                    .parse()
                    .unwrap(),
                "amari-probe:tropical:viterbi:v1".parse().unwrap(),
            ]
        );
    }

    #[test]
    fn duplicate_and_unknown_adapters_are_rejected() {
        let duplicate = registration();
        let catalog = Catalog::embedded().unwrap();
        assert!(matches!(
            ProbeRegistry::build(&catalog, vec![duplicate.clone(), duplicate]),
            Err(DiscoveryError::CatalogCorruption(message)) if message.contains("duplicate")
        ));

        let mut unknown = registration();
        unknown.id = "amari-probe:tropical:unknown:v1".parse().unwrap();
        rejects(unknown, "unknown descriptor");
    }

    #[test]
    fn capability_and_schema_mismatches_are_rejected() {
        let mut capability = registration();
        capability.capability_id = "amari:amari-core:product:geometric-product"
            .parse()
            .unwrap();
        rejects(capability, "capability");

        let mut input = registration();
        input.input_schema = "amari.discovery/probe/wrong/input/v1".to_owned();
        rejects(input, "input schema");

        let mut output = registration();
        output.output_schema = "amari.discovery/probe/wrong/output/v1".to_owned();
        rejects(output, "output schema");
    }

    #[test]
    fn side_effect_and_network_authority_are_rejected() {
        let mut side_effects = registration();
        side_effects.side_effects = SideEffectPolicy::ReadOnly;
        rejects(side_effects, "side effects");

        let mut network = registration();
        network.network = true;
        rejects(network, "network");
    }

    #[test]
    fn limit_determinism_and_feature_mismatches_are_rejected() {
        let mut limits = registration();
        limits.limits.max_operations -= 1;
        rejects(limits, "limits");

        let mut determinism = registration();
        determinism.deterministic = false;
        rejects(determinism, "determinism");

        let mut features = registration();
        features.required_features = vec!["ai".to_owned()];
        rejects(features, "features");
    }
}
