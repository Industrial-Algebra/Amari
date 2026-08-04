// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery::{
    Catalog, CgtNimSumOutput, CgtNimSumRequest, Cl3ProductOutput, Cl3ProductRequest,
    HolographicRecallOutput, HolographicRecallRequest, HolographicSuperpositionOutput,
    HolographicSuperpositionRequest, NetworkShortestPathOutput, NetworkShortestPathRequest,
    ParetoFrontOutput, ParetoFrontRequest, PolynomialDerivativeOutput, PolynomialDerivativeRequest,
    ProbeEngine, ProbeId, ProbeSchemaContractState, ProbeSchemaDocument, ProbeSchemaRegistration,
    ProbeWireSchemaRegistry, RationalSurcomplexDivisionOutput, RationalSurcomplexDivisionRequest,
    RationalSurrealArithmeticOutput, RationalSurrealArithmeticRequest, RewriteInferRuleOutput,
    RewriteInferRuleRequest, RewriteNormalizeOutput, RewriteNormalizeRequest,
    RewritePredecessorsOutput, RewritePredecessorsRequest, TropicalViterbiOutput,
    TropicalViterbiRequest, WireCompatibility, WireSchemaRole, SCHEMA_V1,
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

#[cfg(feature = "standard-probes")]
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

#[test]
fn rational_holographic_contracts() {
    let catalog = Catalog::embedded().unwrap();

    let rational_input =
        ProbeSchemaDocument::from_contract::<RationalSurrealArithmeticRequest>().unwrap();
    let rational_output =
        ProbeSchemaDocument::from_contract::<RationalSurrealArithmeticOutput>().unwrap();
    assert_registered_pair(
        &catalog,
        "amari-probe:surreal:rational-arithmetic:v1",
        rational_input.clone(),
        rational_output.clone(),
    );
    assert_eq!(
        constraint_ids(&rational_input),
        [
            "decimal_length_limit",
            "decimal_strings_are_i128",
            "nonzero_denominators",
            "nonzero_rhs"
        ]
    );
    assert_eq!(
        constraint_ids(&rational_output),
        ["nonzero_denominators", "normalized_exact_rationals"]
    );
    let rational_schema = rational_input.exported_value().unwrap();
    assert_eq!(rational_schema["additionalProperties"], false);
    assert!(rational_schema["properties"]["lhs"].is_object());
    assert!(rational_schema["properties"]["rhs"].is_object());

    let surcomplex_input =
        ProbeSchemaDocument::from_contract::<RationalSurcomplexDivisionRequest>().unwrap();
    let surcomplex_output =
        ProbeSchemaDocument::from_contract::<RationalSurcomplexDivisionOutput>().unwrap();
    assert_registered_pair(
        &catalog,
        "amari-probe:surcomplex:rational-division:v1",
        surcomplex_input.clone(),
        surcomplex_output.clone(),
    );
    assert_eq!(
        constraint_ids(&surcomplex_input),
        [
            "decimal_length_limit",
            "decimal_strings_are_i128",
            "nonzero_denominators",
            "nonzero_divisor"
        ]
    );
    assert_eq!(
        constraint_ids(&surcomplex_output),
        ["nonzero_denominators", "normalized_exact_rationals"]
    );
    let surcomplex_schema = surcomplex_input.exported_value().unwrap();
    assert_eq!(surcomplex_schema["additionalProperties"], false);
    assert!(surcomplex_schema["properties"]["dividend"].is_object());
    assert!(surcomplex_schema["properties"]["divisor"].is_object());

    let superposition_input =
        ProbeSchemaDocument::from_contract::<HolographicSuperpositionRequest>().unwrap();
    let superposition_output =
        ProbeSchemaDocument::from_contract::<HolographicSuperpositionOutput>().unwrap();
    assert_registered_pair(
        &catalog,
        "amari-probe:holographic:superposition:v1",
        superposition_input.clone(),
        superposition_output.clone(),
    );
    assert_eq!(
        constraint_ids(&superposition_input),
        ["integer_seeds", "nonempty_seeds", "seed_count_limit"]
    );
    assert_eq!(
        constraint_ids(&superposition_output),
        ["finite_coefficients", "map256_dimension"]
    );
    assert_eq!(
        superposition_input.exported_value().unwrap()["additionalProperties"],
        false
    );

    let recall_input = ProbeSchemaDocument::from_contract::<HolographicRecallRequest>().unwrap();
    let recall_output = ProbeSchemaDocument::from_contract::<HolographicRecallOutput>().unwrap();
    assert_registered_pair(
        &catalog,
        "amari-probe:holographic:recall:v1",
        recall_input.clone(),
        recall_output.clone(),
    );
    assert_eq!(
        constraint_ids(&recall_input),
        ["entry_count_limit", "integer_seeds", "nonempty_entries"]
    );
    assert_eq!(
        constraint_ids(&recall_output),
        [
            "bounded_warnings",
            "capacity_metrics_consistent",
            "finite_metrics",
            "map256_dimension",
            "nonnegative_attribution_weights"
        ]
    );
    let recall_schema = recall_input.exported_value().unwrap();
    assert_eq!(recall_schema["additionalProperties"], false);
    assert!(recall_schema["properties"]["entries"].is_object());
    assert!(recall_schema["properties"]["query_seed"].is_object());
}

