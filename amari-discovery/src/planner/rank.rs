// SPDX-License-Identifier: MIT OR Apache-2.0

//! Transparent multi-objective ranking of expanded capability paths.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use amari_optimization::multiobjective::{Individual, ParetoFront};
use serde::{Deserialize, Serialize};

use crate::{
    CapabilityId, Catalog, CostHint, DiscoveryError, DiscoveryResult, Evidence, GraphExpansion,
    GraphExpansionState, GraphPath, ProbeId, ProbeResult, ProjectKind, ProjectSnapshot,
    RetrievedCandidate, SnapshotState, StabilityTier,
};

/// Ordered names of the canonical all-minimization objective vector.
pub const RANKING_OBJECTIVE_ORDER: [&str; 8] = [
    "one_minus_applicability",
    "one_minus_evidence",
    "effort",
    "one_minus_maturity",
    "runtime",
    "one_minus_platform",
    "one_minus_verification",
    "risk",
];

/// Transparent normalized ranking dimensions for one capability.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RankingComponents {
    /// How well project constraints and prerequisites permit the capability.
    pub applicability: f64,
    /// Strength of recall, project, and probe evidence.
    pub evidence: f64,
    /// Relative integration effort; lower is better.
    pub effort: f64,
    /// API stability and maturity; higher is better.
    pub maturity: f64,
    /// Expected runtime cost; lower is better.
    pub runtime: f64,
    /// Compatibility with the inspected project platform; higher is better.
    pub platform: f64,
    /// Strength of matching bounded verification; higher is better.
    pub verification: f64,
    /// Uncertainty and integration risk; lower is better.
    pub risk: f64,
}

impl RankingComponents {
    /// Returns the canonical eight-dimensional all-minimization vector.
    ///
    /// Benefit dimensions are negated as `1 - value`; cost dimensions are
    /// retained. Values are quantized to twelve decimal places so equivalent
    /// calculations serialize identically.
    pub fn minimization_objectives(&self) -> [f64; 8] {
        [
            quantize(1.0 - self.applicability),
            quantize(1.0 - self.evidence),
            quantize(self.effort),
            quantize(1.0 - self.maturity),
            quantize(self.runtime),
            quantize(1.0 - self.platform),
            quantize(1.0 - self.verification),
            quantize(self.risk),
        ]
    }
}

/// A typed project signal that adjusts one candidate's ranking components.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankingSignalKind {
    /// Evidence that the capability applies to the project goal.
    Applicability,
    /// Additional typed project evidence.
    Evidence,
    /// Explicit platform compatibility evidence.
    Platform,
    /// Additional bounded verification evidence.
    Verification,
    /// Additional uncertainty or risk.
    Risk,
}

/// Candidate-scoped ranking evidence supplied by a planner adapter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RankingSignal {
    /// Capability affected by this signal.
    pub capability_id: CapabilityId,
    /// Ranking dimension affected by the signal.
    pub kind: RankingSignalKind,
    /// Normalized strength in the inclusive range `[0.0, 1.0]`.
    pub strength: f64,
    /// Concise typed evidence summary.
    pub summary: String,
}

/// Additional deterministic context used while ranking graph candidates.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RankingContext {
    /// Candidates whose prerequisites are known to be satisfied.
    pub prerequisites_satisfied: BTreeSet<CapabilityId>,
    /// Candidates whose prerequisites are known to be impossible or refuted.
    pub prerequisites_blocked: BTreeSet<CapabilityId>,
    /// Candidate-scoped typed ranking signals.
    pub signals: Vec<RankingSignal>,
    /// Saved bounded probe results considered as verification evidence.
    pub probe_results: Vec<ProbeResult>,
}

/// A ranked, unblocked capability path with transparent evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RankedCandidate {
    /// Stable catalog capability ID.
    pub capability_id: CapabilityId,
    /// Lowest-cost graph path supporting this candidate.
    pub path: GraphPath,
    /// Human-facing normalized component values.
    pub components: RankingComponents,
    /// Canonical all-minimization objective vector used by Pareto optimization.
    pub objectives: [f64; 8],
    /// Monotone confidence summary derived from benefit/risk dimensions.
    pub confidence: f64,
    /// Typed evidence contributing to this score.
    pub evidence: Vec<Evidence>,
    /// Validated assumptions retained for later plan generation.
    pub validated_assumptions: Vec<String>,
}

