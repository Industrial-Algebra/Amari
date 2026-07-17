// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery::{
    CandidatePlan, CapabilityId, Catalog, DiscoveryError, GoalSpec, GraphPath, NormalizationLimits,
    PlanGenerator, PlanNormalization, PlanNormalizer, PlanStep, PlanTestTarget, PlanningContext,
    ProjectKind, ProjectSnapshot, RankedCandidate, RankingComponents, SnapshotState,
};

fn id(value: &str) -> CapabilityId {
    value.parse().unwrap()
}

fn snapshot(project_hash: &str) -> ProjectSnapshot {
    ProjectSnapshot {
        project_hash: project_hash.to_owned(),
        project_kind: ProjectKind::RustCargo,
        signals: Vec::new(),
        cargo: None,
        rust: None,
        platform: None,
        npm: None,
        typescript: None,
        file_count: 0,
        total_bytes: 0,
        state: SnapshotState::Complete,
        warnings: Vec::new(),
        files: Vec::new(),
    }
}

fn context(project_hash: &str) -> PlanningContext {
    PlanningContext {
        snapshot: snapshot(project_hash),
        goal: GoalSpec {
            statement: "differentiate a scalar polynomial".to_owned(),
            constraints: vec!["offline".to_owned(), "cpu-only".to_owned()],
        },
        probe_results: Vec::new(),
    }
}

fn ranked(capabilities: &[CapabilityId]) -> RankedCandidate {
    let target = capabilities.last().unwrap().clone();
    RankedCandidate {
        capability_id: target.clone(),
        path: GraphPath {
            target,
            source_seed: capabilities[0].clone(),
            capabilities: capabilities.to_vec(),
            steps: Vec::new(),
            total_cost: 0.0,
        },
        components: RankingComponents {
            applicability: 1.0,
            evidence: 0.8,
            effort: 0.2,
            maturity: 1.0,
            runtime: 0.2,
            platform: 1.0,
            verification: 0.5,
            risk: 0.1,
        },
        objectives: [0.0, 0.2, 0.2, 0.0, 0.2, 0.0, 0.5, 0.1],
        confidence: 0.8,
        evidence: Vec::new(),
        validated_assumptions: Vec::new(),
    }
}

fn generated_dual_plan() -> CandidatePlan {
    let catalog = Catalog::embedded().unwrap();
    let target = id("amari:amari-dual:autodiff:forward-derivative");
    PlanGenerator::default()
        .generate(&catalog, &context("project-plan"), &ranked(&[target]))
        .unwrap()
}

fn pending_with_steps(plan: &CandidatePlan, steps: Vec<PlanStep>) -> CandidatePlan {
    CandidatePlan {
        steps,
        plan_hash: String::new(),
        normalization: PlanNormalization::default(),
        ..plan.clone()
    }
}

#[test]
fn generated_plan_contains_exact_catalog_actions_and_replay_hashes() {
    let catalog = Catalog::embedded().unwrap();
    let target = id("amari:amari-dual:autodiff:forward-derivative");
    let plan = PlanGenerator::default()
        .generate(
            &catalog,
            &context("project-exact"),
            &ranked(std::slice::from_ref(&target)),
        )
        .unwrap();
    let version = catalog
        .crates()
        .iter()
        .find(|record| record.name == "amari-dual")
        .unwrap()
        .version
        .clone();

    assert_eq!(plan.capability_id, target.clone());
    assert_eq!(plan.prerequisite_order, vec![target.clone()]);
    assert!(plan.steps.contains(&PlanStep::Dependency {
        capability_id: target.clone(),
        package: "amari-dual".to_owned(),
        version,
    }));
    assert!(plan.steps.contains(&PlanStep::Feature {
        capability_id: target.clone(),
        package: "amari-dual".to_owned(),
        feature: "std".to_owned(),
    }));
    assert!(plan.steps.contains(&PlanStep::Symbol {
        capability_id: target.clone(),
        path: "amari_dual::DualNumber::derivative".to_owned(),
    }));
    assert!(plan.steps.contains(&PlanStep::Example {
        capability_id: target.clone(),
        package: "amari-dual".to_owned(),
        example: "gradient_seeding".to_owned(),
    }));
    assert!(plan.steps.contains(&PlanStep::Probe {
        capability_id: target.clone(),
        probe_id: "amari-probe:dual:polynomial-derivative:v1".parse().unwrap(),
    }));
    assert!(plan.steps.contains(&PlanStep::Test {
        capability_id: target,
        package: "amari-dual".to_owned(),
        target: PlanTestTarget::AllTargets,
    }));
    assert_eq!(plan.compatibility.catalog.version, catalog.version());
    assert_eq!(plan.compatibility.catalog.hash, catalog.content_hash());
    assert_eq!(plan.compatibility.project_hash, "project-exact");
    assert_eq!(plan.compatibility.input_hash.len(), 64);
    assert_eq!(plan.plan_hash.len(), 64);
    assert!(plan.normalization.normalized);
}