#[test]
fn rewrite_contracts() {
    let catalog = Catalog::embedded().unwrap();

    let normalize_input = ProbeSchemaDocument::from_contract::<RewriteNormalizeRequest>().unwrap();
    let normalize_output = ProbeSchemaDocument::from_contract::<RewriteNormalizeOutput>().unwrap();
    assert_registered_pair(
        &catalog,
        "amari-probe:rewrite:normalize:v1",
        normalize_input.clone(),
        normalize_output.clone(),
    );
    assert_eq!(
        constraint_ids(&normalize_input),
        [
            "max_steps_limit",
            "max_steps_positive",
            "rules_checked",
            "rules_count_limit",
            "rules_non_expanding",
            "term_bounds",
            "term_name_bytes_limit"
        ]
    );
    assert_eq!(
        constraint_ids(&normalize_output),
        [
            "normal_form_within_term_bounds",
            "steps_within_request_limit"
        ]
    );
    let normalize_schema = normalize_input.exported_value().unwrap();
    assert_eq!(normalize_schema["additionalProperties"], false);
    assert_recursive_term_schema_is_internally_tagged_and_strict(&normalize_schema);
    assert!(normalize_schema["properties"]["rules"].is_object());
    assert!(normalize_schema["properties"]["max_steps"].is_object());

    let infer_input = ProbeSchemaDocument::from_contract::<RewriteInferRuleRequest>().unwrap();
    let infer_output = ProbeSchemaDocument::from_contract::<RewriteInferRuleOutput>().unwrap();
    assert_registered_pair(
        &catalog,
        "amari-probe:rewrite:infer-rule:v1",
        infer_input.clone(),
        infer_output.clone(),
    );
    assert_eq!(
        constraint_ids(&infer_input),
        [
            "example_count_limit",
            "examples_nonempty",
            "term_bounds",
            "term_name_bytes_limit"
        ]
    );
    assert_eq!(
        constraint_ids(&infer_output),
        [
            "rhs_no_duplicate_variables",
            "rhs_variables_subset_lhs",
            "rule_within_term_bounds"
        ]
    );
    let infer_schema = infer_input.exported_value().unwrap();
    assert_eq!(infer_schema["additionalProperties"], false);
    assert_recursive_term_schema_is_internally_tagged_and_strict(&infer_schema);

    let predecessors_input =
        ProbeSchemaDocument::from_contract::<RewritePredecessorsRequest>().unwrap();
    let predecessors_output =
        ProbeSchemaDocument::from_contract::<RewritePredecessorsOutput>().unwrap();
    assert_registered_pair(
        &catalog,
        "amari-probe:rewrite:predecessors:v1",
        predecessors_input.clone(),
        predecessors_output.clone(),
    );
    assert_eq!(
        constraint_ids(&predecessors_input),
        [
            "max_depth_limit",
            "max_frontier_limit",
            "max_frontier_positive",
            "max_results_limit",
            "max_results_positive",
            "reverse_lhs_no_duplicate_variables",
            "rules_checked",
            "rules_count_limit",
            "term_bounds",
            "term_name_bytes_limit"
        ]
    );
    assert_eq!(
        constraint_ids(&predecessors_output),
        [
            "predecessors_canonical_order",
            "predecessors_within_result_limit",
            "truncation_truthful"
        ]
    );
    let predecessors_schema = predecessors_input.exported_value().unwrap();
    assert_eq!(predecessors_schema["additionalProperties"], false);
    assert_recursive_term_schema_is_internally_tagged_and_strict(&predecessors_schema);
    assert!(predecessors_schema["properties"]["max_depth"].is_object());
    assert!(predecessors_schema["properties"]["max_results"].is_object());
    assert!(predecessors_schema["properties"]["max_frontier"].is_object());
}

fn assert_recursive_term_schema_is_internally_tagged_and_strict(schema: &serde_json::Value) {
    let variants = schema["$defs"]["RewriteTerm"]["oneOf"]
        .as_array()
        .expect("recursive RewriteTerm schema must be an internally tagged oneOf");
    assert_eq!(variants.len(), 2);
    for variant in variants {
        assert_eq!(variant["additionalProperties"], false);
        assert!(variant["properties"]["kind"].is_object());
    }
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

#[cfg(feature = "standard-probes")]
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

#[cfg(feature = "standard-probes")]
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

#[cfg(feature = "standard-probes")]
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

#[cfg(feature = "standard-probes")]
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