/// A graph candidate excluded by an explicit blocker or refuted probe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BlockedCandidate {
    /// Stable catalog capability ID.
    pub capability_id: CapabilityId,
    /// Deterministically sorted blocking reasons.
    pub reasons: Vec<String>,
}

/// Provenance for deterministic Pareto and preferred-candidate selection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RankingProvenance {
    /// Optimization implementation used to retain non-dominated alternatives.
    pub optimizer: String,
    /// Canonical objective order.
    pub objective_order: Vec<String>,
    /// Whether every objective is minimized.
    pub all_minimization: bool,
    /// Stable preferred-candidate tie-break order.
    pub preferred_tie_break: Vec<String>,
    /// Deterministic fallback used if the optimizer produced no front.
    pub deterministic_fallback: Option<String>,
}

/// Complete transparent ranking result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RankingResult {
    /// Deterministically preferred member of the Pareto front.
    pub preferred: Option<RankedCandidate>,
    /// Non-dominated alternatives, preferred candidate first.
    pub pareto: Vec<RankedCandidate>,
    /// Unblocked candidates dominated by the Pareto front.
    pub dominated: Vec<RankedCandidate>,
    /// Candidates excluded by explicit blockers.
    pub blocked: Vec<BlockedCandidate>,
    /// Non-fatal provenance mismatches and ignored evidence.
    pub warnings: Vec<String>,
    /// Optimizer and deterministic fallback provenance.
    pub provenance: RankingProvenance,
}

/// Deterministic transparent Pareto ranker.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CandidateRanker {
    _private: (),
}

impl CandidateRanker {
    /// Ranks expanded graph paths and preserves their Pareto alternatives.
    ///
    /// Probe results contribute only when their probe ID is registered and
    /// both catalog and project provenance match the supplied inputs.
    /// Refuted assumptions block only the capability owned by that probe.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::InvalidInput`] for unknown IDs or malformed
    /// signal strengths, and [`DiscoveryError::Internal`] if adaptation to the
    /// Pareto front violates an internal index invariant.
    pub fn rank(
        &self,
        catalog: &Catalog,
        graph: &GraphExpansion,
        recalled: &[RetrievedCandidate],
        snapshot: &ProjectSnapshot,
        context: &RankingContext,
    ) -> DiscoveryResult<RankingResult> {
        let capability_records: BTreeMap<_, _> = catalog
            .capabilities()
            .iter()
            .map(|capability| (capability.id.clone(), capability))
            .collect();
        validate_context(context, &capability_records)?;

        let recalled_by_id = recalled_candidates(recalled, &capability_records)?;
        let signals_by_id = ranking_signals(context);
        let (probe_effects, mut warnings) =
            probe_effects(catalog, snapshot, &context.probe_results);
        let graph_candidate_ids: BTreeSet<_> = graph
            .paths
            .iter()
            .map(|path| path.target.clone())
            .chain(graph.blocked_capabilities.iter().cloned())
            .collect();
        let mut blocked =
            initial_blocked(graph, context, &capability_records, &graph_candidate_ids)?;
        for (capability_id, effect) in &probe_effects {
            if graph_candidate_ids.contains(capability_id) && !effect.refuted_assumptions.is_empty()
            {
                blocked.entry(capability_id.clone()).or_default().extend(
                    effect
                        .refuted_assumptions
                        .iter()
                        .map(|assumption| format!("probe_refuted:{assumption}")),
                );
            }
        }

        let graph_is_partial = matches!(graph.state, GraphExpansionState::Partial { .. });
        let mut candidates = Vec::new();
        for path in &graph.paths {
            validate_graph_path(path)?;
            let capability = capability_records.get(&path.target).ok_or_else(|| {
                DiscoveryError::InvalidInput(format!(
                    "graph capability `{}` is absent from the catalog",
                    path.target
                ))
            })?;
            if blocked.contains_key(&path.target) {
                continue;
            }
            let candidate = score_candidate(
                capability,
                path,
                ScoreInputs {
                    recalled: recalled_by_id.get(&path.target).copied(),
                    signals: signals_by_id
                        .get(&path.target)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                    probe: probe_effects.get(&path.target),
                    snapshot,
                    prerequisites_satisfied: context.prerequisites_satisfied.contains(&path.target),
                    graph_is_partial,
                },
            );
            candidates.push(candidate);
        }
        candidates.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));

        let (mut pareto, mut dominated, deterministic_fallback) = pareto_partition(candidates)?;
        pareto.sort_by(preferred_order);
        dominated.sort_by(preferred_order);
        let preferred = pareto.first().cloned();

        let mut blocked: Vec<_> = blocked
            .into_iter()
            .map(|(capability_id, reasons)| {
                let mut reasons: Vec<_> = reasons.into_iter().collect();
                reasons.sort();
                reasons.dedup();
                BlockedCandidate {
                    capability_id,
                    reasons,
                }
            })
            .collect();
        blocked.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        warnings.sort();
        warnings.dedup();

        Ok(RankingResult {
            preferred,
            pareto,
            dominated,
            blocked,
            warnings,
            provenance: RankingProvenance {
                optimizer: "amari_optimization::multiobjective::ParetoFront".to_owned(),
                objective_order: RANKING_OBJECTIVE_ORDER
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
                all_minimization: true,
                preferred_tie_break: vec![
                    "confidence_desc".to_owned(),
                    "effort_asc".to_owned(),
                    "runtime_asc".to_owned(),
                    "risk_asc".to_owned(),
                    "capability_id_asc".to_owned(),
                ],
                deterministic_fallback,
            },
        })
    }
}

