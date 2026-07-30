// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery::{
    Catalog, CgtNimSumOutput, CgtNimSumRequest, Cl3ProductOutput, Cl3ProductRequest,
    NetworkShortestPathOutput, NetworkShortestPathRequest, ParetoFrontOutput, ParetoFrontRequest,
    PolynomialDerivativeOutput, PolynomialDerivativeRequest, ProbeEngine, ProbeId,
    ProbeSchemaContractState, ProbeSchemaDocument, ProbeSchemaRegistration,
    ProbeWireSchemaRegistry, TropicalViterbiOutput, TropicalViterbiRequest, WireCompatibility,
    WireSchemaRole, SCHEMA_V1,
};

fn synthetic_document(schema_id: &str, role: WireSchemaRole) -> ProbeSchemaDocument {
    ProbeSchemaDocument::new(
        schema_id,
        role,
        SCHEMA_V1,
        serde_json::json!({"type": "object"}),
        Vec::new(),
        Vec::new(),
        WireCompatibility::AdditivePatch,
    )
    .expect("synthetic schema document must validate")
}

fn executable_ids() -> Vec<ProbeId> {
    ProbeEngine::new().unwrap().executable_probe_ids()
}

fn registrations(catalog: &Catalog, executable_ids: &[ProbeId]) -> Vec<ProbeSchemaRegistration> {
    catalog
        .probes()
        .iter()
        .filter(|descriptor| executable_ids.contains(&descriptor.id))
        .flat_map(|descriptor| {
            [
                ProbeSchemaRegistration::new(
                    descriptor.id.clone(),
                    synthetic_document(&descriptor.input_schema, WireSchemaRole::Input),
                ),
                ProbeSchemaRegistration::new(
                    descriptor.id.clone(),
                    synthetic_document(&descriptor.output_schema, WireSchemaRole::Output),
                ),
            ]
        })
        .collect()
}

#[test]
fn cgt_core_dual_contracts() {
    let catalog = Catalog::embedded().unwrap();

    let cgt_input = ProbeSchemaDocument::from_contract::<CgtNimSumRequest>().unwrap();
    let cgt_output = ProbeSchemaDocument::from_contract::<CgtNimSumOutput>().unwrap();
    assert_registered_pair(
        &catalog,
        "amari-probe:cgt:nim-sum:v1",
        cgt_input.clone(),
        cgt_output.clone(),
    );
    assert_eq!(cgt_input.id(), "amari.discovery/probe/cgt-nim-sum/input/v1");
    assert_eq!(
        cgt_output.id(),
        "amari.discovery/probe/cgt-nim-sum/output/v1"
    );
    assert_eq!(
        constraint_ids(&cgt_input),
        ["heap_count_limit", "heap_value_limit"]
    );
    assert_eq!(
        constraint_ids(&cgt_output),
        ["grundy_values_align_with_heaps", "nim_sum_is_xor"]
    );
    assert_eq!(
        cgt_input.exported_value().unwrap()["additionalProperties"],
        false
    );
    assert!(cgt_input.exported_value().unwrap()["properties"]["heaps"].is_object());

    let core_input = ProbeSchemaDocument::from_contract::<Cl3ProductRequest>().unwrap();
    let core_output = ProbeSchemaDocument::from_contract::<Cl3ProductOutput>().unwrap();
    assert_registered_pair(
        &catalog,
        "amari-probe:core:geometric-product:v1",
        core_input.clone(),
        core_output.clone(),
    );
    assert_eq!(
        constraint_ids(&core_input),
        ["finite_numbers", "fixed_coefficient_length"]
    );
    assert_eq!(
        constraint_ids(&core_output),
        ["finite_numbers", "fixed_coefficient_length"]
    );
    let core_schema = core_input.exported_value().unwrap();
    assert_eq!(core_schema["required"][0], "left");
    assert_eq!(core_schema["required"][1], "right");
    assert_eq!(core_schema["properties"]["left"]["minItems"], 8);
    assert_eq!(core_schema["properties"]["left"]["maxItems"], 8);
    assert_eq!(core_schema["properties"]["right"]["minItems"], 8);

    let dual_input = ProbeSchemaDocument::from_contract::<PolynomialDerivativeRequest>().unwrap();
    let dual_output = ProbeSchemaDocument::from_contract::<PolynomialDerivativeOutput>().unwrap();
    assert_registered_pair(
        &catalog,
        "amari-probe:dual:polynomial-derivative:v1",
        dual_input.clone(),
        dual_output.clone(),
    );
    assert_eq!(
        constraint_ids(&dual_input),
        [
            "coefficient_count_limit",
            "finite_numbers",
            "nonempty_coefficients"
        ]
    );
    assert_eq!(constraint_ids(&dual_output), ["finite_numbers"]);
    assert_eq!(
        dual_input.exported_value().unwrap()["additionalProperties"],
        false
    );
    assert!(dual_input.exported_value().unwrap()["properties"]["coefficients"].is_object());
    assert!(dual_input.exported_value().unwrap()["properties"]["at"].is_object());
}