#[test]
fn duplicate_steps_normalize_once_with_a_bounded_trace() {
    let generated = generated_dual_plan();
    let dependency = generated
        .steps
        .iter()
        .find(|step| matches!(step, PlanStep::Dependency { .. }))
        .unwrap()
        .clone();
    let feature = generated
        .steps
        .iter()
        .find(|step| matches!(step, PlanStep::Feature { .. }))
        .unwrap()
        .clone();
    let test = generated
        .steps
        .iter()
        .find(|step| matches!(step, PlanStep::Test { .. }))
        .unwrap()
        .clone();
    let raw = pending_with_steps(
        &generated,
        vec![
            test.clone(),
            feature.clone(),
            dependency.clone(),
            feature.clone(),
            dependency.clone(),
        ],
    );
    let normalizer = PlanNormalizer::new(NormalizationLimits {
        max_plan_steps: 16,
        max_rewrites: 8,
    })
    .unwrap();

    let normalized = normalizer.normalize(&raw).unwrap();

    assert_eq!(normalized.steps, vec![dependency, feature, test]);
    assert!(normalized.normalization.normalized);
    assert!(!normalized.normalization.trace.is_empty());
    assert!(normalized.normalization.trace.len() <= 8);
    assert!(normalized
        .normalization
        .trace
        .iter()
        .all(|rewrite| rewrite.before != rewrite.after));
    assert_eq!(normalizer.normalize(&normalized).unwrap(), normalized);
}

#[test]
fn prerequisite_capabilities_have_canonical_step_order() {
    let catalog = Catalog::embedded().unwrap();
    let prerequisite = id("amari:amari-dual:autodiff:forward-derivative");
    let target = id("amari:amari-network:paths:geometric-shortest-path");
    let plan = PlanGenerator::default()
        .generate(
            &catalog,
            &context("project-order"),
            &ranked(&[prerequisite.clone(), target.clone()]),
        )
        .unwrap();

    assert_eq!(
        plan.prerequisite_order,
        vec![prerequisite.clone(), target.clone()]
    );
    let last_prerequisite = plan
        .steps
        .iter()
        .rposition(|step| step.capability_id() == &prerequisite)
        .unwrap();
    let first_target = plan
        .steps
        .iter()
        .position(|step| step.capability_id() == &target)
        .unwrap();
    assert!(last_prerequisite < first_target);
}

#[test]
fn normalization_rejects_step_and_rewrite_limit_exhaustion() {
    let generated = generated_dual_plan();
    let mut reversed = generated.steps.clone();
    reversed.reverse();
    let raw = pending_with_steps(&generated, reversed);

    let step_limited = PlanNormalizer::new(NormalizationLimits {
        max_plan_steps: generated.steps.len() - 1,
        max_rewrites: 128,
    })
    .unwrap();
    assert!(matches!(
        step_limited.normalize(&raw),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("plan steps")
    ));

    let rewrite_limited = PlanNormalizer::new(NormalizationLimits {
        max_plan_steps: 64,
        max_rewrites: 1,
    })
    .unwrap();
    assert!(matches!(
        rewrite_limited.normalize(&raw),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("rewrite")
    ));
}

#[test]
fn replay_rejects_project_catalog_and_input_drift_with_typed_errors() {
    let plan = generated_dual_plan();

    for (field, replacement) in [
        ("project_hash", "different-project"),
        ("catalog_hash", "different-catalog"),
        ("input_hash", "different-input"),
    ] {
        let mut actual = plan.compatibility.clone();
        match field {
            "project_hash" => actual.project_hash = replacement.to_owned(),
            "catalog_hash" => actual.catalog.hash = replacement.to_owned(),
            "input_hash" => actual.input_hash = replacement.to_owned(),
            _ => unreachable!(),
        }
        assert!(matches!(
            plan.verify_replay(&actual),
            Err(DiscoveryError::ReplayDrift { field: drifted, .. }) if drifted == field
        ));
    }

    assert!(plan.verify_replay(&plan.compatibility).is_ok());
}

#[test]
fn generator_threads_explicit_normalization_limits() {
    let catalog = Catalog::embedded().unwrap();
    let target = id("amari:amari-dual:autodiff:forward-derivative");
    let generator = PlanGenerator::new(NormalizationLimits {
        max_plan_steps: 1,
        max_rewrites: 8,
    })
    .unwrap();

    assert!(matches!(
        generator.generate(
            &catalog,
            &context("project-generator-limit"),
            &ranked(&[target]),
        ),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("plan steps")
    ));
}

#[test]
fn generation_and_normalization_are_byte_deterministic() {
    let first = generated_dual_plan();
    let second = generated_dual_plan();

    assert_eq!(first, second);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
}