#[derive(Clone, Debug, Default)]
struct ProbeEffect {
    verification: f64,
    evidence: f64,
    validated_assumptions: BTreeSet<String>,
    refuted_assumptions: BTreeSet<String>,
}

struct ScoreInputs<'a> {
    recalled: Option<&'a RetrievedCandidate>,
    signals: &'a [&'a RankingSignal],
    probe: Option<&'a ProbeEffect>,
    snapshot: &'a ProjectSnapshot,
    prerequisites_satisfied: bool,
    graph_is_partial: bool,
}

fn score_candidate(
    capability: &crate::CapabilityRecord,
    path: &GraphPath,
    inputs: ScoreInputs<'_>,
) -> RankedCandidate {
    let ScoreInputs {
        recalled,
        signals,
        probe,
        snapshot,
        prerequisites_satisfied,
        graph_is_partial,
    } = inputs;
    let mut evidence_items = Vec::new();
    let mut applicability = if path.steps.is_empty() { 0.7 } else { 0.6 };
    if prerequisites_satisfied {
        applicability = boost(applicability, 0.75);
        evidence_items.push(Evidence {
            kind: "prerequisites_satisfied".to_owned(),
            summary: "candidate prerequisites are satisfied".to_owned(),
            source: Some(capability.id.to_string()),
            weight: 0.75,
        });
    }

    let mut evidence = recalled
        .map(|candidate| clamp01(candidate.retrieval_score))
        .unwrap_or(0.15);
    if let Some(candidate) = recalled {
        evidence_items.push(Evidence {
            kind: "candidate_recall".to_owned(),
            summary: format!("{} matched recall tokens", candidate.matched_evidence.len()),
            source: Some(capability.id.to_string()),
            weight: clamp01(candidate.retrieval_score),
        });
    }

    let mut platform = match snapshot.project_kind {
        ProjectKind::Unknown => 0.5,
        ProjectKind::RustCargo | ProjectKind::NpmTypeScript => 0.8,
        ProjectKind::Mixed => 0.9,
    };
    if matches!(snapshot.state, SnapshotState::LimitExceeded { .. }) {
        platform = clamp01(platform - 0.1);
    }
    let mut verification = if capability.probe_refs.is_empty() {
        0.1
    } else {
        0.25
    };
    let mut risk = match capability.stability {
        StabilityTier::Stable => 0.15,
        StabilityTier::Experimental => 0.35,
        StabilityTier::Research => 0.6,
    };
    if graph_is_partial {
        risk = boost(risk, 0.2);
    }
    if recalled.is_none() {
        risk = boost(risk, 0.15);
    }

    for signal in signals {
        match signal.kind {
            RankingSignalKind::Applicability => {
                applicability = boost(applicability, signal.strength)
            }
            RankingSignalKind::Evidence => evidence = boost(evidence, signal.strength),
            RankingSignalKind::Platform => platform = boost(platform, signal.strength),
            RankingSignalKind::Verification => verification = boost(verification, signal.strength),
            RankingSignalKind::Risk => risk = boost(risk, signal.strength),
        }
        evidence_items.push(Evidence {
            kind: format!("ranking_signal:{:?}", signal.kind).to_ascii_lowercase(),
            summary: signal.summary.clone(),
            source: Some(signal.capability_id.to_string()),
            weight: signal.strength,
        });
    }

    let mut validated_assumptions = Vec::new();
    if let Some(probe) = probe {
        verification = verification.max(probe.verification);
        evidence = boost(evidence, probe.evidence);
        validated_assumptions.extend(probe.validated_assumptions.iter().cloned());
        evidence_items.push(Evidence {
            kind: "matching_probe".to_owned(),
            summary: "registered probe provenance matched".to_owned(),
            source: Some(capability.id.to_string()),
            weight: probe.verification,
        });
    }

    let maturity = match capability.stability {
        StabilityTier::Stable => 1.0,
        StabilityTier::Experimental => 0.65,
        StabilityTier::Research => 0.35,
    };
    let runtime = match capability.cost {
        CostHint::Low => 0.2,
        CostHint::Moderate => 0.5,
        CostHint::High => 0.8,
    };
    let effort = clamp01(path.total_cost / (path.total_cost + 2.0));
    let components = RankingComponents {
        applicability: quantize(applicability),
        evidence: quantize(evidence),
        effort: quantize(effort),
        maturity: quantize(maturity),
        runtime: quantize(runtime),
        platform: quantize(platform),
        verification: quantize(verification),
        risk: quantize(risk),
    };
    let confidence = quantize(
        (components.applicability
            + components.evidence
            + components.maturity
            + components.platform
            + components.verification
            + (1.0 - components.risk))
            / 6.0,
    );
    let objectives = components.minimization_objectives();
    evidence_items.sort_by(|left, right| {
        (&left.kind, &left.summary, &left.source).cmp(&(&right.kind, &right.summary, &right.source))
    });

    RankedCandidate {
        capability_id: capability.id.clone(),
        path: path.clone(),
        components,
        objectives,
        confidence,
        evidence: evidence_items,
        validated_assumptions,
    }
}