#[test]
fn structured_contracts() {
    let catalog = Catalog::embedded().unwrap();

    let network_input = ProbeSchemaDocument::from_contract::<NetworkShortestPathRequest>().unwrap();
    let network_output = ProbeSchemaDocument::from_contract::<NetworkShortestPathOutput>().unwrap();
    assert_registered_pair(
        &catalog,
        "amari-probe:network:shortest-path:v1",
        network_input.clone(),
        network_output.clone(),
    );
    assert_eq!(
        constraint_ids(&network_input),
        [
            "adjacency_node_limit",
            "adjacency_nonempty",
            "adjacency_square",
            "endpoint_indices_in_bounds",
            "finite_nonnegative_weights"
        ]
    );
    assert_eq!(
        constraint_ids(&network_output),
        [
            "finite_total_weight",
            "optional_path_shape",
            "path_nodes_within_node_count"
        ]
    );
    let network_schema = network_input.exported_value().unwrap();
    assert_eq!(network_schema["additionalProperties"], false);
    assert!(network_schema["properties"]["adjacency"].is_object());
    assert!(network_schema["properties"]["source"].is_object());
    assert!(network_schema["properties"]["target"].is_object());

    let pareto_input = ProbeSchemaDocument::from_contract::<ParetoFrontRequest>().unwrap();
    let pareto_output = ProbeSchemaDocument::from_contract::<ParetoFrontOutput>().unwrap();
    assert_registered_pair(
        &catalog,
        "amari-probe:optimization:pareto-front:v1",
        pareto_input.clone(),
        pareto_output.clone(),
    );
    assert_eq!(
        constraint_ids(&pareto_input),
        [
            "dimension_limit",
            "finite_objectives",
            "nonempty_directions",
            "nonempty_population",
            "population_limit",
            "rectangular_objectives"
        ]
    );
    assert_eq!(
        constraint_ids(&pareto_output),
        [
            "solution_indices_match_request",
            "solution_objectives_match_request",
            "solutions_are_nondominated"
        ]
    );
    let pareto_schema = pareto_input.exported_value().unwrap();
    assert_eq!(pareto_schema["additionalProperties"], false);
    assert!(pareto_schema["properties"]["objectives"].is_object());
    assert!(pareto_schema["properties"]["directions"].is_object());

    let tropical_input = ProbeSchemaDocument::from_contract::<TropicalViterbiRequest>().unwrap();
    let tropical_output = ProbeSchemaDocument::from_contract::<TropicalViterbiOutput>().unwrap();
    assert_registered_pair(
        &catalog,
        "amari-probe:tropical:viterbi:v1",
        tropical_input.clone(),
        tropical_output.clone(),
    );
    assert_eq!(
        constraint_ids(&tropical_input),
        [
            "emission_rows_match_states",
            "emission_width_limit",
            "finite_weights",
            "nonempty_observations",
            "observation_count_limit",
            "observation_indices_in_bounds",
            "square_transitions",
            "state_limit",
            "states_nonempty"
        ]
    );
    assert_eq!(
        constraint_ids(&tropical_output),
        [
            "finite_score",
            "path_length_matches_observations",
            "path_states_within_state_count"
        ]
    );
    let tropical_schema = tropical_input.exported_value().unwrap();
    assert_eq!(tropical_schema["additionalProperties"], false);
    assert!(tropical_schema["properties"]["transitions"].is_object());
    assert!(tropical_schema["properties"]["emissions"].is_object());
    assert!(tropical_schema["properties"]["observations"].is_object());
}

fn assert_registered_pair(
    catalog: &Catalog,
    probe_id: &str,
    input: ProbeSchemaDocument,
    output: ProbeSchemaDocument,
) {
    let probe_id: ProbeId = probe_id.parse().unwrap();
    assert_eq!(input.role(), WireSchemaRole::Input);
    assert_eq!(output.role(), WireSchemaRole::Output);
    assert_eq!(
        input.canonical_hash().unwrap(),
        input.canonical_hash().unwrap()
    );
    assert_eq!(
        output.canonical_hash().unwrap(),
        output.canonical_hash().unwrap()
    );
    assert_eq!(
        input.summary().unwrap().hash(),
        input.canonical_hash().unwrap()
    );
    assert_eq!(
        output.summary().unwrap().hash(),
        output.canonical_hash().unwrap()
    );
    assert_eq!(input.compatibility(), WireCompatibility::AdditivePatch);
    assert_eq!(output.compatibility(), WireCompatibility::AdditivePatch);

    let registry = ProbeWireSchemaRegistry::build(
        catalog,
        [probe_id.clone()],
        [
            ProbeSchemaRegistration::new(probe_id.clone(), input),
            ProbeSchemaRegistration::new(probe_id.clone(), output),
        ],
    )
    .unwrap();
    let binding = registry.binding(&probe_id).unwrap();
    assert_eq!(binding.state(), ProbeSchemaContractState::Resolved);
}

