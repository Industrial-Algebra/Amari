// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded `amari-rewrite` normalization for replayable plans.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use amari_rewrite::trs::{Rule, Term, TermSystem};
use serde::{Deserialize, Serialize};

use super::plan::compute_plan_hash;
use crate::{
    CandidatePlan, CapabilityId, DiscoveryError, DiscoveryResult, NormalizationTrace,
    PlanNormalization, PlanStep, PlanTestTarget,
};

const MAX_ENCODED_PLAN_BYTES: usize = 1_048_576;

/// Resource limits for deterministic plan normalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizationLimits {
    /// Maximum input plan steps accepted by the normalizer.
    pub max_plan_steps: usize,
    /// Maximum successful `TermSystem::apply_once` rewrites retained in trace.
    pub max_rewrites: usize,
}

impl NormalizationLimits {
    /// Hard ceiling for plan steps accepted by normalization.
    pub const MAX_ALLOWED_PLAN_STEPS: usize = 64;
    /// Hard ceiling for retained rewrite transitions.
    pub const MAX_ALLOWED_REWRITES: usize = 4_096;

    fn validate(self) -> DiscoveryResult<()> {
        if self.max_plan_steps == 0 || self.max_rewrites == 0 {
            return Err(DiscoveryError::InvalidInput(
                "plan normalization limits must be greater than zero".to_owned(),
            ));
        }
        if self.max_plan_steps > Self::MAX_ALLOWED_PLAN_STEPS {
            return Err(DiscoveryError::LimitExceeded(format!(
                "plan normalization max_plan_steps {} exceeds hard ceiling {}",
                self.max_plan_steps,
                Self::MAX_ALLOWED_PLAN_STEPS
            )));
        }
        if self.max_rewrites > Self::MAX_ALLOWED_REWRITES {
            return Err(DiscoveryError::LimitExceeded(format!(
                "plan normalization max_rewrites {} exceeds hard ceiling {}",
                self.max_rewrites,
                Self::MAX_ALLOWED_REWRITES
            )));
        }
        Ok(())
    }
}

impl Default for NormalizationLimits {
    fn default() -> Self {
        Self {
            max_plan_steps: 64,
            max_rewrites: 4_096,
        }
    }
}

/// Deterministic bounded plan normalizer backed by `amari-rewrite`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlanNormalizer {
    limits: NormalizationLimits,
}

impl PlanNormalizer {
    /// Creates a normalizer with explicit nonzero limits.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::InvalidInput`] when either limit is zero and
    /// [`DiscoveryError::LimitExceeded`] when a fixed hard ceiling is exceeded.
    pub fn new(limits: NormalizationLimits) -> DiscoveryResult<Self> {
        limits.validate()?;
        Ok(Self { limits })
    }

    /// Canonically orders and deduplicates a candidate plan with a bounded trace.
    ///
    /// Plan steps are encoded as first-order [`Term`] values. Explicit adjacent
    /// swap rules reduce prerequisite/kind inversions, and one repeated-variable
    /// rule removes adjacent duplicates. Each transition is performed through
    /// [`TermSystem::apply_once`] and retained as a before/after trace.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::LimitExceeded`] when step, encoded-byte, or
    /// rewrite bounds are exhausted. Invalid plan structure is rejected as
    /// [`DiscoveryError::InvalidInput`].
    pub fn normalize(&self, plan: &CandidatePlan) -> DiscoveryResult<CandidatePlan> {
        validate_plan(plan, self.limits)?;
        if plan.normalization.normalized
            && is_canonical(&plan.steps, &plan.prerequisite_order)?
            && plan.plan_hash == compute_plan_hash(plan)?
        {
            return Ok(plan.clone());
        }

        let system = normalization_system(&plan.steps, &plan.prerequisite_order)?;
        let mut current = encode_plan(&plan.steps);
        let mut current_steps = plan.steps.clone();
        let mut trace = Vec::new();

        for _ in 0..self.limits.max_rewrites {
            let Some(next) = system.apply_once(&current).map_err(rewrite_error)? else {
                return finish(plan, current_steps, trace, self.limits.max_rewrites);
            };
            let next_steps = decode_plan(&next)?;
            trace.push(NormalizationTrace {
                before: current_steps,
                after: next_steps.clone(),
            });
            current = next;
            current_steps = next_steps;
        }

        if system
            .apply_once(&current)
            .map_err(rewrite_error)?
            .is_some()
        {
            Err(DiscoveryError::LimitExceeded(format!(
                "plan normalization rewrite limit {} exhausted",
                self.limits.max_rewrites
            )))
        } else {
            finish(plan, current_steps, trace, self.limits.max_rewrites)
        }
    }
}