fn pareto_partition(
    candidates: Vec<RankedCandidate>,
) -> DiscoveryResult<(Vec<RankedCandidate>, Vec<RankedCandidate>, Option<String>)> {
    if candidates.is_empty() {
        return Ok((Vec::new(), Vec::new(), None));
    }

    let mut front = ParetoFront::<f64>::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let mut individual = Individual::new(vec![index as f64]);
        individual.objectives = candidate.objectives.to_vec();
        front.add_if_non_dominated(individual);
    }
    let front_indices: BTreeSet<_> = front
        .solutions
        .iter()
        .map(|individual| individual.variables[0] as usize)
        .collect();

    if front_indices.is_empty() {
        let mut fallback = candidates;
        fallback.sort_by(preferred_order);
        let preferred = fallback.remove(0);
        return Ok((
            vec![preferred],
            fallback,
            Some("pareto_front_empty_preferred_tie_break".to_owned()),
        ));
    }
    if front_indices.iter().any(|index| *index >= candidates.len()) {
        return Err(DiscoveryError::Internal(
            "Pareto front returned an invalid candidate index".to_owned(),
        ));
    }

    let mut pareto = Vec::new();
    let mut dominated = Vec::new();
    for (index, candidate) in candidates.into_iter().enumerate() {
        if front_indices.contains(&index) {
            pareto.push(candidate);
        } else {
            dominated.push(candidate);
        }
    }
    Ok((pareto, dominated, None))
}