fn constraint_ids(document: &ProbeSchemaDocument) -> Vec<String> {
    document.exported_value().unwrap()["x-amari-semantic-constraints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|constraint| constraint["id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn registry_resolves_every_executable_probe_and_declares_non_executable() {
    let catalog = Catalog::embedded().unwrap();
    let executable_ids = executable_ids();
    assert_eq!(executable_ids.len(), 13);
    let registry = ProbeWireSchemaRegistry::build(
        &catalog,
        executable_ids.iter().cloned(),
        registrations(&catalog, &executable_ids),
    )
    .unwrap();

    for probe_id in &executable_ids {
        let binding = registry.binding(probe_id).expect("executable binding");
        assert_eq!(binding.state(), ProbeSchemaContractState::Resolved);
        let input = binding.input_summary().expect("input summary");
        let output = binding.output_summary().expect("output summary");
        assert_eq!(input.role(), WireSchemaRole::Input);
        assert_eq!(output.role(), WireSchemaRole::Output);
        assert_eq!(input.compatibility(), WireCompatibility::AdditivePatch);
        assert_eq!(input.hash().len(), 64);
        assert_eq!(output.hash().len(), 64);
        assert!(registry.document(input.id()).is_some());
        assert!(registry.document(output.id()).is_some());
    }

    let declared_id: ProbeId = "amari-probe:tropical:shortest-path:v1".parse().unwrap();
    let declared = registry
        .binding(&declared_id)
        .expect("known declared probe");
    assert_eq!(declared.state(), ProbeSchemaContractState::Declared);
    assert!(declared.input_summary().is_none());
    assert!(declared.output_summary().is_none());
    let descriptor = catalog
        .probes()
        .iter()
        .find(|probe| probe.id == declared_id)
        .unwrap();
    assert!(registry.document(&descriptor.input_schema).is_none());
}

#[test]
fn duplicate_schema_id_is_catalog_corruption() {
    let catalog = Catalog::embedded().unwrap();
    let executable_ids = executable_ids();
    let mut registrations = registrations(&catalog, &executable_ids);
    let duplicate = registrations[0].clone();
    registrations.push(duplicate);

    let error =
        ProbeWireSchemaRegistry::build(&catalog, executable_ids.iter().cloned(), registrations)
            .expect_err("duplicate schema IDs must be rejected");
    assert_eq!(error.kind(), "catalog_corruption");
}

#[test]
fn role_and_version_mismatches_are_catalog_corruption() {
    let catalog = Catalog::embedded().unwrap();
    let executable_ids = executable_ids();
    let probe_id = executable_ids[0].clone();
    let descriptor = catalog
        .probes()
        .iter()
        .find(|probe| probe.id == probe_id)
        .unwrap();

    let wrong_role: ProbeSchemaDocument = serde_json::from_value(serde_json::json!({
        "id": descriptor.output_schema,
        "role": "input",
        "protocol_version": SCHEMA_V1,
        "structural_schema": {"type": "object"},
        "semantic_constraints": [],
        "examples": [],
        "compatibility": "additive_patch"
    }))
    .unwrap();
    let error = ProbeWireSchemaRegistry::build(
        &catalog,
        [probe_id.clone()],
        vec![ProbeSchemaRegistration::new(probe_id.clone(), wrong_role)],
    )
    .expect_err("document role must match descriptor direction");
    assert_eq!(error.kind(), "catalog_corruption");

    let wrong_version: ProbeSchemaDocument = serde_json::from_value(serde_json::json!({
        "id": descriptor.input_schema.replace("/v1", "/v2"),
        "role": "input",
        "protocol_version": SCHEMA_V1,
        "structural_schema": {"type": "object"},
        "semantic_constraints": [],
        "examples": [],
        "compatibility": "additive_patch"
    }))
    .unwrap();
    let error = ProbeWireSchemaRegistry::build(
        &catalog,
        [probe_id.clone()],
        vec![ProbeSchemaRegistration::new(probe_id, wrong_version)],
    )
    .expect_err("document version must match owning probe version");
    assert_eq!(error.kind(), "catalog_corruption");
}

#[test]
fn registration_probe_must_own_the_document_descriptor() {
    let catalog = Catalog::embedded().unwrap();
    let executable_ids = executable_ids();
    let wrong_probe: ProbeId = "amari-probe:dual:polynomial-derivative:v1".parse().unwrap();
    let cgt = catalog
        .probes()
        .iter()
        .find(|probe| probe.id.to_string() == "amari-probe:cgt:nim-sum:v1")
        .unwrap();
    let document = synthetic_document(&cgt.input_schema, WireSchemaRole::Input);

    let error = ProbeWireSchemaRegistry::build(
        &catalog,
        executable_ids.iter().cloned(),
        vec![ProbeSchemaRegistration::new(wrong_probe, document)],
    )
    .expect_err("adapter/registration disagreement must be rejected");
    assert_eq!(error.kind(), "catalog_corruption");
}

#[test]
fn executable_probe_requires_exactly_one_input_and_one_output() {
    let catalog = Catalog::embedded().unwrap();
    let executable_ids = executable_ids();
    let incomplete = vec![registrations(&catalog, &executable_ids)[0].clone()];

    let error =
        ProbeWireSchemaRegistry::build(&catalog, executable_ids.iter().cloned(), incomplete)
            .expect_err("missing output contract must be rejected");
    assert_eq!(error.kind(), "catalog_corruption");
}