fn validate_plan(plan: &CandidatePlan, limits: NormalizationLimits) -> DiscoveryResult<()> {
    if plan.steps.len() > limits.max_plan_steps {
        return Err(DiscoveryError::LimitExceeded(format!(
            "plan steps {} exceed limit {}",
            plan.steps.len(),
            limits.max_plan_steps
        )));
    }
    let encoded_bytes = serde_json::to_vec(&plan.steps)?.len();
    if encoded_bytes > MAX_ENCODED_PLAN_BYTES {
        return Err(DiscoveryError::LimitExceeded(format!(
            "encoded plan bytes {encoded_bytes} exceed limit {MAX_ENCODED_PLAN_BYTES}"
        )));
    }
    if plan.prerequisite_order.is_empty()
        || plan.prerequisite_order.last() != Some(&plan.capability_id)
    {
        return Err(DiscoveryError::InvalidInput(
            "plan prerequisite order must end at the candidate capability".to_owned(),
        ));
    }
    let known: BTreeSet<_> = plan.prerequisite_order.iter().cloned().collect();
    if known.len() != plan.prerequisite_order.len() {
        return Err(DiscoveryError::InvalidInput(
            "plan prerequisite order contains duplicate capability IDs".to_owned(),
        ));
    }
    if plan
        .steps
        .iter()
        .any(|step| !known.contains(step.capability_id()))
    {
        return Err(DiscoveryError::InvalidInput(
            "plan step references a capability outside prerequisite order".to_owned(),
        ));
    }
    Ok(())
}

fn normalization_system(
    steps: &[PlanStep],
    prerequisite_order: &[CapabilityId],
) -> DiscoveryResult<TermSystem> {
    let order = capability_order(prerequisite_order);
    let mut rules = vec![Rule::new(
        list(
            Term::var("duplicate"),
            list(Term::var("duplicate"), Term::var("tail")),
        ),
        list(Term::var("duplicate"), Term::var("tail")),
    )
    .map_err(rewrite_error)?];

    let unique: BTreeSet<_> = steps.iter().cloned().collect();
    for left in &unique {
        for right in &unique {
            if compare_steps(left, right, &order)? == Ordering::Greater {
                rules.push(
                    Rule::new(
                        list(
                            encode_step(left),
                            list(encode_step(right), Term::var("tail")),
                        ),
                        list(
                            encode_step(right),
                            list(encode_step(left), Term::var("tail")),
                        ),
                    )
                    .map_err(rewrite_error)?,
                );
            }
        }
    }
    Ok(TermSystem::new(rules))
}

fn finish(
    original: &CandidatePlan,
    steps: Vec<PlanStep>,
    trace: Vec<NormalizationTrace>,
    max_rewrites: usize,
) -> DiscoveryResult<CandidatePlan> {
    if !is_canonical(&steps, &original.prerequisite_order)? {
        return Err(DiscoveryError::Internal(
            "plan rewrite system stopped before canonical fixed point".to_owned(),
        ));
    }
    let mut normalized = CandidatePlan {
        capability_id: original.capability_id.clone(),
        prerequisite_order: original.prerequisite_order.clone(),
        steps,
        compatibility: original.compatibility.clone(),
        normalization: PlanNormalization {
            normalized: true,
            max_rewrites,
            trace,
        },
        plan_hash: String::new(),
    };
    normalized.plan_hash = compute_plan_hash(&normalized)?;
    Ok(normalized)
}

fn capability_order(capabilities: &[CapabilityId]) -> BTreeMap<CapabilityId, usize> {
    capabilities
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, capability)| (capability, index))
        .collect()
}

fn compare_steps(
    left: &PlanStep,
    right: &PlanStep,
    order: &BTreeMap<CapabilityId, usize>,
) -> DiscoveryResult<Ordering> {
    let left_rank = order.get(left.capability_id()).ok_or_else(|| {
        DiscoveryError::InvalidInput(format!(
            "unknown plan-step capability `{}`",
            left.capability_id()
        ))
    })?;
    let right_rank = order.get(right.capability_id()).ok_or_else(|| {
        DiscoveryError::InvalidInput(format!(
            "unknown plan-step capability `{}`",
            right.capability_id()
        ))
    })?;
    Ok(left_rank.cmp(right_rank).then_with(|| left.cmp(right)))
}

fn is_canonical(steps: &[PlanStep], capabilities: &[CapabilityId]) -> DiscoveryResult<bool> {
    let order = capability_order(capabilities);
    for pair in steps.windows(2) {
        if compare_steps(&pair[0], &pair[1], &order)? != Ordering::Less {
            return Ok(false);
        }
    }
    Ok(true)
}

fn encode_plan(steps: &[PlanStep]) -> Term {
    Term::sym("plan", [encode_list(steps)])
}

fn encode_list(steps: &[PlanStep]) -> Term {
    steps
        .iter()
        .rev()
        .fold(Term::constant("nil"), |tail, step| {
            list(encode_step(step), tail)
        })
}

fn list(head: Term, tail: Term) -> Term {
    Term::sym("step_list", [head, tail])
}