fn preferred_order(left: &RankedCandidate, right: &RankedCandidate) -> Ordering {
    right
        .confidence
        .total_cmp(&left.confidence)
        .then_with(|| left.components.effort.total_cmp(&right.components.effort))
        .then_with(|| left.components.runtime.total_cmp(&right.components.runtime))
        .then_with(|| left.components.risk.total_cmp(&right.components.risk))
        .then_with(|| left.capability_id.cmp(&right.capability_id))
}

fn validate_graph_path(path: &GraphPath) -> DiscoveryResult<()> {
    if !path.total_cost.is_finite() || path.total_cost < 0.0 {
        return Err(DiscoveryError::InvalidInput(format!(
            "graph path for `{}` requires a finite nonnegative total cost",
            path.target
        )));
    }
    if path
        .steps
        .iter()
        .any(|step| !step.cost.is_finite() || step.cost < 0.0)
    {
        return Err(DiscoveryError::InvalidInput(format!(
            "graph path for `{}` contains an invalid step cost",
            path.target
        )));
    }
    Ok(())
}

fn validate_context(
    context: &RankingContext,
    capabilities: &BTreeMap<CapabilityId, &crate::CapabilityRecord>,
) -> DiscoveryResult<()> {
    for id in context
        .prerequisites_satisfied
        .iter()
        .chain(&context.prerequisites_blocked)
    {
        validate_capability_id(id, capabilities)?;
    }
    for signal in &context.signals {
        validate_capability_id(&signal.capability_id, capabilities)?;
        if !signal.strength.is_finite() || !(0.0..=1.0).contains(&signal.strength) {
            return Err(DiscoveryError::InvalidInput(format!(
                "ranking signal for `{}` requires finite strength in [0, 1]",
                signal.capability_id
            )));
        }
    }
    Ok(())
}

fn validate_capability_id(
    id: &CapabilityId,
    capabilities: &BTreeMap<CapabilityId, &crate::CapabilityRecord>,
) -> DiscoveryResult<()> {
    if !capabilities.contains_key(id) {
        return Err(DiscoveryError::InvalidInput(format!(
            "ranking capability `{id}` is absent from the catalog"
        )));
    }
    Ok(())
}

fn recalled_candidates<'a>(
    recalled: &'a [RetrievedCandidate],
    capabilities: &BTreeMap<CapabilityId, &crate::CapabilityRecord>,
) -> DiscoveryResult<BTreeMap<CapabilityId, &'a RetrievedCandidate>> {
    let mut by_id = BTreeMap::<CapabilityId, &RetrievedCandidate>::new();
    for candidate in recalled {
        validate_capability_id(&candidate.capability_id, capabilities)?;
        if !candidate.retrieval_score.is_finite() {
            return Err(DiscoveryError::InvalidInput(format!(
                "recalled candidate `{}` has a non-finite score",
                candidate.capability_id
            )));
        }
        match by_id.get(&candidate.capability_id) {
            Some(existing)
                if existing
                    .retrieval_score
                    .total_cmp(&candidate.retrieval_score)
                    != Ordering::Less => {}
            _ => {
                by_id.insert(candidate.capability_id.clone(), candidate);
            }
        }
    }
    Ok(by_id)
}

fn ranking_signals(context: &RankingContext) -> BTreeMap<CapabilityId, Vec<&RankingSignal>> {
    let mut by_id = BTreeMap::<CapabilityId, Vec<&RankingSignal>>::new();
    for signal in &context.signals {
        by_id
            .entry(signal.capability_id.clone())
            .or_default()
            .push(signal);
    }
    for signals in by_id.values_mut() {
        signals.sort_by(|left, right| {
            (
                signal_kind_rank(left.kind),
                left.summary.as_str(),
                left.strength.to_bits(),
            )
                .cmp(&(
                    signal_kind_rank(right.kind),
                    right.summary.as_str(),
                    right.strength.to_bits(),
                ))
        });
    }
    by_id
}

