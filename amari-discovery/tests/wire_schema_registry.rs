// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery::{
    Catalog, ProbeEngine, ProbeId, ProbeSchemaContractState, ProbeSchemaDocument,
    ProbeSchemaRegistration, ProbeWireSchemaRegistry, WireCompatibility, WireSchemaRole, SCHEMA_V1,
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