fn encode_step(step: &PlanStep) -> Term {
    match step {
        PlanStep::Dependency {
            capability_id,
            package,
            version,
        } => Term::sym(
            "dependency",
            [
                constant(capability_id.to_string()),
                constant(package),
                constant(version),
            ],
        ),
        PlanStep::Feature {
            capability_id,
            package,
            feature,
        } => Term::sym(
            "feature",
            [
                constant(capability_id.to_string()),
                constant(package),
                constant(feature),
            ],
        ),
        PlanStep::Symbol {
            capability_id,
            path,
        } => Term::sym(
            "symbol",
            [constant(capability_id.to_string()), constant(path)],
        ),
        PlanStep::Example {
            capability_id,
            package,
            example,
        } => Term::sym(
            "example",
            [
                constant(capability_id.to_string()),
                constant(package),
                constant(example),
            ],
        ),
        PlanStep::Probe {
            capability_id,
            probe_id,
        } => Term::sym(
            "probe",
            [
                constant(capability_id.to_string()),
                constant(probe_id.to_string()),
            ],
        ),
        PlanStep::Test {
            capability_id,
            package,
            target,
        } => {
            let target = match target {
                PlanTestTarget::AllTargets => "all_targets",
                PlanTestTarget::NpmPackage => "npm_package",
            };
            Term::sym(
                "test",
                [
                    constant(capability_id.to_string()),
                    constant(package),
                    constant(target),
                ],
            )
        }
    }
}

fn constant(value: impl Into<String>) -> Term {
    Term::constant(value.into())
}

fn decode_plan(term: &Term) -> DiscoveryResult<Vec<PlanStep>> {
    let Term::Sym(symbol, args) = term else {
        return invalid_encoded_plan();
    };
    if symbol.as_str() != "plan" || args.len() != 1 {
        return invalid_encoded_plan();
    }
    decode_list(&args[0])
}

fn decode_list(term: &Term) -> DiscoveryResult<Vec<PlanStep>> {
    let mut steps = Vec::new();
    let mut cursor = term;
    loop {
        let Term::Sym(symbol, args) = cursor else {
            return invalid_encoded_plan();
        };
        match (symbol.as_str(), args.as_slice()) {
            ("nil", []) => return Ok(steps),
            ("step_list", [head, tail]) => {
                steps.push(decode_step(head)?);
                cursor = tail;
            }
            _ => return invalid_encoded_plan(),
        }
    }
}

fn decode_step(term: &Term) -> DiscoveryResult<PlanStep> {
    let Term::Sym(symbol, args) = term else {
        return invalid_encoded_plan();
    };
    let strings = args
        .iter()
        .map(decode_constant)
        .collect::<DiscoveryResult<Vec<_>>>()?;
    let capability = |value: &str| {
        value.parse().map_err(|error| {
            DiscoveryError::Internal(format!("cannot decode normalized capability ID: {error}"))
        })
    };
    match (symbol.as_str(), strings.as_slice()) {
        ("dependency", [id, package, version]) => Ok(PlanStep::Dependency {
            capability_id: capability(id)?,
            package: (*package).to_owned(),
            version: (*version).to_owned(),
        }),
        ("feature", [id, package, feature]) => Ok(PlanStep::Feature {
            capability_id: capability(id)?,
            package: (*package).to_owned(),
            feature: (*feature).to_owned(),
        }),
        ("symbol", [id, path]) => Ok(PlanStep::Symbol {
            capability_id: capability(id)?,
            path: (*path).to_owned(),
        }),
        ("example", [id, package, example]) => Ok(PlanStep::Example {
            capability_id: capability(id)?,
            package: (*package).to_owned(),
            example: (*example).to_owned(),
        }),
        ("probe", [id, probe_id]) => Ok(PlanStep::Probe {
            capability_id: capability(id)?,
            probe_id: probe_id.parse().map_err(|error| {
                DiscoveryError::Internal(format!("cannot decode normalized probe ID: {error}"))
            })?,
        }),
        ("test", [id, package, target @ ("all_targets" | "npm_package")]) => {
            let target = if *target == "all_targets" {
                PlanTestTarget::AllTargets
            } else {
                PlanTestTarget::NpmPackage
            };
            Ok(PlanStep::Test {
                capability_id: capability(id)?,
                package: (*package).to_owned(),
                target,
            })
        }
        _ => invalid_encoded_plan(),
    }
}

fn decode_constant(term: &Term) -> DiscoveryResult<&str> {
    match term {
        Term::Sym(symbol, args) if args.is_empty() => Ok(symbol.as_str()),
        _ => invalid_encoded_plan(),
    }
}

fn invalid_encoded_plan<T>() -> DiscoveryResult<T> {
    Err(DiscoveryError::Internal(
        "amari-rewrite produced an invalid discovery plan term".to_owned(),
    ))
}

fn rewrite_error(error: amari_rewrite::RewriteError) -> DiscoveryError {
    DiscoveryError::Internal(format!("plan rewrite failure: {error}"))
}