fn signal_kind_rank(kind: RankingSignalKind) -> u8 {
    match kind {
        RankingSignalKind::Applicability => 0,
        RankingSignalKind::Evidence => 1,
        RankingSignalKind::Platform => 2,
        RankingSignalKind::Verification => 3,
        RankingSignalKind::Risk => 4,
    }
}

fn probe_effects(
    catalog: &Catalog,
    snapshot: &ProjectSnapshot,
    probe_results: &[ProbeResult],
) -> (BTreeMap<CapabilityId, ProbeEffect>, Vec<String>) {
    let descriptors: BTreeMap<ProbeId, CapabilityId> = catalog
        .probes()
        .iter()
        .map(|probe| (probe.id.clone(), probe.capability_id.clone()))
        .collect();
    let mut ordered: Vec<_> = probe_results.iter().collect();
    ordered.sort_by(|left, right| {
        (&left.probe_id, &left.input_hash).cmp(&(&right.probe_id, &right.input_hash))
    });
    let mut effects = BTreeMap::<CapabilityId, ProbeEffect>::new();
    let mut warnings = Vec::new();

    for result in ordered {
        let Some(capability_id) = descriptors.get(&result.probe_id) else {
            warnings.push(format!("ignored_probe:{}:unknown_probe", result.probe_id));
            continue;
        };
        if result.catalog_hash != catalog.content_hash() {
            warnings.push(format!(
                "ignored_probe:{}:catalog_hash_mismatch",
                result.probe_id
            ));
            continue;
        }
        if result.project_hash.as_deref() != Some(snapshot.project_hash.as_str()) {
            warnings.push(format!(
                "ignored_probe:{}:project_hash_mismatch",
                result.probe_id
            ));
            continue;
        }

        let effect = effects.entry(capability_id.clone()).or_default();
        effect.verification = effect
            .verification
            .max(if result.validated_assumptions.is_empty() {
                0.75
            } else {
                0.9
            });
        effect.evidence = effect
            .evidence
            .max(if result.validated_assumptions.is_empty() {
                0.1
            } else {
                0.25
            });
        effect.validated_assumptions.extend(
            result
                .validated_assumptions
                .iter()
                .map(|assumption| sanitize_assumption(assumption)),
        );
        effect.refuted_assumptions.extend(
            result.refuted_assumptions.iter().map(|assumption| {
                format!("{}:{}", result.probe_id, sanitize_assumption(assumption))
            }),
        );
    }
    (effects, warnings)
}

fn initial_blocked(
    graph: &GraphExpansion,
    context: &RankingContext,
    capabilities: &BTreeMap<CapabilityId, &crate::CapabilityRecord>,
    graph_candidate_ids: &BTreeSet<CapabilityId>,
) -> DiscoveryResult<BTreeMap<CapabilityId, BTreeSet<String>>> {
    let mut blocked = BTreeMap::<CapabilityId, BTreeSet<String>>::new();
    for id in &graph.blocked_capabilities {
        validate_capability_id(id, capabilities)?;
        blocked
            .entry(id.clone())
            .or_default()
            .insert("graph_constraint".to_owned());
    }
    for id in &context.prerequisites_blocked {
        if graph_candidate_ids.contains(id) {
            blocked
                .entry(id.clone())
                .or_default()
                .insert("prerequisites_blocked".to_owned());
        }
    }
    Ok(blocked)
}

fn sanitize_assumption(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let bounded: String = normalized.chars().take(128).collect();
    if bounded.is_empty() {
        "unspecified".to_owned()
    } else {
        bounded
    }
}

fn boost(current: f64, strength: f64) -> f64 {
    clamp01(current + (1.0 - current) * clamp01(strength))
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn quantize(value: f64) -> f64 {
    (clamp01(value) * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}
