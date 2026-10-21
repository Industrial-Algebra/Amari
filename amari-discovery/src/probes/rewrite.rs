// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded DTO validation shared by registered rewrite probes.

#[cfg(feature = "standard-probes")]
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[cfg(feature = "standard-probes")]
use amari_rewrite::{
    analysis::{
        critical_pairs, local_confluence, prove_terminating, PairJoinability, Precedence,
        TerminationProof,
    },
    completion::{
        complete, guided_predecessors, prepare_guidance, CandidateRejection, CompletionConfig,
        CompletionOutcome, CompletionTrace, FailedReason, GuidedSearchOutcome, InconclusiveReason,
        PartialReason, TraceStep,
    },
    inverse::{system_hash, InverseSearchConfig},
    language::{
        classify_and_query, classify_system, finite_horizon_lower_bound, finite_horizon_preimage,
        one_step_lower_bound, one_step_preimage, saturation_lower_bound, saturation_preimage,
        ApproximationEvent, ApproximationOutcome, MembershipVerdict, PreimageCapability,
        PreimageCertificate, PreimageConstruction, PreimageOperation, PreimageOutcome,
        RankedSymbol, RegularTreeGrammar, TreeAutomaton, TreeAutomatonLimits, TreeState,
        TreeTransition, TrsClass,
    },
    relation::{RelationLimits, RelationResources, Sha256Digest},
    synthesis::{
        infer_rule, specialize_rules, synthesize_inverse, InverseClause, InverseEvidence,
        InverseRefutation, InverseSynthesisConfig, InverseSynthesisOutcome, InverseUnknownReason,
        SpecializationConfig, SpecializationOutcome,
    },
    trs::{match_pattern, Rule, Symbol, Term, TermSystem},
    RewriteError,
};
use serde::{Deserialize, Serialize};
#[cfg(feature = "standard-probes")]
use serde_json::Value;

#[cfg(feature = "standard-probes")]
use super::registry::{AdapterOutput, AdapterRegistration, EffectiveProbeLimits};
#[cfg(feature = "standard-probes")]
use crate::{DiscoveryError, DiscoveryResult, ProbeLimits, ResourceObservations, SideEffectPolicy};

#[cfg(feature = "standard-probes")]
const MAX_NAME_BYTES: usize = 256;
#[cfg(feature = "standard-probes")]
const MAX_TERM_DEPTH: u64 = 64;
#[cfg(feature = "standard-probes")]
const MAX_TERM_NODES: u64 = 4_096;
#[cfg(feature = "standard-probes")]
const MAX_RULES: u64 = 256;
#[cfg(feature = "standard-probes")]
const MAX_NORMALIZATION_STEPS: u64 = 4_096;
#[cfg(feature = "standard-probes")]
const MAX_PREDECESSOR_DEPTH: u64 = 16;
#[cfg(feature = "standard-probes")]
const MAX_PREDECESSOR_RESULTS: u64 = 1_024;
#[cfg(feature = "standard-probes")]
const MAX_PREDECESSOR_FRONTIER: u64 = 1_024;
#[cfg(feature = "standard-probes")]
const MAX_INFERENCE_EXAMPLES: u64 = 256;

/// Serializable first-order term accepted by rewrite probes.
#[derive(
    Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RewriteTerm {
    /// Pattern variable.
    Variable {
        /// Variable name.
        name: String,
    },
    /// Function symbol or constant when `arguments` is empty.
    Symbol {
        /// Function or constant name.
        name: String,
        /// Ordered child terms.
        arguments: Vec<RewriteTerm>,
    },
}

#[cfg(feature = "standard-probes")]
impl RewriteTerm {
    fn to_term(&self) -> DiscoveryResult<Term> {
        match self {
            Self::Variable { name } => {
                validate_name(name)?;
                Ok(Term::var(name.clone()))
            }
            Self::Symbol { name, arguments } => {
                validate_name(name)?;
                let arguments = arguments
                    .iter()
                    .map(Self::to_term)
                    .collect::<DiscoveryResult<Vec<_>>>()?;
                Ok(Term::sym(name.clone(), arguments))
            }
        }
    }

    fn from_term(term: &Term) -> Self {
        match term {
            Term::Var(variable) => Self::Variable {
                name: variable.as_str().to_owned(),
            },
            Term::Sym(symbol, arguments) => Self::Symbol {
                name: symbol.as_str().to_owned(),
                arguments: arguments.iter().map(Self::from_term).collect(),
            },
        }
    }
}

/// Serializable checked first-order rewrite rule.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteRule {
    /// Left-hand side pattern.
    pub lhs: RewriteTerm,
    /// Right-hand side template.
    pub rhs: RewriteTerm,
}

#[cfg(feature = "standard-probes")]
impl RewriteRule {
    fn to_rule(&self) -> DiscoveryResult<Rule> {
        Rule::new(self.lhs.to_term()?, self.rhs.to_term()?).map_err(|_| {
            DiscoveryError::InvalidInput(
                "rewrite rule RHS variable does not occur in its LHS".to_owned(),
            )
        })
    }

    fn from_rule(rule: &Rule) -> Self {
        Self {
            lhs: RewriteTerm::from_term(rule.lhs()),
            rhs: RewriteTerm::from_term(rule.rhs()),
        }
    }
}

/// Typed input for bounded ordered term normalization.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-normalize/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        max_steps_limit = "max_steps is no greater than 4096",
        max_steps_positive = "max_steps is greater than zero",
        rules_checked = "every rule RHS variable occurs in its LHS",
        rules_count_limit = "at most 256 ordered rules are accepted",
        rules_non_expanding = "normalization rejects rules that expand forward term size",
        term_bounds = "terms have depth at most 64 and at most 4096 nodes",
        term_name_bytes_limit = "variable and symbol names contain at most 256 bytes"
    ),
    example(
        label = "identity_step",
        value = "{\"term\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},\"rules\":[{\"lhs\":{\"kind\":\"variable\",\"name\":\"x\"},\"rhs\":{\"kind\":\"variable\",\"name\":\"x\"}}],\"max_steps\":1}"
    )
)]
pub struct RewriteNormalizeRequest {
    /// Initial first-order term.
    pub term: RewriteTerm,
    /// Ordered checked rewrite rules.
    pub rules: Vec<RewriteRule>,
    /// Maximum successful rewrite steps.
    pub max_steps: u64,
}

/// Typed fixed-point result from bounded term normalization.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-normalize/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        normal_form_within_term_bounds = "the normal form has depth at most 64 and at most 4096 nodes",
        steps_within_request_limit = "steps never exceeds the requested max_steps bound"
    ),
    example(
        label = "identity_step",
        value = "{\"normal_form\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},\"steps\":0}"
    )
)]
pub struct RewriteNormalizeOutput {
    /// Reached normal form.
    pub normal_form: RewriteTerm,
    /// Number of successful rewrite steps.
    pub steps: u64,
}

/// Typed input for bounded inverse-rewrite predecessor search.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-predecessors/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        max_depth_limit = "max_depth is no greater than 16",
        max_frontier_limit = "max_frontier is no greater than 1024",
        max_frontier_positive = "max_frontier is greater than zero",
        max_results_limit = "max_results is no greater than 1024",
        max_results_positive = "max_results is greater than zero",
        reverse_lhs_no_duplicate_variables = "reverse search rejects rules whose LHS duplicates variable occurrences",
        rules_checked = "every rule RHS variable occurs in its LHS",
        rules_count_limit = "at most 256 ordered rules are accepted",
        term_bounds = "terms have depth at most 64 and at most 4096 nodes",
        term_name_bytes_limit = "variable and symbol names contain at most 256 bytes"
    ),
    example(
        label = "one_reverse_step",
        value = "{\"target\":{\"kind\":\"symbol\",\"name\":\"b\",\"arguments\":[]},\"rules\":[{\"lhs\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},\"rhs\":{\"kind\":\"symbol\",\"name\":\"b\",\"arguments\":[]}}],\"max_depth\":1,\"max_results\":8,\"max_frontier\":8}"
    )
)]
pub struct RewritePredecessorsRequest {
    /// Target term whose predecessors are requested.
    pub target: RewriteTerm,
    /// Ordered checked forward rules explored in reverse.
    pub rules: Vec<RewriteRule>,
    /// Maximum backward-search depth.
    pub max_depth: u64,
    /// Maximum returned predecessor terms.
    pub max_results: u64,
    /// Maximum queued search terms at one time.
    pub max_frontier: u64,
}

/// Deterministic bounded predecessor-search result.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-predecessors/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        predecessors_canonical_order = "predecessors are unique and canonically ordered by the wire DTO",
        predecessors_within_result_limit = "the predecessor count never exceeds max_results",
        truncation_truthful = "truncated is true exactly when max_results omitted at least one discovered predecessor"
    ),
    example(
        label = "one_reverse_step",
        value = "{\"predecessors\":[{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]}],\"truncated\":false}"
    )
)]
pub struct RewritePredecessorsOutput {
    /// Unique predecessor terms in canonical DTO order.
    pub predecessors: Vec<RewriteTerm>,
    /// Whether the requested result cap omitted at least one predecessor.
    pub truncated: bool,
}

/// One positive before/after rewrite example.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteExample {
    /// Concrete term before rewriting.
    pub before: RewriteTerm,
    /// Concrete term after rewriting.
    pub after: RewriteTerm,
}

/// Typed input for bounded single-rule inference.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-infer-rule/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        example_count_limit = "at most 256 positive rewrite examples are accepted",
        examples_nonempty = "at least one positive rewrite example is required",
        term_bounds = "terms have depth at most 64 and at most 4096 nodes",
        term_name_bytes_limit = "variable and symbol names contain at most 256 bytes"
    ),
    example(
        label = "one_example",
        value = "{\"examples\":[{\"before\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},\"after\":{\"kind\":\"symbol\",\"name\":\"b\",\"arguments\":[]}}]}"
    )
)]
pub struct RewriteInferRuleRequest {
    /// Positive rewrite examples used for anti-unification.
    pub examples: Vec<RewriteExample>,
}

/// Exact checked rule inferred from positive examples.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-infer-rule/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        rhs_no_duplicate_variables = "the inferred RHS does not duplicate variable occurrences",
        rhs_variables_subset_lhs = "every inferred RHS variable occurs in its LHS",
        rule_within_term_bounds = "the inferred rule has depth at most 64 and at most 4096 nodes"
    )
)]
pub struct RewriteInferRuleOutput {
    /// Inferred checked first-order rewrite rule.
    pub rule: RewriteRule,
}

#[cfg(feature = "standard-probes")]
pub(super) fn normalize_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:normalize:v1".parse()?,
        capability_id: "amari:amari-rewrite:trs:normalization".parse()?,
        input_schema: "amari.discovery/probe/rewrite-normalize/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-normalize/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_normalize,
    })
}

#[cfg(feature = "standard-probes")]
pub(super) fn infer_rule_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:infer-rule:v1".parse()?,
        capability_id: "amari:amari-rewrite:synthesis:infer-rule".parse()?,
        input_schema: "amari.discovery/probe/rewrite-infer-rule/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-infer-rule/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_infer_rule,
    })
}

#[cfg(feature = "standard-probes")]
pub(super) fn predecessors_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:predecessors:v1".parse()?,
        capability_id: "amari:amari-rewrite:inverse:predecessors".parse()?,
        input_schema: "amari.discovery/probe/rewrite-predecessors/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-predecessors/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_predecessors,
    })
}

#[cfg(feature = "standard-probes")]
fn execute_normalize(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: RewriteNormalizeRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "rewrite normalization request has an invalid term, rule, or limit shape: {error}"
            ))
        })?;
    if request.max_steps == 0 {
        return Err(DiscoveryError::InvalidInput(
            "rewrite normalization max steps must be greater than zero".to_owned(),
        ));
    }
    enforce(
        "normalization steps",
        request.max_steps,
        MAX_NORMALIZATION_STEPS,
    )?;

    let bounds = effective_bounds(limits);
    let analysis = validate_rewrite_request(&request, [&request.term], &request.rules, bounds)?;
    if analysis.max_forward_constant > 0 {
        return Err(DiscoveryError::InvalidInput(
            "rewrite normalization rejects expanding rules".to_owned(),
        ));
    }
    let predicted_nodes = checked_growth_bound(
        analysis.input_nodes,
        request.max_steps,
        analysis.max_forward_constant,
    )?;
    enforce("term nodes", predicted_nodes, bounds.max_term_nodes)?;

    let rules = request
        .rules
        .iter()
        .map(RewriteRule::to_rule)
        .collect::<DiscoveryResult<Vec<_>>>()?;
    let system = TermSystem::new(rules);
    let mut current = request.term.to_term()?;
    let mut current_dto = request.term;
    let mut steps = 0_u64;
    let mut operations = 0_u64;
    let mut iterations = 0_u64;
    let mut observed_nodes = analysis.input_nodes;
    let rule_factor = analysis.rule_count.max(1);

    loop {
        let stats = term_stats(&current_dto, bounds.max_term_depth, bounds.max_term_nodes)?;
        observed_nodes = observed_nodes.max(stats.nodes);
        let attempt_operations = stats.nodes.checked_mul(rule_factor).ok_or_else(|| {
            DiscoveryError::LimitExceeded("rewrite operation count overflow".to_owned())
        })?;
        operations = operations.checked_add(attempt_operations).ok_or_else(|| {
            DiscoveryError::LimitExceeded("rewrite operation count overflow".to_owned())
        })?;
        iterations = iterations.checked_add(1).ok_or_else(|| {
            DiscoveryError::LimitExceeded("rewrite iteration count overflow".to_owned())
        })?;
        enforce("operations", operations, limits.max_operations)?;
        enforce("iterations", iterations, limits.max_iterations)?;

        let next = system.apply_once(&current).map_err(|_| {
            DiscoveryError::ProbeFailed("bounded rewrite normalization failed".to_owned())
        })?;
        let Some(next) = next else {
            let output = RewriteNormalizeOutput {
                normal_form: current_dto,
                steps,
            };
            validate_encoded_output(&output, bounds)?;
            return Ok(AdapterOutput {
                resources: ResourceObservations {
                    operations,
                    nodes: observed_nodes,
                    iterations,
                    bytes: 0,
                },
                output: serde_json::to_value(output)?,
            });
        };
        if steps == request.max_steps {
            return Err(DiscoveryError::LimitExceeded(format!(
                "rewrite normalization step limit {} reached before fixed point",
                request.max_steps
            )));
        }
        steps = steps.checked_add(1).ok_or_else(|| {
            DiscoveryError::LimitExceeded("rewrite normalization step count overflow".to_owned())
        })?;
        current = next;
        current_dto = RewriteTerm::from_term(&current);
    }
}

#[cfg(feature = "standard-probes")]
fn execute_infer_rule(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: RewriteInferRuleRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "rewrite inference request has an invalid example shape: {error}"
            ))
        })?;
    if request.examples.is_empty() {
        return Err(DiscoveryError::InvalidInput(
            "rewrite inference requires at least one example".to_owned(),
        ));
    }
    let example_count = u64::try_from(request.examples.len()).map_err(|_| {
        DiscoveryError::LimitExceeded("rewrite inference example count overflow".to_owned())
    })?;
    enforce(
        "inference example count",
        example_count,
        MAX_INFERENCE_EXAMPLES,
    )?;

    let bounds = effective_bounds(limits);
    let terms = request
        .examples
        .iter()
        .flat_map(|example| [&example.before, &example.after]);
    let analysis = validate_rewrite_request(&request, terms, &[], bounds)?;
    enforce(
        "inference input nodes",
        analysis.input_nodes,
        limits.max_nodes,
    )?;
    let operations = analysis
        .input_nodes
        .checked_mul(example_count)
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("rewrite inference operation count overflow".to_owned())
        })?;
    enforce("operations", operations, limits.max_operations)?;
    enforce("iterations", example_count, limits.max_iterations)?;

    let examples = request
        .examples
        .iter()
        .map(|example| Ok((example.before.to_term()?, example.after.to_term()?)))
        .collect::<DiscoveryResult<Vec<_>>>()?;
    let inferred = infer_rule(&examples).map_err(|_| {
        DiscoveryError::ProbeFailed("bounded rewrite rule inference failed".to_owned())
    })?;
    let inferred = RewriteRule::from_rule(&inferred);
    let lhs = term_stats(&inferred.lhs, bounds.max_term_depth, bounds.max_term_nodes)?;
    let rhs = term_stats(&inferred.rhs, bounds.max_term_depth, bounds.max_term_nodes)?;
    inferred.to_rule()?;
    let growth = analyze_rule_growth(&inferred)?;
    if growth.rhs_duplicates_variable {
        return Err(DiscoveryError::InvalidInput(
            "inferred rewrite rule RHS duplicates variable occurrences".to_owned(),
        ));
    }
    let generated_nodes = lhs.nodes.checked_add(rhs.nodes).ok_or_else(|| {
        DiscoveryError::LimitExceeded("generated rewrite rule node count overflow".to_owned())
    })?;
    let total_nodes = analysis
        .input_nodes
        .checked_add(generated_nodes)
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("generated rewrite rule node count overflow".to_owned())
        })?;
    if total_nodes > limits.max_nodes {
        return Err(DiscoveryError::LimitExceeded(format!(
            "rewrite generated rule nodes raise cumulative nodes {total_nodes} above limit {}",
            limits.max_nodes
        )));
    }

    let output = RewriteInferRuleOutput { rule: inferred };
    validate_encoded_output(&output, bounds)?;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations,
            nodes: total_nodes,
            iterations: example_count,
            bytes: 0,
        },
        output: serde_json::to_value(output)?,
    })
}

#[cfg(feature = "standard-probes")]
fn execute_predecessors(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: RewritePredecessorsRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "rewrite predecessor request has an invalid term, rule, or limit shape: {error}"
            ))
        })?;
    if request.max_results == 0 || request.max_frontier == 0 {
        return Err(DiscoveryError::InvalidInput(
            "rewrite predecessor results and frontier limits must be greater than zero".to_owned(),
        ));
    }
    enforce(
        "predecessor depth",
        request.max_depth,
        MAX_PREDECESSOR_DEPTH,
    )?;
    enforce(
        "predecessor results",
        request.max_results,
        MAX_PREDECESSOR_RESULTS,
    )?;
    enforce(
        "predecessor frontier",
        request.max_frontier,
        MAX_PREDECESSOR_FRONTIER,
    )?;

    let bounds = effective_bounds(limits);
    let analysis = validate_rewrite_request(&request, [&request.target], &request.rules, bounds)?;
    if analysis.lhs_duplicates_variable {
        return Err(DiscoveryError::InvalidInput(
            "reverse rewrite rejects a rule whose LHS duplicates variable occurrences".to_owned(),
        ));
    }
    let predicted_term_nodes = checked_growth_bound(
        analysis.input_nodes,
        request.max_depth,
        analysis.max_backward_constant,
    )?;
    enforce(
        "predecessor term nodes",
        predicted_term_nodes,
        bounds.max_term_nodes,
    )?;

    let rules = request
        .rules
        .iter()
        .map(RewriteRule::to_rule)
        .collect::<DiscoveryResult<Vec<_>>>()?;
    let target = request.target.to_term()?;
    let mut queue = VecDeque::from([(target.clone(), 0_u64)]);
    let mut visited = BTreeSet::from([target]);
    let mut predecessors = BTreeSet::new();
    let mut operations = 0_u64;
    let mut iterations = 0_u64;
    let mut cumulative_nodes = analysis.input_nodes;
    let mut encoded_result_bytes = 0_u64;
    let mut truncated = false;

    'search: while let Some((term, depth)) = queue.pop_front() {
        if depth >= request.max_depth {
            continue;
        }
        iterations = iterations.checked_add(1).ok_or_else(|| {
            DiscoveryError::LimitExceeded("rewrite predecessor iteration overflow".to_owned())
        })?;
        enforce("iterations", iterations, limits.max_iterations)?;

        for path in term.positions() {
            let subterm = term.subterm(&path).ok_or_else(|| {
                DiscoveryError::ProbeFailed(
                    "bounded predecessor search produced an invalid term path".to_owned(),
                )
            })?;
            for rule in &rules {
                operations = operations.checked_add(1).ok_or_else(|| {
                    DiscoveryError::LimitExceeded(
                        "rewrite predecessor operation count overflow".to_owned(),
                    )
                })?;
                enforce("operations", operations, limits.max_operations)?;
                let Some(substitution) = match_pattern(rule.rhs(), subterm) else {
                    continue;
                };
                let replacement = substitution.apply(rule.lhs());
                let candidate = term.replace_at(&path, replacement).map_err(|_| {
                    DiscoveryError::ProbeFailed("bounded predecessor replacement failed".to_owned())
                })?;
                if candidate == term || visited.contains(&candidate) {
                    continue;
                }
                if u64::try_from(predecessors.len()).map_err(|_| {
                    DiscoveryError::LimitExceeded(
                        "rewrite predecessor result count overflow".to_owned(),
                    )
                })? >= request.max_results
                {
                    truncated = true;
                    break 'search;
                }

                let candidate_dto = RewriteTerm::from_term(&candidate);
                let stats =
                    term_stats(&candidate_dto, bounds.max_term_depth, bounds.max_term_nodes)?;
                cumulative_nodes = cumulative_nodes.checked_add(stats.nodes).ok_or_else(|| {
                    DiscoveryError::LimitExceeded(
                        "rewrite predecessor cumulative node count overflow".to_owned(),
                    )
                })?;
                enforce("predecessor nodes", cumulative_nodes, limits.max_nodes)?;
                encoded_result_bytes = encoded_result_bytes
                    .checked_add(encoded_bytes(&candidate_dto, "rewrite predecessor")?)
                    .ok_or_else(|| {
                        DiscoveryError::LimitExceeded(
                            "rewrite predecessor output byte count overflow".to_owned(),
                        )
                    })?;
                enforce(
                    "predecessor output bytes",
                    encoded_result_bytes,
                    bounds.max_output_bytes,
                )?;

                visited.insert(candidate.clone());
                predecessors.insert(candidate_dto);
                let next_depth = depth.checked_add(1).ok_or_else(|| {
                    DiscoveryError::LimitExceeded("rewrite predecessor depth overflow".to_owned())
                })?;
                if next_depth < request.max_depth {
                    let next_frontier = queue.len().checked_add(1).ok_or_else(|| {
                        DiscoveryError::LimitExceeded(
                            "rewrite predecessor frontier overflow".to_owned(),
                        )
                    })?;
                    let next_frontier = u64::try_from(next_frontier).map_err(|_| {
                        DiscoveryError::LimitExceeded(
                            "rewrite predecessor frontier overflow".to_owned(),
                        )
                    })?;
                    enforce("predecessor frontier", next_frontier, request.max_frontier)?;
                    queue.push_back((candidate, next_depth));
                }
            }
        }
    }

    let output = RewritePredecessorsOutput {
        predecessors: predecessors.into_iter().collect(),
        truncated,
    };
    validate_encoded_output(&output, bounds)?;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations,
            nodes: cumulative_nodes,
            iterations,
            bytes: 0,
        },
        output: serde_json::to_value(output)?,
    })
}

#[cfg(feature = "standard-probes")]
fn effective_bounds(limits: &EffectiveProbeLimits) -> RewriteBounds {
    RewriteBounds {
        max_request_bytes: limits.max_input_bytes,
        max_output_bytes: limits.max_output_bytes,
        max_term_depth: MAX_TERM_DEPTH,
        max_term_nodes: MAX_TERM_NODES.min(limits.max_nodes),
        max_rules: MAX_RULES,
    }
}

#[cfg(feature = "standard-probes")]
#[derive(Clone, Copy, Debug)]
struct RewriteBounds {
    max_request_bytes: u64,
    max_output_bytes: u64,
    max_term_depth: u64,
    max_term_nodes: u64,
    max_rules: u64,
}

#[cfg(feature = "standard-probes")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RewriteInputAnalysis {
    request_bytes: u64,
    input_nodes: u64,
    max_input_depth: u64,
    rule_count: u64,
    max_forward_constant: u64,
    max_backward_constant: u64,
    lhs_duplicates_variable: bool,
}

#[cfg(feature = "standard-probes")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuleGrowth {
    forward_constant: u64,
    backward_constant: u64,
    lhs_duplicates_variable: bool,
    rhs_duplicates_variable: bool,
}

#[cfg(feature = "standard-probes")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TermStats {
    nodes: u64,
    depth: u64,
}

#[cfg(feature = "standard-probes")]
fn validate_rewrite_request<'a, S, I>(
    request: &S,
    terms: I,
    rules: &[RewriteRule],
    bounds: RewriteBounds,
) -> DiscoveryResult<RewriteInputAnalysis>
where
    S: Serialize,
    I: IntoIterator<Item = &'a RewriteTerm>,
{
    let request_bytes = encoded_bytes(request, "rewrite request")?;
    enforce("request bytes", request_bytes, bounds.max_request_bytes)?;

    let rule_count = u64::try_from(rules.len())
        .map_err(|_| DiscoveryError::LimitExceeded("rewrite rule count overflow".to_owned()))?;
    enforce("rule count", rule_count, bounds.max_rules)?;

    let mut input_nodes = 0_u64;
    let mut max_input_depth = 0_u64;
    for term in terms {
        let stats = term_stats(term, bounds.max_term_depth, bounds.max_term_nodes)?;
        input_nodes = input_nodes.checked_add(stats.nodes).ok_or_else(|| {
            DiscoveryError::LimitExceeded("rewrite input node count overflow".to_owned())
        })?;
        max_input_depth = max_input_depth.max(stats.depth);
    }

    let mut max_forward_constant = 0_u64;
    let mut max_backward_constant = 0_u64;
    let mut lhs_duplicates_variable = false;
    for rule in rules {
        let lhs = term_stats(&rule.lhs, bounds.max_term_depth, bounds.max_term_nodes)?;
        let rhs = term_stats(&rule.rhs, bounds.max_term_depth, bounds.max_term_nodes)?;
        max_input_depth = max_input_depth.max(lhs.depth).max(rhs.depth);
        rule.to_rule()?;
        let growth = analyze_rule_growth(rule)?;
        if growth.rhs_duplicates_variable {
            return Err(DiscoveryError::InvalidInput(
                "rewrite rule RHS duplicates variable occurrences".to_owned(),
            ));
        }
        max_forward_constant = max_forward_constant.max(growth.forward_constant);
        max_backward_constant = max_backward_constant.max(growth.backward_constant);
        lhs_duplicates_variable |= growth.lhs_duplicates_variable;
    }

    Ok(RewriteInputAnalysis {
        request_bytes,
        input_nodes,
        max_input_depth,
        rule_count,
        max_forward_constant,
        max_backward_constant,
        lhs_duplicates_variable,
    })
}

#[cfg(feature = "standard-probes")]
fn term_stats(term: &RewriteTerm, max_depth: u64, max_nodes: u64) -> DiscoveryResult<TermStats> {
    fn visit(
        term: &RewriteTerm,
        depth: u64,
        nodes: &mut u64,
        deepest: &mut u64,
        max_depth: u64,
        max_nodes: u64,
    ) -> DiscoveryResult<()> {
        if depth > max_depth {
            return Err(DiscoveryError::LimitExceeded(format!(
                "rewrite term depth {depth} exceeds limit {max_depth}"
            )));
        }
        *nodes = nodes.checked_add(1).ok_or_else(|| {
            DiscoveryError::LimitExceeded("rewrite term node count overflow".to_owned())
        })?;
        enforce("term nodes", *nodes, max_nodes)?;
        *deepest = (*deepest).max(depth);

        match term {
            RewriteTerm::Variable { name } => validate_name(name),
            RewriteTerm::Symbol { name, arguments } => {
                validate_name(name)?;
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    DiscoveryError::LimitExceeded("rewrite term depth overflow".to_owned())
                })?;
                for argument in arguments {
                    visit(argument, child_depth, nodes, deepest, max_depth, max_nodes)?;
                }
                Ok(())
            }
        }
    }

    let mut nodes = 0;
    let mut depth = 0;
    visit(term, 1, &mut nodes, &mut depth, max_depth, max_nodes)?;
    Ok(TermStats { nodes, depth })
}

#[cfg(feature = "standard-probes")]
fn analyze_rule_growth(rule: &RewriteRule) -> DiscoveryResult<RuleGrowth> {
    let lhs_nodes = count_nodes(&rule.lhs)?;
    let rhs_nodes = count_nodes(&rule.rhs)?;
    let mut lhs_variables = BTreeMap::new();
    let mut rhs_variables = BTreeMap::new();
    count_variables(&rule.lhs, &mut lhs_variables)?;
    count_variables(&rule.rhs, &mut rhs_variables)?;

    Ok(RuleGrowth {
        forward_constant: rhs_nodes.saturating_sub(lhs_nodes),
        backward_constant: lhs_nodes.saturating_sub(rhs_nodes),
        lhs_duplicates_variable: lhs_variables.values().any(|count| *count > 1),
        rhs_duplicates_variable: rhs_variables.values().any(|count| *count > 1),
    })
}

#[cfg(feature = "standard-probes")]
fn count_nodes(term: &RewriteTerm) -> DiscoveryResult<u64> {
    match term {
        RewriteTerm::Variable { .. } => Ok(1),
        RewriteTerm::Symbol { arguments, .. } => arguments.iter().try_fold(1_u64, |total, term| {
            total.checked_add(count_nodes(term)?).ok_or_else(|| {
                DiscoveryError::LimitExceeded("rewrite term node count overflow".to_owned())
            })
        }),
    }
}

#[cfg(feature = "standard-probes")]
fn count_variables<'a>(
    term: &'a RewriteTerm,
    counts: &mut BTreeMap<&'a str, u64>,
) -> DiscoveryResult<()> {
    match term {
        RewriteTerm::Variable { name } => {
            let count = counts.entry(name.as_str()).or_default();
            *count = count.checked_add(1).ok_or_else(|| {
                DiscoveryError::LimitExceeded("rewrite variable count overflow".to_owned())
            })?;
        }
        RewriteTerm::Symbol { arguments, .. } => {
            for argument in arguments {
                count_variables(argument, counts)?;
            }
        }
    }
    Ok(())
}

#[cfg(feature = "standard-probes")]
fn checked_growth_bound(initial: u64, steps: u64, growth_per_step: u64) -> DiscoveryResult<u64> {
    steps
        .checked_mul(growth_per_step)
        .and_then(|growth| initial.checked_add(growth))
        .ok_or_else(|| DiscoveryError::LimitExceeded("rewrite growth bound overflow".to_owned()))
}

#[cfg(feature = "standard-probes")]
fn validate_encoded_output<T: Serialize>(
    output: &T,
    bounds: RewriteBounds,
) -> DiscoveryResult<u64> {
    let bytes = encoded_bytes(output, "rewrite output")?;
    enforce("output bytes", bytes, bounds.max_output_bytes)?;
    Ok(bytes)
}

#[cfg(feature = "standard-probes")]
fn encoded_bytes<T: Serialize>(value: &T, context: &str) -> DiscoveryResult<u64> {
    let bytes = serde_json::to_vec(value)?;
    u64::try_from(bytes.len())
        .map_err(|_| DiscoveryError::LimitExceeded(format!("{context} byte count overflow")))
}

#[cfg(feature = "standard-probes")]
fn validate_name(name: &str) -> DiscoveryResult<()> {
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return Err(DiscoveryError::InvalidInput(format!(
            "rewrite name length {} is outside 1..={MAX_NAME_BYTES}",
            name.len()
        )));
    }
    Ok(())
}

#[cfg(feature = "standard-probes")]
fn enforce(kind: &str, observed: u64, maximum: u64) -> DiscoveryResult<()> {
    if observed <= maximum {
        Ok(())
    } else {
        Err(DiscoveryError::LimitExceeded(format!(
            "rewrite {kind} {observed} exceeds limit {maximum}"
        )))
    }
}

/// One step of the symbolic predecessor relation over checked rules.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-symbolic-predecessors/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        max_results_limit = "max_results is no greater than 1024",
        max_results_positive = "max_results is greater than zero",
        rules_checked = "every rule RHS variable occurs in its LHS",
        rules_count_limit = "at most 256 ordered rules are accepted",
        term_bounds = "terms have depth at most 64 and at most 4096 nodes",
        term_name_bytes_limit = "variable and symbol names contain at most 256 bytes"
    ),
    example(
        label = "one_symbolic_step",
        value = "{\"target\":{\"kind\":\"symbol\",\"name\":\"g\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]}]},\"rules\":[{\"lhs\":{\"kind\":\"symbol\",\"name\":\"f\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"X\"},{\"kind\":\"variable\",\"name\":\"Y\"}]},\"rhs\":{\"kind\":\"symbol\",\"name\":\"g\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"X\"}]}}],\"max_results\":16}"
    )
)]
pub struct RewriteSymbolicPredecessorsRequest {
    /// Target term whose one-step symbolic predecessors are requested.
    pub target: RewriteTerm,
    /// Ordered checked forward rules.
    pub rules: Vec<RewriteRule>,
    /// Maximum returned predecessors.
    pub max_results: u64,
}

/// Provenance of one symbolic predecessor step.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteSymbolicProvenance {
    /// Canonical 64-hex identity of the checked rule that inverted.
    pub rule_id: String,
    /// Position of the replaced subterm in the target.
    pub position: Vec<u64>,
    /// Freshening namespace scope of the query.
    pub scope: u64,
    /// Canonical 64-hex digest of the target term.
    pub target_hash: String,
    /// Canonical 64-hex digest of the predecessor term.
    pub predecessor_hash: String,
}

/// One residual constraint attached to a symbolic predecessor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RewriteTermConstraint {
    /// Required term equality.
    Equal {
        /// Left side.
        left: RewriteTerm,
        /// Right side.
        right: RewriteTerm,
    },
    /// Required term disequality.
    NotEqual {
        /// Left side.
        left: RewriteTerm,
        /// Right side.
        right: RewriteTerm,
    },
}

/// One symbolic predecessor with freshened existentials.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteSymbolicPredecessor {
    /// The predecessor term (with freshened logic variables).
    pub term: RewriteTerm,
    /// Freshened existential variable names.
    pub existentials: Vec<String>,
    /// Residual constraints on this predecessor.
    pub constraints: Vec<RewriteTermConstraint>,
    /// Step provenance.
    pub provenance: RewriteSymbolicProvenance,
}

/// Symbolic predecessor relation result.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-symbolic-predecessors/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        predecessors_within_result_limit = "the predecessor count never exceeds max_results",
        truncation_truthful = "truncated is true exactly when max_results omitted at least one discovered predecessor"
    ),
    example(
        label = "one_symbolic_step",
        value = "{\"predecessors\":[{\"term\":{\"kind\":\"symbol\",\"name\":\"f\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},{\"kind\":\"variable\",\"name\":\"?0.0\"}]},\"existentials\":[\"?0.0\"],\"constraints\":[],\"provenance\":{\"rule_id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"position\":[],\"scope\":0,\"target_hash\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"predecessor_hash\":\"0000000000000000000000000000000000000000000000000000000000000000\"}}],\"truncated\":false}"
    )
)]
pub struct RewriteSymbolicPredecessorsOutput {
    /// One-step symbolic predecessors in derivation order.
    pub predecessors: Vec<RewriteSymbolicPredecessor>,
    /// Whether max_results omitted at least one predecessor.
    pub truncated: bool,
}

/// Typed input for structural inverse analysis.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-inverse-analysis/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        rules_checked = "every rule RHS variable occurs in its LHS",
        rules_count_limit = "at most 256 ordered rules are accepted",
        term_bounds = "terms have depth at most 64 and at most 4096 nodes",
        term_name_bytes_limit = "variable and symbol names contain at most 256 bytes"
    ),
    example(
        label = "one_rule",
        value = "{\"rules\":[{\"lhs\":{\"kind\":\"symbol\",\"name\":\"f\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"X\"}]},\"rhs\":{\"kind\":\"symbol\",\"name\":\"g\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"X\"}]}}]}"
    )
)]
pub struct RewriteInverseAnalysisRequest {
    /// Ordered checked forward rules analyzed as a system.
    pub rules: Vec<RewriteRule>,
}

/// Structural branching estimate for one rule's backward behavior.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteBranchingEstimate {
    /// Existential choices introduced by erased variables.
    pub existentials: u64,
    /// Other rules whose RHS unifies with this rule's RHS.
    pub ambiguous_peers: u64,
}

/// Per-rule inverse analysis report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteInverseRuleReport {
    /// Canonical 64-hex identity of the analyzed rule.
    pub rule_id: String,
    /// LHS variables absent from the RHS (the residual schema).
    pub erased_variables: Vec<String>,
    /// "lossless" or "existential" backward behavior.
    pub backward: String,
    /// Canonical identities of rules whose RHS overlaps this RHS.
    pub rhs_ambiguity: Vec<String>,
    /// "reversible", "ambiguous", or "lossy_residual" classification.
    pub reversibility: String,
    /// Structural branching estimate.
    pub branching: RewriteBranchingEstimate,
    /// Unsupported or unknown reasons (empty for checked rules).
    pub unsupported_reasons: Vec<String>,
}

/// System-wide inverse analysis report.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-inverse-analysis/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(classifier_honest = "reversible requires lossless and unambiguous evidence"),
    example(
        label = "one_rule",
        value = "{\"rules\":[{\"rule_id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"erased_variables\":[],\"backward\":\"lossless\",\"rhs_ambiguity\":[],\"reversibility\":\"reversible\",\"branching\":{\"existentials\":0,\"ambiguous_peers\":0},\"unsupported_reasons\":[]}]}"
    )
)]
pub struct RewriteInverseAnalysisOutput {
    /// Per-rule analysis, in request rule order.
    pub rules: Vec<RewriteInverseRuleReport>,
}

/// Typed input for a residual-backed forward step and exact replay.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-residual-replay/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        path_indices_valid = "every path index selects a child of the enclosing term",
        rule_index_in_range = "rule_index is less than the rule count",
        rules_checked = "every rule RHS variable occurs in its LHS",
        rules_count_limit = "at most 256 ordered rules are accepted",
        term_bounds = "terms have depth at most 64 and at most 4096 nodes",
        term_name_bytes_limit = "variable and symbol names contain at most 256 bytes"
    ),
    example(
        label = "one_step",
        value = "{\"source\":{\"kind\":\"symbol\",\"name\":\"f\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},{\"kind\":\"symbol\",\"name\":\"b\",\"arguments\":[]}]},\"rules\":[{\"lhs\":{\"kind\":\"symbol\",\"name\":\"f\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"X\"},{\"kind\":\"variable\",\"name\":\"Y\"}]},\"rhs\":{\"kind\":\"symbol\",\"name\":\"g\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"X\"}]}}],\"rule_index\":0,\"path\":[]}"
    )
)]
pub struct RewriteResidualReplayRequest {
    /// Concrete source term.
    pub source: RewriteTerm,
    /// Ordered checked forward rules.
    pub rules: Vec<RewriteRule>,
    /// Index of the rule to apply.
    pub rule_index: u64,
    /// Child-index path of the rewritten subterm.
    pub path: Vec<u64>,
}

/// One erased binding in a residual.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteErasedBinding {
    /// Schema position of the erased variable (sorted order).
    pub key_index: u64,
    /// The bound term.
    pub term: RewriteTerm,
}

/// Residual authority emitted by a forward step.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteResidualAuthority {
    /// Canonical 64-hex identity of the rule that fired.
    pub rule_id: String,
    /// Position of the replaced subterm.
    pub position: Vec<u64>,
    /// Bindings the forward step erased.
    pub erased_bindings: Vec<RewriteErasedBinding>,
    /// Canonical 64-hex digest of the source term.
    pub source_hash: String,
    /// Canonical 64-hex digest of the target term.
    pub target_hash: String,
}

/// Residual round-trip evidence.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-residual-replay/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        roundtrip_exact = "matches_source is true and reconstructed equals the request source whenever the probe succeeds"
    ),
    example(
        label = "one_step",
        value = "{\"target\":{\"kind\":\"symbol\",\"name\":\"g\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]}]},\"residual\":{\"rule_id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"position\":[],\"erased_bindings\":[{\"key_index\":0,\"term\":{\"kind\":\"symbol\",\"name\":\"b\",\"arguments\":[]}}],\"source_hash\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"target_hash\":\"0000000000000000000000000000000000000000000000000000000000000000\"},\"reconstructed\":{\"kind\":\"symbol\",\"name\":\"f\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},{\"kind\":\"symbol\",\"name\":\"b\",\"arguments\":[]}]},\"matches_source\":true}"
    )
)]
pub struct RewriteResidualReplayOutput {
    /// The rewritten term.
    pub target: RewriteTerm,
    /// Residual authority from the forward step.
    pub residual: RewriteResidualAuthority,
    /// The term reconstructed by validating replay.
    pub reconstructed: RewriteTerm,
    /// Exact round-trip evidence: replay returned the source.
    pub matches_source: bool,
}

#[cfg(feature = "standard-probes")]
pub(super) fn symbolic_predecessors_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:symbolic-predecessors:v1".parse()?,
        capability_id: "amari:amari-rewrite:inverse:symbolic-predecessors".parse()?,
        input_schema: "amari.discovery/probe/rewrite-symbolic-predecessors/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-symbolic-predecessors/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_symbolic_predecessors,
    })
}

#[cfg(feature = "standard-probes")]
pub(super) fn inverse_analysis_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:inverse-analysis:v1".parse()?,
        capability_id: "amari:amari-rewrite:inverse:analysis".parse()?,
        input_schema: "amari.discovery/probe/rewrite-inverse-analysis/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-inverse-analysis/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_inverse_analysis,
    })
}

#[cfg(feature = "standard-probes")]
pub(super) fn residual_replay_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:residual-replay:v1".parse()?,
        capability_id: "amari:amari-rewrite:reversible:residual-replay".parse()?,
        input_schema: "amari.discovery/probe/rewrite-residual-replay/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-residual-replay/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_residual_replay,
    })
}

#[cfg(feature = "standard-probes")]
fn path_from_indices(indices: &[u64]) -> DiscoveryResult<amari_rewrite::rewritable::Path> {
    let mut path = amari_rewrite::rewritable::Path::root();
    for index in indices {
        let index = usize::try_from(*index).map_err(|_| {
            DiscoveryError::InvalidInput("rewrite path index overflows usize".to_owned())
        })?;
        path = path.child(index);
    }
    Ok(path)
}

#[cfg(feature = "standard-probes")]
fn execute_symbolic_predecessors(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: RewriteSymbolicPredecessorsRequest = serde_json::from_value(input.clone())
        .map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "symbolic predecessor request has an invalid term, rule, or limit shape: {error}"
            ))
        })?;
    if request.max_results == 0 {
        return Err(DiscoveryError::InvalidInput(
            "symbolic predecessor max results must be greater than zero".to_owned(),
        ));
    }
    enforce(
        "symbolic predecessor results",
        request.max_results,
        MAX_PREDECESSOR_RESULTS,
    )?;

    let bounds = effective_bounds(limits);
    let analysis = validate_rewrite_request(&request, [&request.target], &request.rules, bounds)?;

    let rules = request
        .rules
        .iter()
        .map(RewriteRule::to_rule)
        .collect::<DiscoveryResult<Vec<_>>>()?;
    let system = TermSystem::new(rules);
    let target = request.target.to_term()?;
    let predecessors = amari_rewrite::inverse::symbolic_predecessors(
        &system,
        &target,
        &amari_rewrite::relation::RelationLimits::default(),
        0,
    )
    .map_err(|error| {
        DiscoveryError::ProbeFailed(format!("symbolic predecessor derivation failed: {error}"))
    })?;

    let mut operations = analysis.rule_count.max(1);
    let mut cumulative_nodes = analysis.input_nodes;
    let mut truncated = false;
    let mut predecessors_dto = Vec::new();
    for predecessor in &predecessors {
        operations = operations.checked_add(1).ok_or_else(|| {
            DiscoveryError::LimitExceeded("symbolic predecessor operation overflow".to_owned())
        })?;
        enforce("operations", operations, limits.max_operations)?;
        if u64::try_from(predecessors_dto.len()).map_err(|_| {
            DiscoveryError::LimitExceeded("symbolic predecessor count overflow".to_owned())
        })? >= request.max_results
        {
            truncated = true;
            break;
        }
        let term_dto = RewriteTerm::from_term(&predecessor.term);
        let stats = term_stats(&term_dto, bounds.max_term_depth, bounds.max_term_nodes)?;
        cumulative_nodes = cumulative_nodes.checked_add(stats.nodes).ok_or_else(|| {
            DiscoveryError::LimitExceeded("symbolic predecessor node overflow".to_owned())
        })?;
        enforce(
            "symbolic predecessor nodes",
            cumulative_nodes,
            limits.max_nodes,
        )?;
        predecessors_dto.push(RewriteSymbolicPredecessor {
            term: term_dto,
            existentials: predecessor
                .existentials
                .iter()
                .map(ToString::to_string)
                .collect(),
            constraints: predecessor
                .constraints
                .constraints()
                .iter()
                .map(|constraint| match constraint {
                    amari_rewrite::relation::TermConstraint::Equal(left, right) => {
                        RewriteTermConstraint::Equal {
                            left: RewriteTerm::from_term(left),
                            right: RewriteTerm::from_term(right),
                        }
                    }
                    amari_rewrite::relation::TermConstraint::NotEqual(left, right) => {
                        RewriteTermConstraint::NotEqual {
                            left: RewriteTerm::from_term(left),
                            right: RewriteTerm::from_term(right),
                        }
                    }
                })
                .collect(),
            provenance: RewriteSymbolicProvenance {
                rule_id: predecessor.provenance.rule_id.to_string(),
                position: predecessor
                    .provenance
                    .position
                    .as_slice()
                    .iter()
                    .map(|index| *index as u64)
                    .collect(),
                scope: u64::from(predecessor.provenance.scope),
                target_hash: predecessor.provenance.target_hash.to_hex(),
                predecessor_hash: predecessor.provenance.predecessor_hash.to_hex(),
            },
        });
    }

    let output = RewriteSymbolicPredecessorsOutput {
        predecessors: predecessors_dto,
        truncated,
    };
    validate_encoded_output(&output, bounds)?;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations,
            nodes: cumulative_nodes,
            iterations: 1,
            bytes: 0,
        },
        output: serde_json::to_value(output)?,
    })
}

#[cfg(feature = "standard-probes")]
fn execute_inverse_analysis(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: RewriteInverseAnalysisRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "inverse analysis request has an invalid rule shape: {error}"
            ))
        })?;
    let bounds = effective_bounds(limits);
    let analysis = validate_rewrite_request(&request, [], &request.rules, bounds)?;

    let rules = request
        .rules
        .iter()
        .map(RewriteRule::to_rule)
        .collect::<DiscoveryResult<Vec<_>>>()?;
    let system = amari_rewrite::reversible::BidirectionalSystem::new(rules).map_err(|error| {
        DiscoveryError::InvalidInput(format!("inverse analysis rejected the rule set: {error}"))
    })?;
    let report = amari_rewrite::analysis::InverseAnalyzer::analyze(&system);
    enforce(
        "operations",
        analysis.rule_count.max(1),
        limits.max_operations,
    )?;

    let output = RewriteInverseAnalysisOutput {
        rules: report
            .rules
            .iter()
            .map(|rule| RewriteInverseRuleReport {
                rule_id: rule.rule_id.to_string(),
                erased_variables: rule.erased_variables.clone(),
                backward: match rule.backward {
                    amari_rewrite::analysis::BackwardKind::Lossless => "lossless",
                    amari_rewrite::analysis::BackwardKind::Existential => "existential",
                }
                .to_owned(),
                rhs_ambiguity: rule.rhs_ambiguity.iter().map(ToString::to_string).collect(),
                reversibility: match rule.reversibility {
                    amari_rewrite::analysis::ReversibilityClass::Reversible => "reversible",
                    amari_rewrite::analysis::ReversibilityClass::Ambiguous => "ambiguous",
                    amari_rewrite::analysis::ReversibilityClass::LossyResidual => "lossy_residual",
                }
                .to_owned(),
                branching: RewriteBranchingEstimate {
                    existentials: rule.branching.existentials as u64,
                    ambiguous_peers: rule.branching.ambiguous_peers as u64,
                },
                unsupported_reasons: rule.unsupported_reasons.clone(),
            })
            .collect(),
    };
    validate_encoded_output(&output, bounds)?;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations: analysis.rule_count.max(1),
            nodes: analysis.input_nodes,
            iterations: 1,
            bytes: 0,
        },
        output: serde_json::to_value(output)?,
    })
}

#[cfg(feature = "standard-probes")]
fn execute_residual_replay(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: RewriteResidualReplayRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "residual replay request has an invalid term, rule, or path shape: {error}"
            ))
        })?;
    let bounds = effective_bounds(limits);
    let analysis = validate_rewrite_request(&request, [&request.source], &request.rules, bounds)?;

    let rules = request
        .rules
        .iter()
        .map(RewriteRule::to_rule)
        .collect::<DiscoveryResult<Vec<_>>>()?;
    let rule_index = usize::try_from(request.rule_index).map_err(|_| {
        DiscoveryError::InvalidInput("residual replay rule index overflows usize".to_owned())
    })?;
    let rule = rules.get(rule_index).ok_or_else(|| {
        DiscoveryError::InvalidInput(format!(
            "residual replay rule index {rule_index} is out of range for {} rules",
            rules.len()
        ))
    })?;
    let path = path_from_indices(&request.path)?;
    let rule_id = amari_rewrite::relation::RuleId::from_rule(rule);
    let system = amari_rewrite::reversible::BidirectionalSystem::new(rules).map_err(|error| {
        DiscoveryError::InvalidInput(format!("residual replay rejected the rule set: {error}"))
    })?;
    let source = request.source.to_term()?;
    let relation_limits = amari_rewrite::relation::RelationLimits::default();

    let step = system
        .forward_step(&source, &rule_id, &path, true, &relation_limits)
        .map_err(|error| {
            DiscoveryError::InvalidInput(format!("residual replay forward step failed: {error}"))
        })?;
    let residual = step.residual.ok_or_else(|| {
        DiscoveryError::ProbeFailed("residual replay did not emit residual authority".to_owned())
    })?;
    let reconstructed = system
        .replay(&residual, &step.target, &relation_limits)
        .map_err(|error| {
            DiscoveryError::ProbeFailed(format!("residual replay reconstruction failed: {error}"))
        })?;
    let matches_source = reconstructed == source;

    let output = RewriteResidualReplayOutput {
        target: RewriteTerm::from_term(&step.target),
        residual: RewriteResidualAuthority {
            rule_id: residual.rule_id.to_string(),
            position: residual
                .position
                .as_slice()
                .iter()
                .map(|index| *index as u64)
                .collect(),
            erased_bindings: residual
                .erased_bindings
                .iter()
                .map(|(key, term)| RewriteErasedBinding {
                    key_index: u64::from(key.index()),
                    term: RewriteTerm::from_term(term),
                })
                .collect(),
            source_hash: residual.source_hash.to_hex(),
            target_hash: residual.target_hash.to_hex(),
        },
        reconstructed: RewriteTerm::from_term(&reconstructed),
        matches_source,
    };
    validate_encoded_output(&output, bounds)?;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations: 2,
            nodes: analysis.input_nodes,
            iterations: 2,
            bytes: 0,
        },
        output: serde_json::to_value(output)?,
    })
}

#[cfg(all(test, feature = "standard-probes"))]
mod tests {
    use amari_rewrite::trs::{Term, Variable};
    use serde::Serialize;

    use super::*;
    use crate::DiscoveryError;

    fn var(name: &str) -> RewriteTerm {
        RewriteTerm::Variable {
            name: name.to_owned(),
        }
    }

    fn sym(name: &str, arguments: Vec<RewriteTerm>) -> RewriteTerm {
        RewriteTerm::Symbol {
            name: name.to_owned(),
            arguments,
        }
    }

    fn rule(lhs: RewriteTerm, rhs: RewriteTerm) -> RewriteRule {
        RewriteRule { lhs, rhs }
    }

    fn bounds() -> RewriteBounds {
        RewriteBounds {
            max_request_bytes: 65_536,
            max_output_bytes: 65_536,
            max_term_depth: 64,
            max_term_nodes: 4_096,
            max_rules: 256,
        }
    }

    #[derive(Serialize)]
    struct Request<'a> {
        term: &'a RewriteTerm,
        rules: &'a [RewriteRule],
    }

    #[test]
    fn recursive_term_and_rule_dtos_convert_to_checked_trs_values() {
        let term = sym("f", vec![var("X"), sym("g", vec![sym("a", Vec::new())])]);
        let dto_rule = rule(
            sym("f", vec![var("X"), var("Y")]),
            sym("pair", vec![var("X"), var("Y")]),
        );

        assert_eq!(
            term.to_term().unwrap(),
            Term::sym(
                "f",
                [
                    Term::Var(Variable::new("X")),
                    Term::sym("g", [Term::constant("a")])
                ]
            )
        );
        let converted = dto_rule.to_rule().unwrap();
        assert_eq!(converted.lhs(), &dto_rule.lhs.to_term().unwrap());
        assert_eq!(converted.rhs(), &dto_rule.rhs.to_term().unwrap());
    }

    #[test]
    fn validation_counts_request_bytes_term_depth_nodes_and_rules() {
        let term = sym("f", vec![sym("g", vec![var("X")]), sym("a", vec![])]);
        let rules = vec![rule(sym("g", vec![var("X")]), var("X"))];
        let request = Request {
            term: &term,
            rules: &rules,
        };
        let encoded = serde_json::to_vec(&request).unwrap().len() as u64;
        let analysis = validate_rewrite_request(&request, [&term], &rules, bounds()).unwrap();

        assert_eq!(analysis.request_bytes, encoded);
        assert_eq!(analysis.input_nodes, 4);
        assert_eq!(analysis.max_input_depth, 3);
        assert_eq!(analysis.rule_count, 1);

        let mut too_few_bytes = bounds();
        too_few_bytes.max_request_bytes = encoded - 1;
        assert!(matches!(
            validate_rewrite_request(&request, [&term], &rules, too_few_bytes),
            Err(DiscoveryError::LimitExceeded(message)) if message.contains("request bytes")
        ));

        let mut too_shallow = bounds();
        too_shallow.max_term_depth = 2;
        assert!(matches!(
            validate_rewrite_request(&request, [&term], &rules, too_shallow),
            Err(DiscoveryError::LimitExceeded(message)) if message.contains("depth")
        ));

        let mut too_few_nodes = bounds();
        too_few_nodes.max_term_nodes = 3;
        assert!(matches!(
            validate_rewrite_request(&request, [&term], &rules, too_few_nodes),
            Err(DiscoveryError::LimitExceeded(message)) if message.contains("nodes")
        ));

        let mut too_few_rules = bounds();
        too_few_rules.max_rules = 0;
        assert!(matches!(
            validate_rewrite_request(&request, [&term], &rules, too_few_rules),
            Err(DiscoveryError::LimitExceeded(message)) if message.contains("rule count")
        ));
    }

    #[test]
    fn encoded_output_bytes_are_checked_before_return() {
        let output = sym("result", vec![sym("a", vec![]), sym("b", vec![])]);
        let bytes = serde_json::to_vec(&output).unwrap().len() as u64;

        let mut exact = bounds();
        exact.max_output_bytes = bytes;
        assert_eq!(validate_encoded_output(&output, exact).unwrap(), bytes);
        exact.max_output_bytes = bytes - 1;
        assert!(matches!(
            validate_encoded_output(&output, exact),
            Err(DiscoveryError::LimitExceeded(message)) if message.contains("output bytes")
        ));
    }

    #[test]
    fn duplicate_rhs_variables_and_unbound_rhs_variables_are_rejected() {
        let duplicate = vec![rule(
            sym("f", vec![var("X")]),
            sym("pair", vec![var("X"), var("X")]),
        )];
        let request = Request {
            term: &duplicate[0].lhs,
            rules: &duplicate,
        };
        assert!(matches!(
            validate_rewrite_request(&request, [&duplicate[0].lhs], &duplicate, bounds()),
            Err(DiscoveryError::InvalidInput(message)) if message.contains("duplicates variable")
        ));

        let unbound = rule(sym("f", vec![var("X")]), var("Y"));
        assert!(matches!(
            unbound.to_rule(),
            Err(DiscoveryError::InvalidInput(message)) if message.contains("does not occur")
        ));
    }

    #[test]
    fn constant_growth_is_directional_and_checked() {
        let expanding = rule(
            sym("f", vec![var("X")]),
            sym("g", vec![sym("a", vec![]), var("X")]),
        );
        let contracting = rule(expanding.rhs.clone(), expanding.lhs.clone());

        assert_eq!(analyze_rule_growth(&expanding).unwrap().forward_constant, 1);
        assert_eq!(
            analyze_rule_growth(&expanding).unwrap().backward_constant,
            0
        );
        assert_eq!(
            analyze_rule_growth(&contracting).unwrap().forward_constant,
            0
        );
        assert_eq!(
            analyze_rule_growth(&contracting).unwrap().backward_constant,
            1
        );
        assert_eq!(checked_growth_bound(3, 4, 2).unwrap(), 11);
        assert!(matches!(
            checked_growth_bound(u64::MAX, 1, 1),
            Err(DiscoveryError::LimitExceeded(message)) if message.contains("growth")
        ));
        assert!(matches!(
            checked_growth_bound(1, u64::MAX, 2),
            Err(DiscoveryError::LimitExceeded(message)) if message.contains("growth")
        ));
    }

    #[test]
    fn empty_and_oversized_names_are_rejected_without_conversion() {
        for term in [var(""), sym(&"x".repeat(257), vec![])] {
            let request = Request {
                term: &term,
                rules: &[],
            };
            assert!(matches!(
                validate_rewrite_request(&request, [&term], &[], bounds()),
                Err(DiscoveryError::InvalidInput(message)) if message.contains("name")
            ));
        }
    }
}

/// Search ordering accepted by the backward-search probe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RewriteSearchMode {
    /// Canonical breadth-first ordering.
    BreadthFirst,
    /// Deterministic symbolic cost ordering.
    CostFirst,
}

/// Guidance accepted by the backward-search probe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RewriteGuidance {
    /// Ordering only; no candidate is dropped before a limit.
    CompleteWithinLimits,
    /// Explicit beam pruning with per-round dropped counts.
    HeuristicPruning {
        /// Candidates kept per expansion round.
        beam_width: u64,
    },
}

/// Typed input for bounded symbolic backward search.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-backward-search/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        rules_checked = "every rule RHS variable occurs in its LHS",
        rules_count_limit = "at most 256 ordered rules are accepted",
        term_bounds = "terms have depth at most 64 and at most 4096 nodes",
        term_name_bytes_limit = "variable and symbol names contain at most 256 bytes",
        max_depth_limit = "max_depth is between 1 and 16",
        beam_width_limit = "heuristic_pruning beam width is between 1 and 1024"
    ),
    example(
        label = "one_step_witness",
        value = "{\"target\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},\"goal\":{\"kind\":\"symbol\",\"name\":\"add\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},{\"kind\":\"symbol\",\"name\":\"zero\",\"arguments\":[]}]},\"rules\":[{\"lhs\":{\"kind\":\"symbol\",\"name\":\"add\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"X\"},{\"kind\":\"symbol\",\"name\":\"zero\",\"arguments\":[]}]},\"rhs\":{\"kind\":\"variable\",\"name\":\"X\"}}],\"mode\":\"breadth_first\",\"guidance\":{\"kind\":\"complete_within_limits\"},\"max_depth\":8}"
    )
)]
pub struct RewriteBackwardSearchRequest {
    /// Target term the search starts from. Variable names are
    /// SHARED with the goal: a name used in both terms denotes the
    /// same variable, and unification enforces one common
    /// instantiation.
    pub target: RewriteTerm,
    /// Goal pattern the search seeks.
    pub goal: RewriteTerm,
    /// Ordered checked forward rules.
    pub rules: Vec<RewriteRule>,
    /// Expansion ordering.
    pub mode: RewriteSearchMode,
    /// Guidance mode (complete or explicit beam pruning).
    pub guidance: RewriteGuidance,
    /// Maximum search depth (1..=16).
    pub max_depth: u64,
}

/// Typed output of bounded symbolic backward search.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-backward-search/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        outcome_truthful = "witness steps replay through the submitted system; exhausted is returned only on zero-drop frontier closure; approximate carries exact dropped counts"
    ),
    example(
        label = "one_step_witness",
        value = "{\"outcome\":\"witness\",\"steps\":[{\"term\":{\"kind\":\"symbol\",\"name\":\"add\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},{\"kind\":\"symbol\",\"name\":\"zero\",\"arguments\":[]}]},\"existentials\":[],\"constraints\":[],\"provenance\":{\"rule_id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"position\":[],\"scope\":1,\"target_hash\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"predecessor_hash\":\"0000000000000000000000000000000000000000000000000000000000000000\"}}],\"exhaustion_authority\":null,\"frontier\":[],\"depth_reached\":1,\"dropped_candidates\":0,\"reason\":null}"
    )
)]
pub struct RewriteBackwardSearchOutput {
    /// Typed outcome: witness, exhausted, partial, approximate, or
    /// unsupported.
    pub outcome: String,
    /// Witness derivation steps (empty unless outcome is witness).
    pub steps: Vec<RewriteSymbolicPredecessor>,
    /// Exhaustion authority kind (only when outcome is exhausted).
    pub exhaustion_authority: Option<String>,
    /// Retained unexpanded frontier terms (partial/approximate).
    pub frontier: Vec<RewriteTerm>,
    /// Deepest depth reached: for a witness, the derivation length
    /// (the depth at which the witness was found); for partial and
    /// approximate outcomes, the deepest retained frontier depth.
    pub depth_reached: u64,
    /// Candidates dropped by heuristic pruning (0 otherwise).
    pub dropped_candidates: u64,
    /// Reason string for unsupported or approximate summaries.
    pub reason: Option<String>,
}

/// Typed input for bounded bidirectional search.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-bidirectional-search/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        rules_checked = "every rule RHS variable occurs in its LHS",
        rules_count_limit = "at most 256 ordered rules are accepted",
        term_bounds = "terms have depth at most 64 and at most 4096 nodes",
        term_name_bytes_limit = "variable and symbol names contain at most 256 bytes",
        max_depth_limit = "max_depth is between 1 and 16"
    ),
    example(
        label = "forward_meets_backward",
        value = "{\"source\":{\"kind\":\"symbol\",\"name\":\"add\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},{\"kind\":\"symbol\",\"name\":\"zero\",\"arguments\":[]}]},\"goal\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},\"rules\":[{\"lhs\":{\"kind\":\"symbol\",\"name\":\"add\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"X\"},{\"kind\":\"symbol\",\"name\":\"zero\",\"arguments\":[]}]},\"rhs\":{\"kind\":\"variable\",\"name\":\"X\"}}],\"max_depth\":8}"
    )
)]
pub struct RewriteBidirectionalSearchRequest {
    /// Source term the forward frontier starts from. Variable names
    /// are SHARED with the goal: a name used in both terms denotes
    /// the same variable, and unification enforces one common
    /// instantiation.
    pub source: RewriteTerm,
    /// Goal term the backward frontier starts from.
    pub goal: RewriteTerm,
    /// Ordered checked forward rules.
    pub rules: Vec<RewriteRule>,
    /// Maximum per-side search depth (1..=16).
    pub max_depth: u64,
}

/// One forward step in a bidirectional witness.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteForwardStep {
    /// Canonical 64-hex identity of the applied rule.
    pub rule_id: String,
    /// Position of the rewritten subterm.
    pub position: Vec<u64>,
    /// Canonical 64-hex digest of the source term.
    pub source_hash: String,
    /// Canonical 64-hex digest of the result term.
    pub result_hash: String,
}

/// Typed output of bounded bidirectional search.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-bidirectional-search/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        outcome_truthful = "meets replay through the submitted system before acceptance; exhausted is returned only on zero-drop closure of both frontiers"
    ),
    example(
        label = "forward_meets_backward",
        value = "{\"outcome\":\"witness\",\"forward_steps\":[{\"rule_id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"position\":[],\"source_hash\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"result_hash\":\"0000000000000000000000000000000000000000000000000000000000000000\"}],\"backward_steps\":[],\"meeting_forward\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},\"meeting_backward\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},\"exhaustion_authority\":null,\"frontier_forward\":[],\"frontier_backward\":[],\"depth_reached\":1,\"reason\":null}"
    )
)]
pub struct RewriteBidirectionalSearchOutput {
    /// Typed outcome: witness, exhausted, partial, or unsupported.
    pub outcome: String,
    /// Forward steps from source to the meeting point.
    pub forward_steps: Vec<RewriteForwardStep>,
    /// Backward steps from goal to the meeting point.
    pub backward_steps: Vec<RewriteSymbolicPredecessor>,
    /// Meeting term on the forward side (witness only).
    pub meeting_forward: Option<RewriteTerm>,
    /// Meeting term on the backward side (witness only).
    pub meeting_backward: Option<RewriteTerm>,
    /// Exhaustion authority kind (only when outcome is exhausted).
    pub exhaustion_authority: Option<String>,
    /// Retained unexpanded forward frontier terms.
    pub frontier_forward: Vec<RewriteTerm>,
    /// Retained unexpanded backward frontier terms.
    pub frontier_backward: Vec<RewriteTerm>,
    /// Deepest depth reached on either side: for a witness, the
    /// forward derivation length; for partial, the deepest retained
    /// frontier depth.
    pub depth_reached: u64,
    /// Reason string for unsupported outcomes.
    pub reason: Option<String>,
}

#[cfg(feature = "standard-probes")]
const MAX_SEARCH_DEPTH: u64 = 16;
#[cfg(feature = "standard-probes")]
const MAX_BEAM_WIDTH: u64 = 1_024;

#[cfg(feature = "standard-probes")]
pub(super) fn backward_search_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:backward-search:v1".parse()?,
        capability_id: "amari:amari-rewrite:inverse:backward-search".parse()?,
        input_schema: "amari.discovery/probe/rewrite-backward-search/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-backward-search/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_backward_search,
    })
}

#[cfg(feature = "standard-probes")]
pub(super) fn bidirectional_search_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:bidirectional-search:v1".parse()?,
        capability_id: "amari:amari-rewrite:inverse:bidirectional-search".parse()?,
        input_schema: "amari.discovery/probe/rewrite-bidirectional-search/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-bidirectional-search/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_bidirectional_search,
    })
}

#[cfg(feature = "standard-probes")]
fn search_config(max_depth: u64) -> DiscoveryResult<amari_rewrite::inverse::InverseSearchConfig> {
    if max_depth == 0 || max_depth > MAX_SEARCH_DEPTH {
        return Err(DiscoveryError::InvalidInput(format!(
            "search max depth must be between 1 and {MAX_SEARCH_DEPTH}"
        )));
    }
    amari_rewrite::inverse::InverseSearchConfig::new(
        max_depth,
        4_096,
        16_384,
        4_096,
        64,
        4_096,
        65_536,
        50_000,
        1 << 20,
        1 << 20,
    )
    .map_err(|error| DiscoveryError::InvalidInput(format!("search configuration invalid: {error}")))
}

#[cfg(feature = "standard-probes")]
fn predecessor_dto(
    predecessor: &amari_rewrite::inverse::SymbolicPredecessor,
) -> RewriteSymbolicPredecessor {
    RewriteSymbolicPredecessor {
        term: RewriteTerm::from_term(&predecessor.term),
        existentials: predecessor
            .existentials
            .iter()
            .map(ToString::to_string)
            .collect(),
        constraints: predecessor
            .constraints
            .constraints()
            .iter()
            .map(|constraint| match constraint {
                amari_rewrite::relation::TermConstraint::Equal(left, right) => {
                    RewriteTermConstraint::Equal {
                        left: RewriteTerm::from_term(left),
                        right: RewriteTerm::from_term(right),
                    }
                }
                amari_rewrite::relation::TermConstraint::NotEqual(left, right) => {
                    RewriteTermConstraint::NotEqual {
                        left: RewriteTerm::from_term(left),
                        right: RewriteTerm::from_term(right),
                    }
                }
            })
            .collect(),
        provenance: RewriteSymbolicProvenance {
            rule_id: predecessor.provenance.rule_id.to_string(),
            position: predecessor
                .provenance
                .position
                .as_slice()
                .iter()
                .map(|index| *index as u64)
                .collect(),
            scope: u64::from(predecessor.provenance.scope),
            target_hash: predecessor.provenance.target_hash.to_hex(),
            predecessor_hash: predecessor.provenance.predecessor_hash.to_hex(),
        },
    }
}

#[cfg(feature = "standard-probes")]
fn execute_backward_search(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: RewriteBackwardSearchRequest = serde_json::from_value(input.clone()).map_err(|error| {
        DiscoveryError::InvalidInput(format!(
            "backward search request has an invalid term, rule, mode, guidance, or limit shape: {error}"
        ))
    })?;
    let bounds = effective_bounds(limits);
    validate_rewrite_request(
        &request,
        [&request.target, &request.goal],
        &request.rules,
        bounds,
    )?;
    let rules = request
        .rules
        .iter()
        .map(RewriteRule::to_rule)
        .collect::<DiscoveryResult<Vec<_>>>()?;
    let system = TermSystem::new(rules);
    let target = request.target.to_term()?;
    let goal = request.goal.to_term()?;
    let config = search_config(request.max_depth)?;
    let mode = match request.mode {
        RewriteSearchMode::BreadthFirst => amari_rewrite::inverse::SearchMode::BreadthFirst,
        RewriteSearchMode::CostFirst => amari_rewrite::inverse::SearchMode::CostFirst,
    };
    let guidance = match &request.guidance {
        RewriteGuidance::CompleteWithinLimits => {
            amari_rewrite::inverse::GuidanceMode::CompleteWithinLimits
        }
        RewriteGuidance::HeuristicPruning { beam_width } => {
            if *beam_width == 0 || *beam_width > MAX_BEAM_WIDTH {
                return Err(DiscoveryError::InvalidInput(format!(
                    "heuristic pruning beam width must be between 1 and {MAX_BEAM_WIDTH}"
                )));
            }
            amari_rewrite::inverse::GuidanceMode::HeuristicPruning {
                beam_width: *beam_width,
            }
        }
    };
    let outcome = amari_rewrite::inverse::BackwardExplorer::new(&system, config)
        .search_with_guidance(&target, &goal, mode, &guidance)
        .map_err(|error| DiscoveryError::ProbeFailed(format!("backward search failed: {error}")))?;
    let output = match outcome {
        amari_rewrite::inverse::BackwardSearchOutcome::Witness(derivation) => {
            RewriteBackwardSearchOutput {
                outcome: "witness".to_owned(),
                depth_reached: derivation.steps.len() as u64,
                steps: derivation.steps.iter().map(predecessor_dto).collect(),
                exhaustion_authority: None,
                frontier: Vec::new(),
                dropped_candidates: 0,
                reason: None,
            }
        }
        amari_rewrite::inverse::BackwardSearchOutcome::Exhausted(certified) => {
            RewriteBackwardSearchOutput {
                outcome: "exhausted".to_owned(),
                steps: Vec::new(),
                exhaustion_authority: Some(certified.authority().wire_tag().to_owned()),
                frontier: Vec::new(),
                depth_reached: 0,
                dropped_candidates: 0,
                reason: None,
            }
        }
        amari_rewrite::inverse::BackwardSearchOutcome::Partial(frontier) => {
            RewriteBackwardSearchOutput {
                outcome: "partial".to_owned(),
                steps: Vec::new(),
                exhaustion_authority: None,
                depth_reached: frontier.depth_reached,
                frontier: frontier
                    .states
                    .iter()
                    .map(|state| RewriteTerm::from_term(state.term()))
                    .collect(),
                dropped_candidates: 0,
                reason: None,
            }
        }
        amari_rewrite::inverse::BackwardSearchOutcome::Approximate(evidence) => {
            RewriteBackwardSearchOutput {
                outcome: "approximate".to_owned(),
                steps: Vec::new(),
                exhaustion_authority: None,
                depth_reached: evidence
                    .frontier
                    .as_ref()
                    .map(|frontier| frontier.depth_reached)
                    .unwrap_or(0),
                frontier: evidence
                    .frontier
                    .as_ref()
                    .map(|frontier| {
                        frontier
                            .states
                            .iter()
                            .map(|state| RewriteTerm::from_term(state.term()))
                            .collect()
                    })
                    .unwrap_or_default(),
                dropped_candidates: evidence.dropped_candidates,
                reason: Some(evidence.summary),
            }
        }
        amari_rewrite::inverse::BackwardSearchOutcome::Unsupported(unsupported) => {
            RewriteBackwardSearchOutput {
                outcome: "unsupported".to_owned(),
                steps: Vec::new(),
                exhaustion_authority: None,
                frontier: Vec::new(),
                depth_reached: 0,
                dropped_candidates: 0,
                reason: Some(unsupported.reason),
            }
        }
    };
    let bytes = validate_encoded_output(&output, bounds)?;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations: bytes,
            nodes: 0,
            iterations: 1,
            bytes: 0,
        },
        output: serde_json::to_value(output)?,
    })
}

#[cfg(feature = "standard-probes")]
fn execute_bidirectional_search(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: RewriteBidirectionalSearchRequest = serde_json::from_value(input.clone())
        .map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "bidirectional search request has an invalid term, rule, or limit shape: {error}"
            ))
        })?;
    let bounds = effective_bounds(limits);
    validate_rewrite_request(
        &request,
        [&request.source, &request.goal],
        &request.rules,
        bounds,
    )?;
    let rules = request
        .rules
        .iter()
        .map(RewriteRule::to_rule)
        .collect::<DiscoveryResult<Vec<_>>>()?;
    let system = TermSystem::new(rules);
    let source = request.source.to_term()?;
    let goal = request.goal.to_term()?;
    let config = search_config(request.max_depth)?;
    let outcome = amari_rewrite::inverse::BidirectionalExplorer::new(&system, config)
        .search(&source, &goal)
        .map_err(|error| {
            DiscoveryError::ProbeFailed(format!("bidirectional search failed: {error}"))
        })?;
    let output = match outcome {
        amari_rewrite::inverse::BidirectionalSearchOutcome::Witness(derivation) => {
            RewriteBidirectionalSearchOutput {
                outcome: "witness".to_owned(),
                forward_steps: derivation
                    .forward_steps
                    .iter()
                    .map(|step| RewriteForwardStep {
                        rule_id: step.rule_id.to_string(),
                        position: step
                            .position
                            .as_slice()
                            .iter()
                            .map(|index| *index as u64)
                            .collect(),
                        source_hash: step.source_hash.to_hex(),
                        result_hash: step.result_hash.to_hex(),
                    })
                    .collect(),
                backward_steps: derivation
                    .backward_steps
                    .iter()
                    .map(predecessor_dto)
                    .collect(),
                meeting_forward: Some(RewriteTerm::from_term(&derivation.meeting_forward)),
                meeting_backward: Some(RewriteTerm::from_term(&derivation.meeting_backward)),
                exhaustion_authority: None,
                frontier_forward: Vec::new(),
                frontier_backward: Vec::new(),
                depth_reached: (derivation.forward_steps.len() + derivation.backward_steps.len())
                    as u64,
                reason: None,
            }
        }
        amari_rewrite::inverse::BidirectionalSearchOutcome::Exhausted(certified) => {
            RewriteBidirectionalSearchOutput {
                outcome: "exhausted".to_owned(),
                forward_steps: Vec::new(),
                backward_steps: Vec::new(),
                meeting_forward: None,
                meeting_backward: None,
                exhaustion_authority: Some(certified.authority().wire_tag().to_owned()),
                frontier_forward: Vec::new(),
                frontier_backward: Vec::new(),
                depth_reached: 0,
                reason: None,
            }
        }
        amari_rewrite::inverse::BidirectionalSearchOutcome::Partial(frontier) => {
            RewriteBidirectionalSearchOutput {
                outcome: "partial".to_owned(),
                forward_steps: Vec::new(),
                backward_steps: Vec::new(),
                meeting_forward: None,
                meeting_backward: None,
                exhaustion_authority: None,
                frontier_forward: frontier
                    .forward
                    .iter()
                    .map(|state| RewriteTerm::from_term(state.term()))
                    .collect(),
                frontier_backward: frontier
                    .backward
                    .iter()
                    .map(|state| RewriteTerm::from_term(state.term()))
                    .collect(),
                depth_reached: frontier.depth_reached,
                reason: None,
            }
        }
        amari_rewrite::inverse::BidirectionalSearchOutcome::Unsupported(unsupported) => {
            RewriteBidirectionalSearchOutput {
                outcome: "unsupported".to_owned(),
                forward_steps: Vec::new(),
                backward_steps: Vec::new(),
                meeting_forward: None,
                meeting_backward: None,
                exhaustion_authority: None,
                frontier_forward: Vec::new(),
                frontier_backward: Vec::new(),
                depth_reached: 0,
                reason: Some(unsupported.reason),
            }
        }
    };
    let bytes = validate_encoded_output(&output, bounds)?;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations: bytes,
            nodes: 0,
            iterations: 1,
            bytes: 0,
        },
        output: serde_json::to_value(output)?,
    })
}

// ---- Regular tree language probe (0.25 Cohort 4, Task 22)

#[cfg(feature = "standard-probes")]
const MAX_AUTOMATON_STATES: usize = 64;
#[cfg(feature = "standard-probes")]
const MAX_AUTOMATON_TRANSITIONS: usize = 256;
#[cfg(feature = "standard-probes")]
const MAX_AUTOMATON_ALPHABET: usize = 64;
#[cfg(feature = "standard-probes")]
const MAX_GRAMMAR_TEXT_BYTES: usize = 8_192;

/// A ranked alphabet entry in a language-probe automaton.
#[derive(
    Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RewriteRankedSymbol {
    /// Symbol name.
    pub name: String,
    /// Symbol rank (number of children).
    pub arity: u16,
}

/// One bottom-up transition `symbol(children) -> parent`.
#[derive(
    Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RewriteAutomatonTransition {
    /// Transition symbol.
    pub symbol: String,
    /// Child states in positional order.
    pub children: Vec<String>,
    /// Parent (result) state.
    pub parent: String,
}

/// A checked finite tree automaton accepted by the languages probe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteAutomaton {
    /// Ranked alphabet.
    pub alphabet: Vec<RewriteRankedSymbol>,
    /// State names.
    pub states: Vec<String>,
    /// Bottom-up transitions.
    pub transitions: Vec<RewriteAutomatonTransition>,
    /// Final (accepting) states.
    pub finals: Vec<String>,
}

/// Typed input for the regular-tree-languages probe.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-languages/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        operation_known = "operation is membership, emptiness, witness, union, intersection, determinize, or minimize",
        exactly_one_source = "exactly one of automaton or grammar (fixed-syntax text) is accepted",
        automaton_bounds = "at most 64 states, 256 transitions, 64 alphabet symbols, and 16 children per transition",
        grammar_text_limit = "grammar text is at most 8192 bytes",
        name_bytes_limit = "names contain at most 256 bytes"
    ),
    example(
        label = "parity_membership",
        value = "{\"operation\":\"membership\",\"automaton\":{\"alphabet\":[{\"name\":\"z\",\"arity\":0},{\"name\":\"s\",\"arity\":1}],\"states\":[\"q0\",\"q1\"],\"transitions\":[{\"symbol\":\"z\",\"children\":[],\"parent\":\"q0\"},{\"symbol\":\"s\",\"children\":[\"q0\"],\"parent\":\"q1\"},{\"symbol\":\"s\",\"children\":[\"q1\"],\"parent\":\"q0\"}],\"finals\":[\"q0\"]},\"other\":null,\"term\":{\"kind\":\"symbol\",\"name\":\"s\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"z\",\"arguments\":[]}]},\"grammar\":null}"
    )
)]
pub struct RewriteLanguagesRequest {
    /// The operation to perform: membership, emptiness, witness,
    /// union, intersection, determinize, or minimize.
    pub operation: String,
    /// The automaton to operate on (exactly one of `automaton` /
    /// `grammar`).
    pub automaton: Option<RewriteAutomaton>,
    /// The second automaton for union/intersection.
    pub other: Option<RewriteAutomaton>,
    /// The term for membership (ground terms only).
    pub term: Option<RewriteTerm>,
    /// A grammar in the fixed text syntax, converted to an
    /// automaton before operating (exactly one of `automaton` /
    /// `grammar`).
    pub grammar: Option<String>,
}

/// Typed output of the languages probe: results and certificates
/// (canonical hashes), never source text.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-languages/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        certificates_only = "construction operations return the sha256 of the result's canonical bytes plus state/transition counts, never source text",
        witness_ground = "witness terms are ground and replay-accepted by the submitted automaton"
    ),
    example(
        label = "witness_z",
        value = "{\"outcome\":\"ok\",\"accepted\":null,\"language_empty\":null,\"witness\":{\"kind\":\"symbol\",\"name\":\"z\",\"arguments\":[]},\"result_hash\":null,\"result_states\":null,\"result_transitions\":null}"
    )
)]
pub struct RewriteLanguagesOutput {
    /// `ok` on success; invalid requests are typed errors instead.
    pub outcome: String,
    /// Membership verdict.
    pub accepted: Option<bool>,
    /// Emptiness verdict.
    pub language_empty: Option<bool>,
    /// The smallest witness term (absent when the language is
    /// empty).
    pub witness: Option<RewriteTerm>,
    /// sha256 hex of the result automaton's canonical bytes
    /// (union/intersection/determinize/minimize).
    pub result_hash: Option<String>,
    /// Result automaton state count.
    pub result_states: Option<u64>,
    /// Result automaton transition count.
    pub result_transitions: Option<u64>,
}

#[cfg(feature = "standard-probes")]
fn validate_automaton_dto(automaton: &RewriteAutomaton) -> DiscoveryResult<()> {
    if automaton.states.len() > MAX_AUTOMATON_STATES {
        return Err(DiscoveryError::InvalidInput(format!(
            "automaton declares {} states; the probe ceiling is {MAX_AUTOMATON_STATES}",
            automaton.states.len()
        )));
    }
    if automaton.transitions.len() > MAX_AUTOMATON_TRANSITIONS {
        return Err(DiscoveryError::InvalidInput(format!(
            "automaton declares {} transitions; the probe ceiling is {MAX_AUTOMATON_TRANSITIONS}",
            automaton.transitions.len()
        )));
    }
    if automaton.alphabet.len() > MAX_AUTOMATON_ALPHABET {
        return Err(DiscoveryError::InvalidInput(format!(
            "automaton declares {} symbols; the probe ceiling is {MAX_AUTOMATON_ALPHABET}",
            automaton.alphabet.len()
        )));
    }
    if automaton.finals.len() > automaton.states.len() {
        return Err(DiscoveryError::InvalidInput(
            "automaton declares more finals than states".to_owned(),
        ));
    }
    let mut names = automaton
        .states
        .iter()
        .map(String::as_str)
        .chain(automaton.finals.iter().map(String::as_str))
        .chain(automaton.alphabet.iter().map(|symbol| symbol.name.as_str()))
        .collect::<Vec<_>>();
    for transition in &automaton.transitions {
        names.push(transition.symbol.as_str());
        names.push(transition.parent.as_str());
        names.extend(transition.children.iter().map(String::as_str));
        if transition.children.len() > 16 {
            return Err(DiscoveryError::InvalidInput(format!(
                "transition {} has {} children; the probe ceiling is 16",
                transition.symbol,
                transition.children.len()
            )));
        }
    }
    for name in names {
        validate_name(name)?;
    }
    Ok(())
}

#[cfg(feature = "standard-probes")]
fn build_automaton(automaton: &RewriteAutomaton) -> DiscoveryResult<TreeAutomaton> {
    validate_automaton_dto(automaton)?;
    let alphabet = automaton
        .alphabet
        .iter()
        .map(|symbol| {
            RankedSymbol::new(
                amari_rewrite::trs::Symbol::new(symbol.name.clone()),
                symbol.arity,
            )
        })
        .collect();
    let states = automaton
        .states
        .iter()
        .map(|name| TreeState::new(amari_rewrite::trs::Symbol::new(name.clone())))
        .collect();
    let transitions = automaton
        .transitions
        .iter()
        .map(|transition| {
            TreeTransition::new(
                amari_rewrite::trs::Symbol::new(transition.symbol.clone()),
                transition
                    .children
                    .iter()
                    .map(|child| TreeState::new(amari_rewrite::trs::Symbol::new(child.clone())))
                    .collect(),
                TreeState::new(amari_rewrite::trs::Symbol::new(transition.parent.clone())),
            )
        })
        .collect();
    let finals = automaton
        .finals
        .iter()
        .map(|name| TreeState::new(amari_rewrite::trs::Symbol::new(name.clone())))
        .collect();
    TreeAutomaton::new(
        alphabet,
        states,
        transitions,
        finals,
        TreeAutomatonLimits::default(),
    )
    .map_err(|error| DiscoveryError::InvalidInput(format!("automaton is malformed: {error}")))
}

#[cfg(feature = "standard-probes")]
fn automaton_certificate(automaton: &TreeAutomaton) -> (String, u64, u64) {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(automaton.canonical_bytes());
    let hash = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    (
        hash,
        automaton.states().len() as u64,
        automaton.transitions().len() as u64,
    )
}

#[cfg(feature = "standard-probes")]
fn validate_grammar_derived_names(automaton: &TreeAutomaton) -> DiscoveryResult<()> {
    // The library grammar checks renderability, not the probe's name
    // ceiling; enforce the same rules on grammar-derived content so
    // witnesses replay through membership.
    for symbol in automaton.alphabet() {
        validate_name(symbol.symbol().as_str())?;
    }
    for state in automaton.states() {
        validate_name(state.name().as_str())?;
    }
    Ok(())
}

#[cfg(feature = "standard-probes")]
fn language_resources(limits: &EffectiveProbeLimits) -> DiscoveryResult<RelationResources> {
    // Build the library budget from the caller-tightened engine
    // limits, clamped into the library's ceiling profile. Operations
    // then run under budget enforcement DURING execution, and the
    // probe reports the library's actual charge counts.
    fn clamp(value: u64, ceiling: usize) -> usize {
        usize::try_from(value)
            .unwrap_or(usize::MAX)
            .clamp(1, ceiling)
    }
    let relation_limits = RelationLimits::new(
        clamp(limits.max_nodes, RelationLimits::MAX_TERM_NODES),
        RelationLimits::MAX_TERM_DEPTH,
        RelationLimits::MAX_CONSTRAINTS,
        clamp(limits.max_operations, RelationLimits::MAX_OPERATIONS),
    )
    .map_err(|error| DiscoveryError::InvalidInput(format!("probe limit profile: {error}")))?;
    Ok(RelationResources::new(&relation_limits))
}

#[cfg(feature = "standard-probes")]
fn language_error(operation: &str, error: RewriteError) -> DiscoveryError {
    // Resource conditions stay limit errors (exit 7); malformed or
    // out-of-domain inputs remain invalid-input (exit 2).
    match error {
        RewriteError::RelationLimitExceeded { .. } | RewriteError::InvalidLimit { .. } => {
            DiscoveryError::LimitExceeded(format!("{operation}: {error}"))
        }
        other => DiscoveryError::InvalidInput(format!("{operation}: {other}")),
    }
}

#[cfg(feature = "standard-probes")]
fn execute_languages(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: RewriteLanguagesRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "languages request has an invalid operation, automaton, term, or grammar shape: {error}"
            ))
        })?;
    let bounds = effective_bounds(limits);
    // Exactly one source: an automaton DTO or grammar text.
    let source = match (&request.automaton, &request.grammar) {
        (Some(automaton), None) => build_automaton(automaton)?,
        (None, Some(grammar)) => {
            if grammar.len() > MAX_GRAMMAR_TEXT_BYTES {
                return Err(DiscoveryError::InvalidInput(format!(
                    "grammar text is {} bytes; the probe ceiling is {MAX_GRAMMAR_TEXT_BYTES}",
                    grammar.len()
                )));
            }
            let automaton = RegularTreeGrammar::parse(grammar, &TreeAutomatonLimits::default())
                .map_err(|error| language_error("grammar", error))?
                .to_automaton()
                .map_err(|error| language_error("grammar conversion", error))?;
            validate_grammar_derived_names(&automaton)?;
            // The wire contract's automaton bounds apply to the
            // CONVERTED automaton as well: grammar text is compact
            // enough to declare far more states than the DTO path
            // admits.
            if automaton.states().len() > MAX_AUTOMATON_STATES {
                return Err(DiscoveryError::InvalidInput(format!(
                    "grammar-derived automaton declares {} states; the probe ceiling is {MAX_AUTOMATON_STATES}",
                    automaton.states().len()
                )));
            }
            if automaton.transitions().len() > MAX_AUTOMATON_TRANSITIONS {
                return Err(DiscoveryError::InvalidInput(format!(
                    "grammar-derived automaton declares {} transitions; the probe ceiling is {MAX_AUTOMATON_TRANSITIONS}",
                    automaton.transitions().len()
                )));
            }
            if automaton.alphabet().len() > MAX_AUTOMATON_ALPHABET {
                return Err(DiscoveryError::InvalidInput(format!(
                    "grammar-derived automaton declares {} symbols; the probe ceiling is {MAX_AUTOMATON_ALPHABET}",
                    automaton.alphabet().len()
                )));
            }
            automaton
        }
        (Some(_), Some(_)) => {
            return Err(DiscoveryError::InvalidInput(
                "exactly one of `automaton` and `grammar` is accepted".to_owned(),
            ));
        }
        (None, None) => {
            return Err(DiscoveryError::InvalidInput(
                "one of `automaton` and `grammar` is required".to_owned(),
            ));
        }
    };
    let source_states = source.states().len() as u64;
    let certificate_output = |automaton: &TreeAutomaton, operations: u64| {
        let (hash, states, transitions) = automaton_certificate(automaton);
        (
            RewriteLanguagesOutput {
                outcome: "ok".to_owned(),
                accepted: None,
                language_empty: None,
                witness: None,
                result_hash: Some(hash),
                result_states: Some(states),
                result_transitions: Some(transitions),
            },
            operations,
            states,
        )
    };
    let decision_output = |accepted, language_empty, witness, operations, nodes| {
        (
            RewriteLanguagesOutput {
                outcome: "ok".to_owned(),
                accepted,
                language_empty,
                witness,
                result_hash: None,
                result_states: None,
                result_transitions: None,
            },
            operations,
            nodes,
        )
    };
    let (output, operations, nodes) = match request.operation.as_str() {
        "membership" => {
            let term = request.term.as_ref().ok_or_else(|| {
                DiscoveryError::InvalidInput("membership requires a `term`".to_owned())
            })?;
            // The run visits every node of the term exactly once, so
            // the term's size IS the operation count; enforce the
            // caller-tightened budgets before conversion.
            let stats = term_stats(term, bounds.max_term_depth, bounds.max_term_nodes)?;
            let term_nodes = stats.nodes;
            let term = term.to_term()?;
            let mut resources = language_resources(limits)?;
            let accepted = source
                .accepts_with_resources(&term, &mut resources)
                .map_err(|error| language_error("membership", error))?;
            // accepting_run charges one operation per transition at
            // every term node; the reported count is the library's
            // own.
            decision_output(
                Some(accepted),
                None,
                None,
                resources.operations() as u64,
                term_nodes,
            )
        }
        "emptiness" => {
            let mut resources = language_resources(limits)?;
            let empty = source
                .language_is_empty_with_resources(&mut resources)
                .map_err(|error| language_error("emptiness", error))?;
            decision_output(
                None,
                Some(empty),
                None,
                resources.operations() as u64,
                source_states,
            )
        }
        "witness" => {
            let mut resources = language_resources(limits)?;
            let witness = source
                .witness_with_resources(&mut resources)
                .map_err(|error| language_error("witness", error))?
                .map(|term| RewriteTerm::from_term(&term));
            let witness_nodes = witness
                .as_ref()
                .map(|term| {
                    term_stats(term, bounds.max_term_depth, bounds.max_term_nodes)
                        .map(|stats| stats.nodes)
                })
                .transpose()?
                .unwrap_or(0);
            decision_output(
                None,
                None,
                witness,
                (resources.operations() as u64).saturating_add(witness_nodes),
                witness_nodes,
            )
        }
        "union" | "intersection" => {
            let other = request.other.as_ref().ok_or_else(|| {
                DiscoveryError::InvalidInput(format!(
                    "{} requires a second automaton `other`",
                    request.operation
                ))
            })?;
            let other = build_automaton(other)?;
            let mut resources = language_resources(limits)?;
            let result = if request.operation == "union" {
                source.union_with_resources(&other, &TreeAutomatonLimits::default(), &mut resources)
            } else {
                source.intersection_with_resources(
                    &other,
                    &TreeAutomatonLimits::default(),
                    &mut resources,
                )
            }
            .map_err(|error| language_error(&request.operation, error))?;
            certificate_output(&result, resources.operations() as u64)
        }
        "determinize" => {
            let mut resources = language_resources(limits)?;
            let result = source
                .determinize_with_resources(&TreeAutomatonLimits::default(), &mut resources)
                .map_err(|error| language_error("determinize", error))?;
            certificate_output(&result, resources.operations() as u64)
        }
        "minimize" => {
            let mut resources = language_resources(limits)?;
            let result = source
                .minimized_with_resources(&mut resources)
                .map_err(|error| language_error("minimize", error))?;
            certificate_output(&result, resources.operations() as u64)
        }
        other => {
            return Err(DiscoveryError::InvalidInput(format!(
                "unknown operation {other:?}: expected membership, emptiness, witness, union, intersection, determinize, or minimize"
            )));
        }
    };
    let bytes = validate_encoded_output(&output, bounds)?;
    let _ = bytes;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations,
            nodes,
            iterations: 1,
            bytes: 0,
        },
        output: serde_json::to_value(output)?,
    })
}

#[cfg(feature = "standard-probes")]
pub(super) fn languages_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:languages:v1".parse()?,
        capability_id: "amari:amari-rewrite:language:automata".parse()?,
        input_schema: "amari.discovery/probe/rewrite-languages/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-languages/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_languages,
    })
}

// ---- Regular language preimage probe (0.25 Cohort 5, Task 29)

/// Probe-tightened rule-count ceiling (library default is 256).
#[cfg(feature = "standard-probes")]
const MAX_PREIMAGE_RULES: u64 = 32;
/// Probe-tightened automaton state ceiling (library default 4096).
#[cfg(feature = "standard-probes")]
const MAX_PREIMAGE_AUTOMATON_STATES: usize = 8;
/// Probe-tightened automaton transition ceiling (library default 65536).
#[cfg(feature = "standard-probes")]
const MAX_PREIMAGE_AUTOMATON_TRANSITIONS: usize = 32;
/// Probe-tightened finite-horizon ceiling (library default 64).
#[cfg(feature = "standard-probes")]
const MAX_PREIMAGE_HORIZON: u64 = 4;
/// Probe-tightened per-term node ceiling (library default 4096).
#[cfg(feature = "standard-probes")]
const MAX_PREIMAGE_TERM_NODES: u64 = 32;
/// Probe-tightened per-term depth ceiling (library default 64).
#[cfg(feature = "standard-probes")]
const MAX_PREIMAGE_TERM_DEPTH: u64 = 16;
/// Probe-tightened constraint ceiling (library default 4096).
#[cfg(feature = "standard-probes")]
const MAX_PREIMAGE_CONSTRAINTS: u64 = 128;
/// Probe-tightened operation ceiling (library default 1000000).
#[cfg(feature = "standard-probes")]
const MAX_PREIMAGE_OPERATIONS: u64 = 20_000;
/// Fixed witnessed-replay horizon for a saturation approximation.
#[cfg(feature = "standard-probes")]
const MAX_PREIMAGE_STEPS: u32 = 4;
/// Digest frame for one witnessed ground term.
#[cfg(feature = "standard-probes")]
const PREIMAGE_WITNESS_FRAME: &str = "amari-discovery/rewrite-preimages/witness/v1";

/// Typed input for the regular-language preimage probe.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-preimages/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        automaton_bounds = "at most 8 states, 32 transitions, 64 alphabet symbols, and 16 children per transition",
        horizon_limit = "horizon is no greater than 4",
        name_bytes_limit = "names contain at most 256 bytes",
        operation_known = "operation is classify, preimage, approximate, or membership",
        preimage_known = "preimage is one_step, finite_horizon, or saturation",
        rules_checked = "every rule RHS variable occurs in its LHS",
        rules_count_limit = "at most 32 ordered rules are accepted",
        term_bounds = "terms have depth at most 16 and at most 32 nodes"
    ),
    example(
        label = "one_step_membership",
        value = "{\"operation\":\"membership\",\"preimage\":\"one_step\",\"rules\":[{\"lhs\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},\"rhs\":{\"kind\":\"symbol\",\"name\":\"b\",\"arguments\":[]}}],\"language\":{\"alphabet\":[{\"name\":\"a\",\"arity\":0},{\"name\":\"b\",\"arity\":0}],\"states\":[\"q0\"],\"transitions\":[{\"symbol\":\"b\",\"children\":[],\"parent\":\"q0\"}],\"finals\":[\"q0\"]},\"term\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},\"horizon\":null}"
    )
)]
pub struct RewritePreimagesRequest {
    /// The operation: `classify`, `preimage`, `approximate`, or
    /// `membership`.
    pub operation: String,
    /// The preimage operation the request answers: `one_step`,
    /// `finite_horizon`, or `saturation`. Defaults to `one_step`.
    pub preimage: Option<String>,
    /// Ordered checked rewrite rules forming the system.
    pub rules: Vec<RewriteRule>,
    /// The target language automaton (required for `preimage`,
    /// `approximate`, and `membership`).
    pub language: Option<RewriteAutomaton>,
    /// The query term for `membership` (required there).
    pub term: Option<RewriteTerm>,
    /// The horizon for `finite_horizon` (required there, 0..=4; zero
    /// is the identity construction). Every supplied bounded field is
    /// validated against the probe ceilings even when the selected
    /// operation does not consume it.
    pub horizon: Option<u64>,
}

/// Typed output of the regular-language preimage probe: typed
/// outcomes and canonical digests, never source automata.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-preimages/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        certificates_only = "preimage results report the result digest and state/transition counts, never the source automaton",
        membership_three_valued = "verdict is proven, excluded, or unknown",
        witness_digests_truthful = "witness_hashes has exactly witness_count entries"
    ),
    example(
        label = "one_step_membership",
        value = "{\"outcome\":\"ok\",\"class\":\"ground\",\"capability\":\"exact\",\"construction\":\"left_linear_one_step\",\"unsupported_reasons\":[],\"result_hash\":null,\"result_states\":null,\"result_transitions\":null,\"certificate_hash\":null,\"witness_count\":null,\"witness_hashes\":[],\"truncated\":null,\"verdict\":\"proven\"}"
    )
)]
pub struct RewritePreimagesOutput {
    /// `ok` for classify/membership, `exact` for an exact preimage,
    /// `bounded` for a witnessed lower bound.
    pub outcome: String,
    /// The classified TRS class.
    pub class: String,
    /// `exact` or `approximation_only`.
    pub capability: String,
    /// The approved exact construction, or `witnessed_lower_bound`.
    pub construction: Option<String>,
    /// Why the selected cell has no exact construction.
    pub unsupported_reasons: Vec<String>,
    /// sha256 hex of the result automaton's canonical bytes.
    pub result_hash: Option<String>,
    /// Result automaton state count.
    pub result_states: Option<u64>,
    /// Result automaton transition count.
    pub result_transitions: Option<u64>,
    /// sha256 hex of the certificate's binding fields.
    pub certificate_hash: Option<String>,
    /// Admitted witness count for `approximate`.
    pub witness_count: Option<u64>,
    /// Canonical digest of each admitted witness.
    pub witness_hashes: Vec<String>,
    /// Whether the approximation enumeration was truncated.
    pub truncated: Option<bool>,
    /// The three-valued membership verdict.
    pub verdict: Option<String>,
}

#[cfg(feature = "standard-probes")]
fn parse_preimage_operation(
    request: &RewritePreimagesRequest,
) -> DiscoveryResult<PreimageOperation> {
    match request.preimage.as_deref().unwrap_or("one_step") {
        "one_step" => Ok(PreimageOperation::OneStep),
        "finite_horizon" => {
            let horizon = request.horizon.ok_or_else(|| {
                DiscoveryError::InvalidInput(
                    "the finite_horizon preimage requires a `horizon`".to_owned(),
                )
            })?;
            // Zero is the identity construction (Task 24): the
            // library supports it, so the probe does too
            // (PR #286 round 1).
            if horizon > MAX_PREIMAGE_HORIZON {
                return Err(DiscoveryError::InvalidInput(format!(
                    "finite_horizon horizon {horizon} is outside 0..={MAX_PREIMAGE_HORIZON}"
                )));
            }
            let horizon = u32::try_from(horizon).map_err(|_| {
                DiscoveryError::LimitExceeded("preimage horizon overflow".to_owned())
            })?;
            Ok(PreimageOperation::FiniteHorizon(horizon))
        }
        "saturation" => Ok(PreimageOperation::Saturation),
        other => Err(DiscoveryError::InvalidInput(format!(
            "unknown preimage {other:?}: expected one_step, finite_horizon, or saturation"
        ))),
    }
}

#[cfg(feature = "standard-probes")]
fn class_name(class: TrsClass) -> &'static str {
    match class {
        TrsClass::Ground => "ground",
        TrsClass::LinearVariableDisjoint => "linear_variable_disjoint",
        TrsClass::LeftLinearShared => "left_linear_shared",
        TrsClass::NonLeftLinear => "non_left_linear",
    }
}

#[cfg(feature = "standard-probes")]
fn capability_name(capability: PreimageCapability) -> &'static str {
    match capability {
        PreimageCapability::Exact(_) => "exact",
        PreimageCapability::ApproximationOnly => "approximation_only",
    }
}

#[cfg(feature = "standard-probes")]
fn preimage_operation_name(operation: PreimageOperation) -> &'static str {
    match operation {
        PreimageOperation::OneStep => "one_step",
        PreimageOperation::FiniteHorizon(_) => "finite_horizon",
        PreimageOperation::Saturation => "saturation",
    }
}

#[cfg(feature = "standard-probes")]
fn preimage_construction_name(construction: PreimageConstruction) -> &'static str {
    match construction {
        PreimageConstruction::Identity => "identity",
        PreimageConstruction::LeftLinearOneStep => "left_linear_one_step",
        PreimageConstruction::FiniteHorizonIteration(_) => "finite_horizon_iteration",
        PreimageConstruction::GttSaturation => "gtt_saturation",
        PreimageConstruction::WitnessedLowerBound => "witnessed_lower_bound",
    }
}

#[cfg(feature = "standard-probes")]
fn membership_verdict_name(verdict: &MembershipVerdict) -> &'static str {
    match verdict {
        MembershipVerdict::Proven(_) => "proven",
        MembershipVerdict::Excluded => "excluded",
        MembershipVerdict::Unknown => "unknown",
    }
}

#[cfg(feature = "standard-probes")]
fn preimage_output_base(class: &str, capability: &str) -> RewritePreimagesOutput {
    RewritePreimagesOutput {
        outcome: "ok".to_owned(),
        class: class.to_owned(),
        capability: capability.to_owned(),
        construction: None,
        unsupported_reasons: Vec::new(),
        result_hash: None,
        result_states: None,
        result_transitions: None,
        certificate_hash: None,
        witness_count: None,
        witness_hashes: Vec::new(),
        truncated: None,
        verdict: None,
    }
}

#[cfg(feature = "standard-probes")]
fn validate_preimage_automaton_bounds(automaton: &RewriteAutomaton) -> DiscoveryResult<()> {
    validate_automaton_dto(automaton)?;
    if automaton.states.len() > MAX_PREIMAGE_AUTOMATON_STATES {
        return Err(DiscoveryError::InvalidInput(format!(
            "automaton declares {} states; the preimage probe ceiling is \
             {MAX_PREIMAGE_AUTOMATON_STATES}",
            automaton.states.len()
        )));
    }
    if automaton.transitions.len() > MAX_PREIMAGE_AUTOMATON_TRANSITIONS {
        return Err(DiscoveryError::InvalidInput(format!(
            "automaton declares {} transitions; the preimage probe ceiling is \
             {MAX_PREIMAGE_AUTOMATON_TRANSITIONS}",
            automaton.transitions.len()
        )));
    }
    Ok(())
}

#[cfg(feature = "standard-probes")]
fn require_preimage_language(request: &RewritePreimagesRequest) -> DiscoveryResult<TreeAutomaton> {
    let automaton = request.language.as_ref().ok_or_else(|| {
        DiscoveryError::InvalidInput(format!(
            "{} requires a `language` automaton",
            request.operation
        ))
    })?;
    validate_preimage_automaton_bounds(automaton)?;
    build_automaton(automaton)
}

#[cfg(feature = "standard-probes")]
fn preimage_relation_limits(limits: &EffectiveProbeLimits) -> DiscoveryResult<RelationLimits> {
    fn clamp(value: u64, ceiling: u64) -> usize {
        usize::try_from(value.min(ceiling)).unwrap_or(1).max(1)
    }
    RelationLimits::new(
        clamp(limits.max_nodes, MAX_PREIMAGE_TERM_NODES),
        MAX_PREIMAGE_TERM_DEPTH as usize,
        clamp(limits.max_nodes, MAX_PREIMAGE_CONSTRAINTS),
        clamp(limits.max_operations, MAX_PREIMAGE_OPERATIONS),
    )
    .map_err(|error| DiscoveryError::InvalidInput(format!("preimage limit profile: {error}")))
}

#[cfg(feature = "standard-probes")]
fn preimage_automaton_limits() -> DiscoveryResult<TreeAutomatonLimits> {
    TreeAutomatonLimits::new(
        MAX_PREIMAGE_AUTOMATON_STATES,
        MAX_PREIMAGE_AUTOMATON_TRANSITIONS,
        TreeAutomatonLimits::MAX_RANK,
    )
    .map_err(|error| {
        DiscoveryError::InvalidInput(format!("preimage automaton limit profile: {error}"))
    })
}

#[cfg(feature = "standard-probes")]
fn run_exact_preimage(
    system: &TermSystem,
    language: &TreeAutomaton,
    operation: PreimageOperation,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
) -> Result<PreimageOutcome, RewriteError> {
    match operation {
        PreimageOperation::OneStep => one_step_preimage(system, language, limits, automaton_limits),
        PreimageOperation::FiniteHorizon(horizon) => {
            finite_horizon_preimage(system, language, horizon, limits, automaton_limits)
        }
        PreimageOperation::Saturation => {
            saturation_preimage(system, language, limits, automaton_limits)
        }
    }
}

#[cfg(feature = "standard-probes")]
fn run_lower_bound(
    system: &TermSystem,
    language: &TreeAutomaton,
    operation: PreimageOperation,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
) -> Result<ApproximationOutcome, RewriteError> {
    match operation {
        PreimageOperation::OneStep => {
            one_step_lower_bound(system, language, limits, automaton_limits)
        }
        PreimageOperation::FiniteHorizon(horizon) => {
            finite_horizon_lower_bound(system, language, horizon, limits, automaton_limits)
        }
        PreimageOperation::Saturation => saturation_lower_bound(
            system,
            language,
            MAX_PREIMAGE_STEPS,
            limits,
            automaton_limits,
        ),
    }
}

/// Canonical digest of a certificate's COMPLETE binding fields,
/// including the recorded limit profile: delegated to the library so
/// the probe's digest is exactly the certificate's evidence identity
/// (cohort 5 closeout F1). NOTE: the values of this field changed
/// when the limit profile became part of the digest.
#[cfg(feature = "standard-probes")]
fn certificate_digest(certificate: &PreimageCertificate) -> String {
    certificate
        .binding_digest()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(feature = "standard-probes")]
fn execute_preimages(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: RewritePreimagesRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "preimages request has an invalid operation, preimage, rule, automaton, term, \
                 or horizon shape: {error}"
            ))
        })?;
    let rule_count = u64::try_from(request.rules.len())
        .map_err(|_| DiscoveryError::LimitExceeded("preimage rule count overflow".to_owned()))?;
    enforce("preimage rule count", rule_count, MAX_PREIMAGE_RULES)?;
    for rule in &request.rules {
        term_stats(&rule.lhs, MAX_PREIMAGE_TERM_DEPTH, MAX_PREIMAGE_TERM_NODES)?;
        term_stats(&rule.rhs, MAX_PREIMAGE_TERM_DEPTH, MAX_PREIMAGE_TERM_NODES)?;
    }
    // Every SUPPLIED bounded field is validated before dispatch —
    // including fields the selected operation does not consume — so a
    // request can never carry an over-ceiling payload past the gate
    // (PR #286 round 1).
    if let Some(horizon) = request.horizon {
        if horizon > MAX_PREIMAGE_HORIZON {
            return Err(DiscoveryError::InvalidInput(format!(
                "horizon {horizon} is outside 0..={MAX_PREIMAGE_HORIZON}"
            )));
        }
    }
    if let Some(term) = &request.term {
        term_stats(term, MAX_PREIMAGE_TERM_DEPTH, MAX_PREIMAGE_TERM_NODES)?;
    }
    if let Some(language) = &request.language {
        validate_preimage_automaton_bounds(language)?;
    }
    let rules = request
        .rules
        .iter()
        .map(RewriteRule::to_rule)
        .collect::<DiscoveryResult<Vec<_>>>()?;
    let system = TermSystem::new(rules);
    let operation = parse_preimage_operation(&request)?;
    let classification =
        classify_system(&system).map_err(|error| language_error("classify", error))?;
    let capability = classification.capability(operation);
    let class = class_name(classification.class());
    let capability_label = capability_name(capability);

    let (output, operations, nodes) = match request.operation.as_str() {
        "classify" => {
            let construction = match capability {
                PreimageCapability::Exact(construction) => construction,
                PreimageCapability::ApproximationOnly => PreimageConstruction::WitnessedLowerBound,
            };
            let mut output = preimage_output_base(class, capability_label);
            output.construction = Some(preimage_construction_name(construction).to_owned());
            if matches!(capability, PreimageCapability::ApproximationOnly) {
                output.unsupported_reasons.push(format!(
                    "class {class} has no approved exact construction for {} (ADR 0001)",
                    preimage_operation_name(operation)
                ));
            }
            (output, rule_count, 0_u64)
        }
        "preimage" => {
            let language = require_preimage_language(&request)?;
            let relation_limits = preimage_relation_limits(limits)?;
            let automaton_limits = preimage_automaton_limits()?;
            let outcome = run_exact_preimage(
                &system,
                &language,
                operation,
                &relation_limits,
                &automaton_limits,
            )
            .map_err(|error| language_error("preimage", error))?;
            let (result, certificate) = outcome.into_parts();
            let (hash, states, transitions) = automaton_certificate(&result);
            let mut output = preimage_output_base(class, "exact");
            output.outcome = "exact".to_owned();
            output.construction =
                Some(preimage_construction_name(certificate.construction()).to_owned());
            output.result_hash = Some(hash);
            output.result_states = Some(states);
            output.result_transitions = Some(transitions);
            output.certificate_hash = Some(certificate_digest(&certificate));
            let operations = rule_count
                .saturating_add(states)
                .saturating_add(transitions);
            (output, operations, states)
        }
        "approximate" => {
            let language = require_preimage_language(&request)?;
            let relation_limits = preimage_relation_limits(limits)?;
            let automaton_limits = preimage_automaton_limits()?;
            let outcome = run_lower_bound(
                &system,
                &language,
                operation,
                &relation_limits,
                &automaton_limits,
            )
            .map_err(|error| language_error("approximate", error))?;
            let witness_hashes: Vec<String> = outcome
                .witnesses()
                .iter()
                .map(|term| Sha256Digest::canonical_term(PREIMAGE_WITNESS_FRAME, term).to_hex())
                .collect();
            let truncated = outcome
                .trace()
                .iter()
                .any(|event| matches!(event, ApproximationEvent::EnumerationTruncated { .. }));
            let witness_count = u64::try_from(witness_hashes.len())
                .map_err(|_| DiscoveryError::LimitExceeded("witness count overflow".to_owned()))?;
            let mut output = preimage_output_base(class, "approximation_only");
            output.outcome = "bounded".to_owned();
            output.construction = Some("witnessed_lower_bound".to_owned());
            output.witness_count = Some(witness_count);
            output.witness_hashes = witness_hashes;
            output.truncated = Some(truncated);
            output.certificate_hash = Some(certificate_digest(outcome.certificate()));
            let operations = rule_count.saturating_add(witness_count);
            (output, operations, witness_count)
        }
        "membership" => {
            let language = require_preimage_language(&request)?;
            let term = request.term.as_ref().ok_or_else(|| {
                DiscoveryError::InvalidInput("membership requires a `term`".to_owned())
            })?;
            let stats = term_stats(term, MAX_PREIMAGE_TERM_DEPTH, MAX_PREIMAGE_TERM_NODES)?;
            let term = term.to_term()?;
            let relation_limits = preimage_relation_limits(limits)?;
            let automaton_limits = preimage_automaton_limits()?;
            let query = classify_and_query(
                &system,
                &language,
                operation,
                &term,
                &relation_limits,
                &automaton_limits,
            )
            .map_err(|error| language_error("membership", error))?;
            let mut output = preimage_output_base(class, capability_label);
            output.verdict = Some(membership_verdict_name(query.verdict()).to_owned());
            output.certificate_hash = Some(certificate_digest(query.certificate()));
            (output, rule_count.saturating_add(1), stats.nodes)
        }
        other => {
            return Err(DiscoveryError::InvalidInput(format!(
                "unknown operation {other:?}: expected classify, preimage, approximate, or \
                 membership"
            )));
        }
    };

    let bounds = effective_bounds(limits);
    let _ = validate_encoded_output(&output, bounds)?;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations,
            nodes,
            iterations: 1,
            bytes: 0,
        },
        output: serde_json::to_value(output)?,
    })
}

#[cfg(feature = "standard-probes")]
pub(super) fn preimages_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:preimages:v1".parse()?,
        capability_id: "amari:amari-rewrite:language:preimages".parse()?,
        input_schema: "amari.discovery/probe/rewrite-preimages/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-preimages/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_preimages,
    })
}

// ============================================================================
// Cohort 6 — bounded analysis/completion discovery surface (0.25 Task 35)
// ============================================================================

/// Probe-tightened ordered-rule ceiling for the analysis/completion
/// probes.
#[cfg(feature = "standard-probes")]
const MAX_ANALYSIS_RULES: u64 = 32;
/// Probe-tightened per-term node ceiling (library default 4096).
#[cfg(feature = "standard-probes")]
const MAX_ANALYSIS_TERM_NODES: u64 = 64;
/// Probe-tightened per-term depth ceiling (library default 64).
#[cfg(feature = "standard-probes")]
const MAX_ANALYSIS_TERM_DEPTH: u64 = 16;
/// Probe-tightened operation ceiling (library default 1000000).
#[cfg(feature = "standard-probes")]
const MAX_ANALYSIS_OPERATIONS: usize = 20_000;
/// Maximum precedence symbols a request may declare.
#[cfg(feature = "standard-probes")]
const MAX_ANALYSIS_PRECEDENCE_SYMBOLS: usize = 16;
/// Maximum arity a declared precedence symbol may claim.
#[cfg(feature = "standard-probes")]
const MAX_ANALYSIS_ARITY: u64 = 8;
/// Maximum pair entries reported in one analysis output.
#[cfg(feature = "standard-probes")]
const MAX_ANALYSIS_PAIRS: usize = 8;
/// Maximum trace steps reported in one completion output.
#[cfg(feature = "standard-probes")]
const MAX_ANALYSIS_TRACE_STEPS: usize = 8;
/// Probe-tightened guided-inverse search depth ceiling.
#[cfg(feature = "standard-probes")]
const MAX_COMPLETION_SEARCH_DEPTH: u64 = 8;
/// Maximum rejected candidate terms reported in one output.
#[cfg(feature = "standard-probes")]
const MAX_COMPLETION_REJECTIONS: usize = 8;
/// Record a library-domain failure as a typed outcome: the probe ran
/// within its bounds and the library declined; the meter observations
/// survive in the returned output (PR #316 round 1 P2).
#[cfg(feature = "standard-probes")]
fn record_domain_failure(
    outcome: &mut String,
    reason: &mut Option<String>,
    operation: &str,
    error: &RewriteError,
) {
    *outcome = "error".to_owned();
    *reason = Some(format!("{operation}: {error}"));
}

/// One ranked function symbol in a caller-supplied LPO precedence.
///
/// Entries are ordered highest precedence first, matching
/// [`amari_rewrite::analysis::Precedence::new`]. The declared arity
/// must agree with every use of the symbol in the request system.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewritePrecedenceSymbol {
    /// Function symbol name.
    pub name: String,
    /// Declared arity, 0..=8.
    pub arity: u64,
}

/// Typed input for the bounded analysis probe.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-analysis/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        operation_known = "operation is critical_pairs, confluence, or termination",
        precedence_symbols_limit = "at most 16 precedence symbols with arity at most 8",
        precedence_required = "termination requires a precedence that covers every used symbol at its arity",
        rules_checked = "every rule RHS variable occurs in its LHS",
        rules_count_limit = "at most 32 ordered rules are accepted",
        term_bounds = "terms have depth at most 16 and at most 64 nodes",
        term_name_bytes_limit = "variable and symbol names contain at most 256 bytes"
    ),
    example(
        label = "root_overlap_critical_pairs",
        value = "{\"operation\":\"critical_pairs\",\"rules\":[{\"lhs\":{\"kind\":\"symbol\",\"name\":\"f\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"x\"}]},\"rhs\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]}},{\"lhs\":{\"kind\":\"symbol\",\"name\":\"f\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"x\"}]},\"rhs\":{\"kind\":\"symbol\",\"name\":\"b\",\"arguments\":[]}}],\"precedence\":null}"
    )
)]
pub struct RewriteAnalysisRequest {
    /// The operation: `critical_pairs`, `confluence`, or
    /// `termination`.
    pub operation: String,
    /// Ordered checked rewrite rules forming the system.
    pub rules: Vec<RewriteRule>,
    /// The LPO precedence (required for `termination`; validated
    /// against the probe ceilings whenever supplied).
    pub precedence: Option<Vec<RewritePrecedenceSymbol>>,
}

/// One bounded critical-pair or joinability entry.
///
/// `outcome` is `overlap` for critical-pair enumeration, and
/// `joinable` or `not_joinable` for local-confluence analysis.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteAnalysisPair {
    /// Index of the overlapped (outer) rule.
    pub left_rule: u64,
    /// Index of the rule unified into the overlap position.
    pub right_rule: u64,
    /// Overlap position inside the outer rule's left-hand side.
    pub path: Vec<u64>,
    /// `overlap`, `joinable`, or `not_joinable`.
    pub outcome: String,
    /// The unified outer left-hand side (critical-pair entries).
    pub overlap: Option<RewriteTerm>,
    /// The outer rule's instantiated right-hand side.
    pub root_step: Option<RewriteTerm>,
    /// The outer left-hand side rewritten at `path`.
    pub position_step: Option<RewriteTerm>,
    /// The common reduct for a joinable pair.
    pub join_term: Option<RewriteTerm>,
}

/// Per-system counts carried by a termination proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteTerminationSummary {
    /// Number of rules examined.
    pub rules_checked: u64,
    /// Number of rules proved strictly decreasing.
    pub decreasing_rules: u64,
}

/// Typed output of the bounded analysis probe.
///
/// Pair evidence is capped at eight entries; `pair_count` always
/// reports the full enumeration count and `pairs_truncated` says
/// whether entries were withheld.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-analysis/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        bounded_pair_evidence = "at most 8 pair entries are reported; pair_count is the full enumeration count and pairs_truncated reports withheld entries",
        confluence_certified_honestly = "locally_confluent is true only when every pair joins and no rule blocks certification",
        termination_certificate = "termination is proved_terminating only when every rule strictly decreases",
        evidence_only = "outputs report terms, indices, and proof status, never an unbounded derivation set"
    ),
    example(
        label = "root_overlap_critical_pairs",
        value = "{\"outcome\":\"ok\",\"operation\":\"critical_pairs\",\"rule_count\":2,\"pair_count\":1,\"trivial_excluded\":0,\"non_left_linear_rules\":[],\"pairs\":[{\"left_rule\":0,\"right_rule\":1,\"path\":[],\"outcome\":\"overlap\",\"overlap\":{\"kind\":\"symbol\",\"name\":\"f\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"x\"}]},\"root_step\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},\"position_step\":{\"kind\":\"symbol\",\"name\":\"b\",\"arguments\":[]},\"join_term\":null}],\"pairs_truncated\":false,\"locally_confluent\":null,\"certification_blocked\":[],\"joinable_count\":null,\"not_joinable_count\":null,\"termination\":null,\"termination_report\":null,\"failing_rule\":null,\"failure_reason\":null}"
    )
)]
pub struct RewriteAnalysisOutput {
    /// `ok` for a completed analysis, or `error` when the library
    /// declined within bounds (see `failure_reason`).
    pub outcome: String,
    /// The dispatched operation.
    pub operation: String,
    /// Number of input rules.
    pub rule_count: u64,
    /// Full critical-pair count (critical-pair and confluence
    /// operations).
    pub pair_count: Option<u64>,
    /// Overlaps excluded because both sides were structurally equal.
    pub trivial_excluded: Option<u64>,
    /// Indices of rules whose left-hand side is not left-linear.
    pub non_left_linear_rules: Vec<u64>,
    /// Bounded pair evidence.
    pub pairs: Vec<RewriteAnalysisPair>,
    /// Whether pair entries were withheld by the output ceiling.
    pub pairs_truncated: Option<bool>,
    /// Certified local confluence (confluence operation).
    pub locally_confluent: Option<bool>,
    /// Rule indices that block certification even when pairs join.
    pub certification_blocked: Vec<u64>,
    /// Number of joinable pairs.
    pub joinable_count: Option<u64>,
    /// Number of non-joinable pairs.
    pub not_joinable_count: Option<u64>,
    /// `proved_terminating` or `not_decreasing`.
    pub termination: Option<String>,
    /// Per-system counts for a termination proof.
    pub termination_report: Option<RewriteTerminationSummary>,
    /// First rule that did not strictly decrease.
    pub failing_rule: Option<u64>,
    /// Library failure detail when `outcome` is `error`.
    pub failure_reason: Option<String>,
}

/// Typed input for the bounded completion probe.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-completion/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        max_depth_limit = "max_depth is between 1 and 8 when supplied",
        operation_known = "operation is complete or guided_inverse",
        precedence_required = "a precedence covering every used symbol at its arity is always required",
        precedence_symbols_limit = "at most 16 precedence symbols with arity at most 8",
        rules_checked = "every rule RHS variable occurs in its LHS",
        rules_count_limit = "at most 32 ordered rules are accepted",
        target_required = "guided_inverse requires a target term",
        term_bounds = "terms have depth at most 16 and at most 64 nodes",
        term_name_bytes_limit = "variable and symbol names contain at most 256 bytes"
    ),
    example(
        label = "complete_orientable_rule",
        value = "{\"operation\":\"complete\",\"rules\":[{\"lhs\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]},\"rhs\":{\"kind\":\"symbol\",\"name\":\"b\",\"arguments\":[]}}],\"precedence\":[{\"name\":\"a\",\"arity\":0},{\"name\":\"b\",\"arity\":0}],\"target\":null,\"max_depth\":null}"
    )
)]
pub struct RewriteCompletionRequest {
    /// The operation: `complete` or `guided_inverse`.
    pub operation: String,
    /// Ordered checked rewrite rules forming the input system.
    pub rules: Vec<RewriteRule>,
    /// The LPO precedence that orients every rule and generated
    /// equation. Always required.
    pub precedence: Vec<RewritePrecedenceSymbol>,
    /// The target term for `guided_inverse` (required there; any
    /// supplied term is validated against the probe ceilings).
    pub target: Option<RewriteTerm>,
    /// Guided-search depth for `guided_inverse`, 1..=8 (default 8).
    /// Any supplied value is validated even for `complete`.
    pub max_depth: Option<u64>,
}

/// One bounded completion trace event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteCompletionTraceStep {
    /// `pair`, `oriented`, or `simplified`.
    pub kind: String,
    /// Outer rule index for `pair`.
    pub left_rule: Option<u64>,
    /// Inner rule index for `pair`.
    pub right_rule: Option<u64>,
    /// Overlap position for `pair`.
    pub path: Vec<u64>,
    /// Affected rule index for `oriented` or `simplified`.
    pub rule_index: Option<u64>,
}

/// Typed output of the bounded completion probe.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-completion/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        bounded_trace = "at most 8 trace steps are reported; trace_step_count is the full count and trace_truncated reports withheld steps",
        completion_outcomes = "completion reports complete, partial, or failed with a typed reason, guided-inverse outcomes, or error when the library declines within bounds (failure_reason)",
        guided_inverse_authority = "verified_witness carries an original-system derivation; no variant claims a negative result from completed-system search",
        rejected_candidates_bounded = "at most 8 rejected candidate terms are reported; candidates_rejected_count is the full count",
        system_hash_is_canonical = "system_hash is the sha256 hex digest of the completion result system"
    ),
    example(
        label = "complete_orientable_rule",
        value = "{\"outcome\":\"complete\",\"operation\":\"complete\",\"rule_count\":1,\"system_rule_count\":1,\"system_hash\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"rules_added\":0,\"pairs_processed\":0,\"trace_step_count\":0,\"trace_steps\":[],\"trace_truncated\":false,\"partial_reason\":null,\"failed_reason\":null,\"path\":[],\"candidates_rejected\":[],\"candidates_rejected_count\":null,\"inconclusive_reason\":null,\"guidance_available\":null,\"failure_reason\":null}"
    )
)]
pub struct RewriteCompletionOutput {
    /// `complete`, `partial`, `failed`, `verified_witness`,
    /// `no_verified_witness`, `inconclusive`, or `error` when the
    /// library declined within bounds (see `failure_reason`).
    pub outcome: String,
    /// The dispatched operation.
    pub operation: String,
    /// Number of input rules.
    pub rule_count: u64,
    /// Rule count of the completion result system.
    pub system_rule_count: Option<u64>,
    /// Sha256 hex of the completion result system.
    pub system_hash: Option<String>,
    /// Rules completion generated from critical pairs.
    pub rules_added: Option<u64>,
    /// Distinct critical-pair equations processed.
    pub pairs_processed: Option<u64>,
    /// Full trace event count.
    pub trace_step_count: Option<u64>,
    /// Bounded trace events.
    pub trace_steps: Vec<RewriteCompletionTraceStep>,
    /// Whether trace events were withheld by the output ceiling.
    pub trace_truncated: Option<bool>,
    /// `unorientable_rule`, `non_left_linear`, or
    /// `confluence_not_certified`.
    pub partial_reason: Option<String>,
    /// `invalid_input` for a fatal precondition failure.
    pub failed_reason: Option<String>,
    /// Verified original-system derivation, source first.
    pub path: Vec<RewriteTerm>,
    /// Bounded rejected completed-system candidates.
    pub candidates_rejected: Vec<RewriteTerm>,
    /// Full rejected-candidate count.
    pub candidates_rejected_count: Option<u64>,
    /// `no_candidates_generated`, `budget_cut`, or
    /// `completion_not_complete`.
    pub inconclusive_reason: Option<String>,
    /// Whether convergent completion guidance was available.
    pub guidance_available: Option<bool>,
    /// Library failure detail when `outcome` is `error`.
    pub failure_reason: Option<String>,
}

/// Validate the rule-count and term ceilings shared by both probes.
#[cfg(feature = "standard-probes")]
fn validate_analysis_rules(rules: &[RewriteRule]) -> DiscoveryResult<u64> {
    let rule_count = u64::try_from(rules.len())
        .map_err(|_| DiscoveryError::LimitExceeded("analysis rule count overflow".to_owned()))?;
    enforce("analysis rule count", rule_count, MAX_ANALYSIS_RULES)?;
    let mut nodes = 0_u64;
    for rule in rules {
        for term in [&rule.lhs, &rule.rhs] {
            let stats = term_stats(term, MAX_ANALYSIS_TERM_DEPTH, MAX_ANALYSIS_TERM_NODES)?;
            nodes = nodes.checked_add(stats.nodes).ok_or_else(|| {
                DiscoveryError::LimitExceeded("analysis input node count overflow".to_owned())
            })?;
        }
    }
    Ok(nodes)
}

/// Build a validated [`Precedence`] from the request DTO.
#[cfg(feature = "standard-probes")]
fn analysis_precedence(symbols: &[RewritePrecedenceSymbol]) -> DiscoveryResult<Precedence> {
    if symbols.is_empty() {
        return Err(DiscoveryError::InvalidInput(
            "precedence requires at least one symbol".to_owned(),
        ));
    }
    if symbols.len() > MAX_ANALYSIS_PRECEDENCE_SYMBOLS {
        return Err(DiscoveryError::LimitExceeded(format!(
            "precedence declares {} symbols; the probe ceiling is \
             {MAX_ANALYSIS_PRECEDENCE_SYMBOLS}",
            symbols.len()
        )));
    }
    let mut ordering = Vec::with_capacity(symbols.len());
    for symbol in symbols {
        validate_name(&symbol.name)?;
        if symbol.arity > MAX_ANALYSIS_ARITY {
            return Err(DiscoveryError::LimitExceeded(format!(
                "precedence symbol `{}` declares arity {}; the probe ceiling is \
                 {MAX_ANALYSIS_ARITY}",
                symbol.name, symbol.arity
            )));
        }
        let arity = usize::try_from(symbol.arity)
            .map_err(|_| DiscoveryError::LimitExceeded("precedence arity overflow".to_owned()))?;
        ordering.push((Symbol::new(symbol.name.clone()), arity));
    }
    Precedence::new(ordering)
        .map_err(|error| DiscoveryError::InvalidInput(format!("precedence rejected: {error}")))
}

/// Build the shared relation budget for the analysis/completion probes.
#[cfg(feature = "standard-probes")]
fn analysis_resources(limits: &EffectiveProbeLimits) -> DiscoveryResult<RelationResources> {
    fn clamp(value: u64, ceiling: usize) -> usize {
        usize::try_from(value)
            .unwrap_or(usize::MAX)
            .clamp(1, ceiling)
    }
    let relation_limits = RelationLimits::new(
        clamp(limits.max_nodes, MAX_ANALYSIS_TERM_NODES as usize),
        MAX_ANALYSIS_TERM_DEPTH as usize,
        RelationLimits::MAX_CONSTRAINTS,
        clamp(limits.max_operations, MAX_ANALYSIS_OPERATIONS),
    )
    .map_err(|error| DiscoveryError::InvalidInput(format!("probe limit profile: {error}")))?;
    Ok(RelationResources::new(&relation_limits))
}

/// Convert a path into wire indices.
#[cfg(feature = "standard-probes")]
fn analysis_path(path: &amari_rewrite::rewritable::Path) -> Vec<u64> {
    path.as_slice().iter().map(|index| *index as u64).collect()
}

/// Convert a count for a wire field, failing closed on overflow.
#[cfg(feature = "standard-probes")]
fn analysis_count(value: usize) -> DiscoveryResult<u64> {
    u64::try_from(value)
        .map_err(|_| DiscoveryError::LimitExceeded("analysis count overflow".to_owned()))
}

/// Base output for an analysis operation.
#[cfg(feature = "standard-probes")]
fn analysis_output_base(operation: &str, rule_count: u64) -> RewriteAnalysisOutput {
    RewriteAnalysisOutput {
        outcome: "ok".to_owned(),
        operation: operation.to_owned(),
        rule_count,
        pair_count: None,
        trivial_excluded: None,
        non_left_linear_rules: Vec::new(),
        pairs: Vec::new(),
        pairs_truncated: None,
        locally_confluent: None,
        certification_blocked: Vec::new(),
        joinable_count: None,
        not_joinable_count: None,
        termination: None,
        termination_report: None,
        failing_rule: None,
        failure_reason: None,
    }
}

/// Base output for a completion operation.
#[cfg(feature = "standard-probes")]
fn completion_output_base(operation: &str, rule_count: u64) -> RewriteCompletionOutput {
    RewriteCompletionOutput {
        outcome: "ok".to_owned(),
        operation: operation.to_owned(),
        rule_count,
        system_rule_count: None,
        system_hash: None,
        rules_added: None,
        pairs_processed: None,
        trace_step_count: None,
        trace_steps: Vec::new(),
        trace_truncated: None,
        partial_reason: None,
        failed_reason: None,
        path: Vec::new(),
        candidates_rejected: Vec::new(),
        candidates_rejected_count: None,
        inconclusive_reason: None,
        guidance_available: None,
        failure_reason: None,
    }
}

/// Name of a bounded partial-stop reason.
#[cfg(feature = "standard-probes")]
fn partial_reason_name(reason: &PartialReason) -> &'static str {
    match reason {
        PartialReason::UnorientableRule { .. } => "unorientable_rule",
        PartialReason::NonLeftLinear => "non_left_linear",
        PartialReason::ConfluenceNotCertified => "confluence_not_certified",
    }
}

/// Name of a fatal completion failure reason.
#[cfg(feature = "standard-probes")]
fn failed_reason_name(reason: &FailedReason) -> &'static str {
    match reason {
        FailedReason::InvalidInput => "invalid_input",
    }
}

/// Name of a guided-search inconclusive reason.
#[cfg(feature = "standard-probes")]
fn inconclusive_reason_name(reason: InconclusiveReason) -> &'static str {
    match reason {
        InconclusiveReason::NoCandidatesGenerated => "no_candidates_generated",
        InconclusiveReason::BudgetCut => "budget_cut",
    }
}

/// Record a completion trace summary, capped for the wire.
#[cfg(feature = "standard-probes")]
fn record_trace(trace: &CompletionTrace, output: &mut RewriteCompletionOutput) {
    output.rules_added = Some(trace.rules_added as u64);
    output.pairs_processed = Some(trace.pairs_processed as u64);
    output.trace_step_count = Some(trace.steps.len() as u64);
    output.trace_truncated = Some(trace.steps.len() > MAX_ANALYSIS_TRACE_STEPS);
    output.trace_steps = trace
        .steps
        .iter()
        .take(MAX_ANALYSIS_TRACE_STEPS)
        .map(|step| match step {
            TraceStep::Pair {
                left_rule,
                right_rule,
                path,
            } => RewriteCompletionTraceStep {
                kind: "pair".to_owned(),
                left_rule: Some(*left_rule as u64),
                right_rule: Some(*right_rule as u64),
                path: analysis_path(path),
                rule_index: None,
            },
            TraceStep::Oriented { rule_index } => RewriteCompletionTraceStep {
                kind: "oriented".to_owned(),
                left_rule: None,
                right_rule: None,
                path: Vec::new(),
                rule_index: Some(*rule_index as u64),
            },
            TraceStep::Simplified { rule_index } => RewriteCompletionTraceStep {
                kind: "simplified".to_owned(),
                left_rule: None,
                right_rule: None,
                path: Vec::new(),
                rule_index: Some(*rule_index as u64),
            },
        })
        .collect();
}

/// Record a boundedly-typed completion outcome.
#[cfg(feature = "standard-probes")]
fn record_completion_outcome(output: &mut RewriteCompletionOutput, outcome: CompletionOutcome) {
    let (system, trace, partial, failed) = match outcome {
        CompletionOutcome::Complete { system, trace } => (Some(system), trace, None, None),
        CompletionOutcome::Partial {
            system,
            trace,
            reason,
        } => (
            Some(system),
            trace,
            Some(partial_reason_name(&reason)),
            None,
        ),
        CompletionOutcome::Failed { reason } => (
            None,
            CompletionTrace::default(),
            None,
            Some(failed_reason_name(&reason)),
        ),
    };
    if let Some(system) = system {
        output.system_rule_count = Some(system.rules().len() as u64);
        output.system_hash = Some(system_hash(&system).to_hex());
    }
    output.partial_reason = partial.map(str::to_owned);
    output.failed_reason = failed.map(str::to_owned);
    record_trace(&trace, output);
    output.outcome = if output.failed_reason.is_some() {
        "failed".to_owned()
    } else if output.partial_reason.is_some() {
        "partial".to_owned()
    } else {
        "complete".to_owned()
    };
}

/// Record rejected candidates, bounded for the wire.
#[cfg(feature = "standard-probes")]
fn record_rejections(rejections: &[CandidateRejection], output: &mut RewriteCompletionOutput) {
    output.candidates_rejected_count = Some(rejections.len() as u64);
    output.candidates_rejected = rejections
        .iter()
        .take(MAX_COMPLETION_REJECTIONS)
        .map(|rejection| match rejection {
            CandidateRejection::NotAnOriginalPredecessor { predecessor } => {
                RewriteTerm::from_term(predecessor)
            }
        })
        .collect();
}

/// Record a completion-guided inverse search outcome.
#[cfg(feature = "standard-probes")]
fn record_guided_outcome(output: &mut RewriteCompletionOutput, outcome: GuidedSearchOutcome) {
    match outcome {
        GuidedSearchOutcome::VerifiedWitness {
            path,
            candidates_rejected,
        } => {
            output.outcome = "verified_witness".to_owned();
            output.path = path.iter().map(RewriteTerm::from_term).collect();
            record_rejections(&candidates_rejected, output);
        }
        GuidedSearchOutcome::NoVerifiedWitness {
            candidates_rejected,
        } => {
            output.outcome = "no_verified_witness".to_owned();
            record_rejections(&candidates_rejected, output);
        }
        GuidedSearchOutcome::Inconclusive {
            reason,
            candidates_rejected,
        } => {
            output.outcome = "inconclusive".to_owned();
            output.inconclusive_reason = Some(inconclusive_reason_name(reason).to_owned());
            record_rejections(&candidates_rejected, output);
        }
    }
}

/// Build the bounded guided-search configuration.
#[cfg(feature = "standard-probes")]
fn completion_search_config(max_depth: u64) -> DiscoveryResult<InverseSearchConfig> {
    InverseSearchConfig::new(
        max_depth,
        4_096,
        16_384,
        4_096,
        64,
        4_096,
        65_536,
        50_000,
        1 << 20,
        1 << 20,
    )
    .map_err(|error| {
        DiscoveryError::InvalidInput(format!("completion search configuration invalid: {error}"))
    })
}

#[cfg(feature = "standard-probes")]
fn execute_analysis(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    // Meter first: JSON bounding, DTO validation, and domain-failure
    // paths are all accounted (PR #316 round 1).
    let mut resources = analysis_resources(limits)?;
    let json_nodes = super::validate_json_bounds(input)?;
    resources
        .record_operations(usize::try_from(json_nodes).unwrap_or(usize::MAX))
        .map_err(|error| language_error("request validation", error))?;
    let request: RewriteAnalysisRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "analysis request has an invalid operation, rule, or precedence shape: {error}"
            ))
        })?;
    if !matches!(
        request.operation.as_str(),
        "critical_pairs" | "confluence" | "termination"
    ) {
        return Err(DiscoveryError::InvalidInput(format!(
            "unknown analysis operation {:?}: expected critical_pairs, confluence, or termination",
            request.operation
        )));
    }
    let bounds = effective_bounds(limits);
    let input_nodes = validate_analysis_rules(&request.rules)?;
    let rule_count = analysis_count(request.rules.len())?;
    // A supplied precedence is validated against the probe ceilings
    // even when the selected operation does not consume it.
    let precedence = match request.precedence.as_deref() {
        Some(symbols) => Some(analysis_precedence(symbols)?),
        None => None,
    };
    let rules = request
        .rules
        .iter()
        .map(RewriteRule::to_rule)
        .collect::<DiscoveryResult<Vec<_>>>()?;
    let system = TermSystem::new(rules);
    let mut output = analysis_output_base(&request.operation, rule_count);

    let dispatch = |output: &mut RewriteAnalysisOutput,
                    resources: &mut RelationResources|
     -> DiscoveryResult<()> {
        match request.operation.as_str() {
            "critical_pairs" => {
                let report = match critical_pairs(&system, resources) {
                    Ok(value) => value,
                    Err(error) => {
                        record_domain_failure(
                            &mut output.outcome,
                            &mut output.failure_reason,
                            "critical pairs",
                            &error,
                        );
                        return Ok(());
                    }
                };
                output.pair_count = Some(analysis_count(report.pairs.len())?);
                output.trivial_excluded = Some(analysis_count(report.trivial_excluded)?);
                output.non_left_linear_rules = report
                    .non_left_linear_rules
                    .iter()
                    .map(|index| *index as u64)
                    .collect();
                output.pairs_truncated = Some(report.pairs.len() > MAX_ANALYSIS_PAIRS);
                output.pairs = report
                    .pairs
                    .iter()
                    .take(MAX_ANALYSIS_PAIRS)
                    .map(|pair| RewriteAnalysisPair {
                        left_rule: pair.left_rule as u64,
                        right_rule: pair.right_rule as u64,
                        path: analysis_path(&pair.path),
                        outcome: "overlap".to_owned(),
                        overlap: Some(RewriteTerm::from_term(&pair.overlap)),
                        root_step: Some(RewriteTerm::from_term(&pair.root_step)),
                        position_step: Some(RewriteTerm::from_term(&pair.position_step)),
                        join_term: None,
                    })
                    .collect();
            }
            "confluence" => {
                let report = match local_confluence(&system, resources) {
                    Ok(value) => value,
                    Err(error) => {
                        record_domain_failure(
                            &mut output.outcome,
                            &mut output.failure_reason,
                            "local confluence",
                            &error,
                        );
                        return Ok(());
                    }
                };
                let mut joinable = 0_u64;
                let mut not_joinable = 0_u64;
                for (_, outcome) in &report.pairs {
                    match outcome {
                        PairJoinability::Joinable(_) => joinable += 1,
                        PairJoinability::NotJoinable => not_joinable += 1,
                    }
                }
                output.pair_count = Some(analysis_count(report.pairs.len())?);
                output.joinable_count = Some(joinable);
                output.not_joinable_count = Some(not_joinable);
                output.locally_confluent = Some(report.locally_confluent);
                output.certification_blocked = report
                    .certification_blocked
                    .iter()
                    .map(|index| *index as u64)
                    .collect();
                output.pairs_truncated = Some(report.pairs.len() > MAX_ANALYSIS_PAIRS);
                output.pairs = report
                    .pairs
                    .iter()
                    .take(MAX_ANALYSIS_PAIRS)
                    .map(|(pair, outcome)| {
                        let (label, join_term) = match outcome {
                            PairJoinability::Joinable(witness) => {
                                ("joinable", Some(RewriteTerm::from_term(&witness.join_term)))
                            }
                            PairJoinability::NotJoinable => ("not_joinable", None),
                        };
                        RewriteAnalysisPair {
                            left_rule: pair.left_rule as u64,
                            right_rule: pair.right_rule as u64,
                            path: analysis_path(&pair.path),
                            outcome: label.to_owned(),
                            overlap: None,
                            root_step: Some(RewriteTerm::from_term(&pair.root_step)),
                            position_step: Some(RewriteTerm::from_term(&pair.position_step)),
                            join_term,
                        }
                    })
                    .collect();
            }
            "termination" => {
                let precedence = precedence.ok_or_else(|| {
                    DiscoveryError::InvalidInput(
                        "termination requires a `precedence` covering every used symbol".to_owned(),
                    )
                })?;
                let proof = match prove_terminating(&system, &precedence, resources) {
                    Ok(value) => value,
                    Err(error) => {
                        record_domain_failure(
                            &mut output.outcome,
                            &mut output.failure_reason,
                            "termination",
                            &error,
                        );
                        return Ok(());
                    }
                };
                match proof {
                    TerminationProof::ProvedTerminating { report } => {
                        output.termination = Some("proved_terminating".to_owned());
                        output.termination_report = Some(RewriteTerminationSummary {
                            rules_checked: analysis_count(report.rules_checked)?,
                            decreasing_rules: analysis_count(report.decreasing_rules)?,
                        });
                    }
                    TerminationProof::NotDecreasing { rule_index } => {
                        output.termination = Some("not_decreasing".to_owned());
                        output.failing_rule = Some(analysis_count(rule_index)?);
                    }
                }
            }
            _ => unreachable!("analysis operation validated before dispatch"),
        }
        Ok(())
    };
    dispatch(&mut output, &mut resources)?;

    let _ = validate_encoded_output(&output, bounds)?;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations: analysis_count(resources.operations())?.max(1),
            nodes: input_nodes,
            iterations: 1,
            bytes: 0,
        },
        output: serde_json::to_value(output)?,
    })
}

#[cfg(feature = "standard-probes")]
fn execute_completion(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    // Meter first: JSON bounding, DTO validation, and domain-failure
    // paths are all accounted (PR #316 round 1).
    let mut resources = analysis_resources(limits)?;
    let json_nodes = super::validate_json_bounds(input)?;
    resources
        .record_operations(usize::try_from(json_nodes).unwrap_or(usize::MAX))
        .map_err(|error| language_error("request validation", error))?;
    let request: RewriteCompletionRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "completion request has an invalid operation, rule, precedence, target, or \
                 depth shape: {error}"
            ))
        })?;
    if !matches!(request.operation.as_str(), "complete" | "guided_inverse") {
        return Err(DiscoveryError::InvalidInput(format!(
            "unknown completion operation {:?}: expected complete or guided_inverse",
            request.operation
        )));
    }
    let bounds = effective_bounds(limits);
    let mut input_nodes = validate_analysis_rules(&request.rules)?;
    let rule_count = analysis_count(request.rules.len())?;
    let precedence = analysis_precedence(&request.precedence)?;
    // Every supplied bounded field is validated before dispatch, even
    // when the selected operation does not consume it.
    if let Some(target) = &request.target {
        let stats = term_stats(target, MAX_ANALYSIS_TERM_DEPTH, MAX_ANALYSIS_TERM_NODES)?;
        input_nodes = input_nodes.checked_add(stats.nodes).ok_or_else(|| {
            DiscoveryError::LimitExceeded("completion input node count overflow".to_owned())
        })?;
    }
    let depth = request.max_depth.unwrap_or(MAX_COMPLETION_SEARCH_DEPTH);
    if depth == 0 || depth > MAX_COMPLETION_SEARCH_DEPTH {
        return Err(DiscoveryError::InvalidInput(format!(
            "max_depth {depth} is outside 1..={MAX_COMPLETION_SEARCH_DEPTH}"
        )));
    }
    let rules = request
        .rules
        .iter()
        .map(RewriteRule::to_rule)
        .collect::<DiscoveryResult<Vec<_>>>()?;
    let system = TermSystem::new(rules);
    let mut output = completion_output_base(&request.operation, rule_count);

    let dispatch = |output: &mut RewriteCompletionOutput,
                    resources: &mut RelationResources|
     -> DiscoveryResult<()> {
        match request.operation.as_str() {
            "complete" => {
                let config = CompletionConfig::new(precedence);
                let outcome = match complete(&system, &config, resources) {
                    Ok(value) => value,
                    Err(error) => {
                        record_domain_failure(
                            &mut output.outcome,
                            &mut output.failure_reason,
                            "completion",
                            &error,
                        );
                        return Ok(());
                    }
                };
                record_completion_outcome(output, outcome);
            }
            "guided_inverse" => {
                let target = request.target.as_ref().ok_or_else(|| {
                    DiscoveryError::InvalidInput(
                        "guided_inverse requires a `target` term".to_owned(),
                    )
                })?;
                let target = target.to_term()?;
                let guidance = match prepare_guidance(&system, precedence, resources) {
                    Ok(value) => value,
                    Err(error) => {
                        record_domain_failure(
                            &mut output.outcome,
                            &mut output.failure_reason,
                            "completion guidance",
                            &error,
                        );
                        return Ok(());
                    }
                };
                match guidance {
                    None => {
                        // Only a convergent completion is equational
                        // evidence; a partial or failed run offers no
                        // guidance, so the search never starts.
                        output.outcome = "inconclusive".to_owned();
                        output.inconclusive_reason = Some("completion_not_complete".to_owned());
                        output.guidance_available = Some(false);
                    }
                    Some(guidance) => {
                        output.guidance_available = Some(true);
                        output.system_rule_count = Some(guidance.completed.rules().len() as u64);
                        output.system_hash = Some(system_hash(&guidance.completed).to_hex());
                        record_trace(&guidance.trace, output);
                        let config = completion_search_config(depth)?;
                        let outcome = match guided_predecessors(
                            &system, &guidance, &target, &config, resources,
                        ) {
                            Ok(value) => value,
                            Err(error) => {
                                record_domain_failure(
                                    &mut output.outcome,
                                    &mut output.failure_reason,
                                    "guided inverse search",
                                    &error,
                                );
                                return Ok(());
                            }
                        };
                        record_guided_outcome(output, outcome);
                    }
                }
            }
            _ => unreachable!("completion operation validated before dispatch"),
        }
        Ok(())
    };
    dispatch(&mut output, &mut resources)?;

    let _ = validate_encoded_output(&output, bounds)?;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations: analysis_count(resources.operations())?.max(1),
            nodes: input_nodes,
            iterations: 1,
            bytes: 0,
        },
        output: serde_json::to_value(output)?,
    })
}

#[cfg(feature = "standard-probes")]
pub(super) fn analysis_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:analysis:v1".parse()?,
        capability_id: "amari:amari-rewrite:analysis:local-confluence".parse()?,
        input_schema: "amari.discovery/probe/rewrite-analysis/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-analysis/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_analysis,
    })
}

#[cfg(feature = "standard-probes")]
pub(super) fn completion_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:completion:v1".parse()?,
        capability_id: "amari:amari-rewrite:completion:knuth-bendix".parse()?,
        input_schema: "amari.discovery/probe/rewrite-completion/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-completion/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_completion,
    })
}

// ---------------------------------------------------------------------
// Cohort 7 synthesis probes (0.25 Task 39)
// ---------------------------------------------------------------------

/// Hard ceiling for one synthesis example list.
#[cfg(feature = "standard-probes")]
const MAX_SYNTHESIS_EXAMPLES: u64 = 32;
/// Hard ceiling for the residual example list.
#[cfg(feature = "standard-probes")]
const MAX_SYNTHESIS_RESIDUALS: u64 = 32;
/// Hard ceiling for synthesis term nodes.
#[cfg(feature = "standard-probes")]
const MAX_SYNTHESIS_TERM_NODES: u64 = 64;
/// Hard ceiling for synthesis term depth.
#[cfg(feature = "standard-probes")]
const MAX_SYNTHESIS_TERM_DEPTH: u64 = 16;
/// Hard ceiling for synthesis domain operations.
#[cfg(feature = "standard-probes")]
const MAX_SYNTHESIS_OPERATIONS: usize = 20_000;
/// Hard ceiling for candidate rules built in one specialization run.
#[cfg(feature = "standard-probes")]
const MAX_SPECIALIZATION_CANDIDATES: usize = 64;
/// Hard ceiling for positive partitions processed in one run.
#[cfg(feature = "standard-probes")]
const MAX_SPECIALIZATION_PARTITIONS: usize = 64;
/// Ceiling on refined rules reported by the specialization probe.
#[cfg(feature = "standard-probes")]
const MAX_SYNTHESIS_RULES: usize = 8;
/// Ceiling on inverse clauses reported by the inverse-synthesis probe.
#[cfg(feature = "standard-probes")]
const MAX_SYNTHESIS_CLAUSES: usize = 8;

/// One observed forward rewrite step used as synthesis evidence.
///
/// `source` is the term before rewriting and `target` the term after
/// it. The same pair shape carries positives (steps the rule set must
/// reproduce), negatives (steps it must never reproduce), and
/// residuals (observed steps that validate an erasing clause's
/// residual schema).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteSynthesisExample {
    /// Term before the observed step.
    pub source: RewriteTerm,
    /// Term after the observed step.
    pub target: RewriteTerm,
}

/// Typed input for bounded negative-example specialization.
///
/// # Limit profile
///
/// Each example list is capped at 32 pairs, terms at depth 16 and 64
/// nodes, and the library budget is
/// `RelationLimits::new(64, 16, RelationLimits::MAX_CONSTRAINTS,
/// 20_000)` with candidate and partition ceilings of 64. The example
/// ceiling additionally follows the caller's iterations budget, so a
/// tightened engine budget is a typed domain-failure outcome instead
/// of a silently widened one.
///
/// The probe meters its own request validation on the shared relation
/// budget and reports that charge; the library opens its run from the
/// same ceiling profile, so a tightened operations ceiling still
/// bounds the library work directly (as a typed domain-failure
/// outcome) rather than being ignored.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-specialize/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        example_count_limit = "at most 32 examples are accepted in each of positives and negatives",
        positives_required = "at least one positive example is required",
        term_bounds = "terms have depth at most 16 and at most 64 nodes",
        term_name_bytes_limit = "variable and symbol names contain at most 256 bytes"
    ),
    example(
        label = "specialize_add_zero",
        value = "{\"positives\":[{\"source\":{\"kind\":\"symbol\",\"name\":\"add\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"0\",\"arguments\":[]},{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]}]},\"target\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]}}],\"negatives\":[{\"source\":{\"kind\":\"symbol\",\"name\":\"add\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"0\",\"arguments\":[]},{\"kind\":\"symbol\",\"name\":\"c\",\"arguments\":[]}]},\"target\":{\"kind\":\"symbol\",\"name\":\"c\",\"arguments\":[]}}]}"
    )
)]
pub struct RewriteSpecializeRequest {
    /// Positive example steps the refined rules must reproduce.
    pub positives: Vec<RewriteSynthesisExample>,
    /// Negative example steps no returned rule may reproduce.
    pub negatives: Option<Vec<RewriteSynthesisExample>>,
}

/// Replayed coverage of one refined rule.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteCoverageEntry {
    /// Index of the rule in the refined rule list.
    pub rule_index: u64,
    /// Canonical indices of covered positive examples.
    pub positives: Vec<u64>,
    /// Canonical indices of covered negative examples (always empty
    /// for a refined outcome).
    pub negatives: Vec<u64>,
}

/// Typed output of the bounded specialization probe.
///
/// Rule and coverage evidence is capped at eight entries;
/// `rule_count` always reports the full count and `rules_truncated`
/// says whether entries were withheld.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-specialize/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        bounded_rule_evidence = "at most 8 rules and coverage entries are reported; rule_count is the full count and rules_truncated reports withheld rules",
        coverage_certified_honestly = "refined rules cover every deduped positive example and no supplied negative example",
        inconclusive_never_claims_absence = "an inconclusive outcome reports that this bounded search did not separate the examples, never that no separating rule exists"
    ),
    example(
        label = "specialize_add_zero",
        value = "{\"outcome\":\"refined\",\"operation\":\"specialize\",\"positive_count\":1,\"negative_count\":1,\"rule_count\":1,\"rules\":[{\"lhs\":{\"kind\":\"symbol\",\"name\":\"add\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"0\",\"arguments\":[]},{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]}]},\"rhs\":{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]}}],\"rules_truncated\":false,\"coverage\":[{\"rule_index\":0,\"positives\":[0],\"negatives\":[]}],\"inconclusive_reason\":null,\"failure_reason\":null}"
    )
)]
pub struct RewriteSpecializeOutput {
    /// `refined` for a validated rule set, `inconclusive` when the
    /// bounded search separated no candidate, or `error` when the
    /// library declined within bounds (see `failure_reason`).
    pub outcome: String,
    /// The dispatched operation (`specialize`).
    pub operation: String,
    /// Number of supplied positive examples.
    pub positive_count: u64,
    /// Number of supplied negative examples.
    pub negative_count: u64,
    /// Full refined rule count.
    pub rule_count: Option<u64>,
    /// Bounded refined rules that cover every positive and no
    /// negative.
    pub rules: Vec<RewriteRule>,
    /// Whether rules were withheld by the output ceiling.
    pub rules_truncated: Option<bool>,
    /// Bounded replayed coverage aligned with `rules`.
    pub coverage: Vec<RewriteCoverageEntry>,
    /// `inseparable` or `unrepresentable` for an inconclusive
    /// outcome.
    pub inconclusive_reason: Option<String>,
    /// Library failure detail when `outcome` is `error`.
    pub failure_reason: Option<String>,
}

/// Typed input for bounded inverse-clause synthesis.
///
/// # Limit profile
///
/// Each example list is capped at 32 pairs, terms at depth 16 and 64
/// nodes, and the library budget is one
/// `InverseSynthesisConfig` over `RelationLimits::new(64, 16,
/// RelationLimits::MAX_CONSTRAINTS, 20_000)` with candidate and
/// partition ceilings of 64 and a residual ceiling of 32. The example
/// ceiling additionally follows the caller's iterations budget, so a
/// tightened engine budget is a typed domain-failure outcome instead
/// of a silently widened one.
///
/// The probe meters its own request validation on the shared relation
/// budget and reports that charge; the library opens its run from the
/// same ceiling profile, so a tightened operations ceiling still
/// bounds the library work directly (as a typed domain-failure
/// outcome) rather than being ignored.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-synthesize-inverse/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        example_count_limit = "at most 32 examples are accepted in each of positives, negatives, and residuals",
        positives_required = "at least one positive example is required",
        term_bounds = "terms have depth at most 16 and at most 64 nodes",
        term_name_bytes_limit = "variable and symbol names contain at most 256 bytes"
    ),
    example(
        label = "lossless_inverse",
        value = "{\"positives\":[{\"source\":{\"kind\":\"symbol\",\"name\":\"k\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]}]},\"target\":{\"kind\":\"symbol\",\"name\":\"m\",\"arguments\":[{\"kind\":\"symbol\",\"name\":\"a\",\"arguments\":[]}]}}],\"negatives\":null,\"residuals\":null}"
    )
)]
pub struct RewriteInverseSynthesisRequest {
    /// Positive example steps the checked forward rules must replay.
    pub positives: Vec<RewriteSynthesisExample>,
    /// Negative example steps no checked forward rule may reproduce.
    pub negatives: Option<Vec<RewriteSynthesisExample>>,
    /// Residual evidence for erasing clauses' residual schemas.
    pub residuals: Option<Vec<RewriteSynthesisExample>>,
}

/// One residual-schema binding in Cohort 2 key order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteResidualSchemaBinding {
    /// Schema position of the erased variable (`LogicVar(0, index)`).
    pub key_index: u64,
    /// The erased variable name the reverse direction must recover.
    pub variable: String,
}

/// One promoted inverse clause.
///
/// `reverse` is present exactly when the reverse direction is itself a
/// checked rule. An erasing clause carries `residual_schema` instead
/// of an unchecked `target -> source` rule that would invent the
/// variables the forward step erased.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteInverseClause {
    /// The checked forward rule the clause was derived from.
    pub forward: RewriteRule,
    /// The checked reverse rule, absent for an erasing clause.
    pub reverse: Option<RewriteRule>,
    /// Stable identity of the compiled backward contract.
    pub backward_rule_id: String,
    /// Variables the forward direction erases, in sorted order.
    pub erased_variables: Vec<String>,
    /// Residual authority the reverse direction needs, in key order.
    pub residual_schema: Vec<RewriteResidualSchemaBinding>,
}

/// Validation evidence for a promoted inverse candidate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteInverseEvidence {
    /// Deduped positive examples replayed forward.
    pub positives: u64,
    /// Deduped negative examples rejected.
    pub negatives: u64,
    /// Deduped residual examples supplied.
    pub residuals: u64,
    /// `(positive, clause)` forward replay checks.
    pub forward_replays: u64,
    /// `(negative, clause)` rejection checks.
    pub negative_rejections: u64,
    /// Residual and reverse round-trip checks performed.
    pub residual_roundtrips: u64,
    /// Clauses whose reverse direction is a checked rule.
    pub reversible_clauses: u64,
    /// Clauses carrying a non-empty residual schema.
    pub residual_clauses: u64,
}

/// A concrete witness that refuted promotion.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteInverseRefutation {
    /// `forward_replay`, `negative_accepted`, `residual_round_trip`,
    /// or `reverse_replay`.
    pub kind: String,
    /// Canonical index of the named positive example.
    pub positive: Option<u64>,
    /// Canonical index of the named negative example.
    pub negative: Option<u64>,
    /// Canonical index of the named residual example.
    pub residual: Option<u64>,
    /// Index of the named clause.
    pub clause: Option<u64>,
}

/// Why the evidence determined no candidate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RewriteInverseUnknown {
    /// `underdetermined`, `residual_underdetermined`, or `ambiguous`.
    pub kind: String,
    /// `inseparable` or `unrepresentable` for an underdetermined
    /// forward search.
    pub detail: Option<String>,
    /// Index of the unvalidated or conflicted clause.
    pub clause: Option<u64>,
}

/// Typed output of the bounded inverse-synthesis probe.
///
/// Clause evidence is capped at eight entries; `clause_count` always
/// reports the full count and `clauses_truncated` says whether
/// entries were withheld.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/rewrite-synthesize-inverse/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        bounded_clause_evidence = "at most 8 clauses are reported; clause_count is the full count and clauses_truncated reports withheld clauses",
        candidate_certified_honestly = "a candidate replays every deduped positive forward, reproduces no negative, and validates its residual and reverse evidence",
        erased_clauses_carry_schema = "an erasing clause never carries a reverse rule; it carries the residual schema the reverse direction needs",
        unknown_and_refuted_are_typed = "refutation names a concrete witness and an unknown reason names why the evidence determines no candidate"
    ),
    example(
        label = "lossless_inverse",
        value = "{\"outcome\":\"candidate\",\"operation\":\"synthesize_inverse\",\"positive_count\":1,\"negative_count\":0,\"residual_count\":0,\"rule_count\":1,\"clause_count\":1,\"clauses\":[{\"forward\":{\"lhs\":{\"kind\":\"symbol\",\"name\":\"k\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"_I0\"}]},\"rhs\":{\"kind\":\"symbol\",\"name\":\"m\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"_I0\"}]}},\"reverse\":{\"lhs\":{\"kind\":\"symbol\",\"name\":\"m\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"_I0\"}]},\"rhs\":{\"kind\":\"symbol\",\"name\":\"k\",\"arguments\":[{\"kind\":\"variable\",\"name\":\"_I0\"}]}},\"backward_rule_id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"erased_variables\":[],\"residual_schema\":[]}],\"clauses_truncated\":false,\"evidence\":{\"positives\":1,\"negatives\":0,\"residuals\":0,\"forward_replays\":1,\"negative_rejections\":0,\"residual_roundtrips\":1,\"reversible_clauses\":1,\"residual_clauses\":0},\"refutation\":null,\"unknown_reason\":null,\"failure_reason\":null}"
    )
)]
pub struct RewriteInverseSynthesisOutput {
    /// `candidate`, `refuted`, `unknown`, or `error` when the library
    /// declined within bounds (see `failure_reason`).
    pub outcome: String,
    /// The dispatched operation (`synthesize_inverse`).
    pub operation: String,
    /// Number of supplied positive examples.
    pub positive_count: u64,
    /// Number of supplied negative examples.
    pub negative_count: u64,
    /// Number of supplied residual examples.
    pub residual_count: u64,
    /// Full checked forward rule count.
    pub rule_count: Option<u64>,
    /// Full derived clause count.
    pub clause_count: Option<u64>,
    /// Bounded derived clauses, aligned with the forward rule order.
    pub clauses: Vec<RewriteInverseClause>,
    /// Whether clauses were withheld by the output ceiling.
    pub clauses_truncated: Option<bool>,
    /// Validation evidence for a promoted candidate.
    pub evidence: Option<RewriteInverseEvidence>,
    /// The concrete witness that refuted promotion.
    pub refutation: Option<RewriteInverseRefutation>,
    /// Why the evidence determined no candidate.
    pub unknown_reason: Option<RewriteInverseUnknown>,
    /// Library failure detail when `outcome` is `error`.
    pub failure_reason: Option<String>,
}

/// Build the shared relation budget for the synthesis probes.
#[cfg(feature = "standard-probes")]
fn synthesis_relation_limits(limits: &EffectiveProbeLimits) -> DiscoveryResult<RelationLimits> {
    fn clamp(value: u64, ceiling: usize) -> usize {
        usize::try_from(value)
            .unwrap_or(usize::MAX)
            .clamp(1, ceiling)
    }
    RelationLimits::new(
        clamp(limits.max_nodes, MAX_SYNTHESIS_TERM_NODES as usize),
        MAX_SYNTHESIS_TERM_DEPTH as usize,
        RelationLimits::MAX_CONSTRAINTS,
        clamp(limits.max_operations, MAX_SYNTHESIS_OPERATIONS),
    )
    .map_err(|error| DiscoveryError::InvalidInput(format!("synthesis limit profile: {error}")))
}

#[cfg(feature = "standard-probes")]
fn synthesis_resources(limits: &EffectiveProbeLimits) -> DiscoveryResult<RelationResources> {
    Ok(RelationResources::new(&synthesis_relation_limits(limits)?))
}

/// The example ceiling actually handed to the library: the tighter of
/// the probe ceiling and the caller's iterations budget.
#[cfg(feature = "standard-probes")]
fn synthesis_example_ceiling(limits: &EffectiveProbeLimits) -> usize {
    usize::try_from(limits.max_iterations.min(MAX_SYNTHESIS_EXAMPLES))
        .unwrap_or(usize::MAX)
        .max(1)
}

/// Build the checked specialization profile through the validated
/// constructor; a zero or above-ceiling knob is rejected rather than
/// silently widened or disabled.
#[cfg(feature = "standard-probes")]
fn specialization_config(limits: &EffectiveProbeLimits) -> DiscoveryResult<SpecializationConfig> {
    SpecializationConfig::new(
        synthesis_example_ceiling(limits),
        MAX_SPECIALIZATION_CANDIDATES,
        MAX_SPECIALIZATION_PARTITIONS,
        synthesis_relation_limits(limits)?,
    )
    .map_err(|error| DiscoveryError::InvalidInput(format!("specialization limit profile: {error}")))
}

/// Build the checked inverse-synthesis profile through the validated
/// constructor.
#[cfg(feature = "standard-probes")]
fn inverse_synthesis_config(
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<InverseSynthesisConfig> {
    InverseSynthesisConfig::new(
        specialization_config(limits)?,
        usize::try_from(MAX_SYNTHESIS_RESIDUALS).unwrap_or(usize::MAX),
    )
    .map_err(|error| {
        DiscoveryError::InvalidInput(format!("inverse synthesis limit profile: {error}"))
    })
}

/// Validate one example list against the probe ceilings and convert it
/// to checked terms, returning the pairs with their total node count.
///
/// Depth and node ceilings are checked iteratively over the borrowed
/// DTOs BEFORE any recursive conversion runs, so adversarial input is a
/// typed limit error instead of a stack abort.
#[cfg(feature = "standard-probes")]
fn validate_synthesis_examples(
    examples: &[RewriteSynthesisExample],
    ceiling: u64,
    kind: &str,
) -> DiscoveryResult<(Vec<(Term, Term)>, u64)> {
    let count = u64::try_from(examples.len())
        .map_err(|_| DiscoveryError::LimitExceeded(format!("{kind} count overflow")))?;
    enforce(kind, count, ceiling)?;
    let mut nodes = 0_u64;
    let mut pairs = Vec::with_capacity(examples.len());
    for example in examples {
        let source = term_stats(
            &example.source,
            MAX_SYNTHESIS_TERM_DEPTH,
            MAX_SYNTHESIS_TERM_NODES,
        )?;
        let target = term_stats(
            &example.target,
            MAX_SYNTHESIS_TERM_DEPTH,
            MAX_SYNTHESIS_TERM_NODES,
        )?;
        for stats in [source, target] {
            nodes = nodes.checked_add(stats.nodes).ok_or_else(|| {
                DiscoveryError::LimitExceeded(format!("{kind} node count overflow"))
            })?;
        }
        pairs.push((example.source.to_term()?, example.target.to_term()?));
    }
    Ok((pairs, nodes))
}

/// Name of a bounded specialization inconclusive reason.
#[cfg(feature = "standard-probes")]
fn specialization_reason_name(
    reason: amari_rewrite::synthesis::InconclusiveReason,
) -> &'static str {
    match reason {
        amari_rewrite::synthesis::InconclusiveReason::Inseparable => "inseparable",
        amari_rewrite::synthesis::InconclusiveReason::Unrepresentable => "unrepresentable",
    }
}

/// Convert one promoted clause into wire evidence.
#[cfg(feature = "standard-probes")]
fn inverse_clause_dto(clause: &InverseClause) -> DiscoveryResult<RewriteInverseClause> {
    let mut residual_schema = Vec::with_capacity(clause.residual_schema().len());
    for (key, variable) in clause.residual_schema().bindings() {
        residual_schema.push(RewriteResidualSchemaBinding {
            key_index: u64::from(key.index()),
            variable: variable.as_str().to_owned(),
        });
    }
    Ok(RewriteInverseClause {
        forward: RewriteRule::from_rule(clause.forward()),
        reverse: clause.reverse_rule().map(RewriteRule::from_rule),
        backward_rule_id: clause.backward().rule_id().to_string(),
        erased_variables: clause.backward().erased_variables().to_vec(),
        residual_schema,
    })
}

/// Convert validation evidence into wire evidence.
#[cfg(feature = "standard-probes")]
fn inverse_evidence_dto(evidence: &InverseEvidence) -> DiscoveryResult<RewriteInverseEvidence> {
    Ok(RewriteInverseEvidence {
        positives: analysis_count(evidence.positives())?,
        negatives: analysis_count(evidence.negatives())?,
        residuals: analysis_count(evidence.residuals())?,
        forward_replays: analysis_count(evidence.forward_replays())?,
        negative_rejections: analysis_count(evidence.negative_rejections())?,
        residual_roundtrips: analysis_count(evidence.residual_roundtrips())?,
        reversible_clauses: analysis_count(evidence.reversible_clauses())?,
        residual_clauses: analysis_count(evidence.residual_clauses())?,
    })
}

/// Convert a refuting witness into its typed wire form.
#[cfg(feature = "standard-probes")]
fn refutation_dto(refutation: &InverseRefutation) -> DiscoveryResult<RewriteInverseRefutation> {
    let (kind, positive, negative, residual, clause) = match refutation {
        InverseRefutation::ForwardReplay { positive } => {
            ("forward_replay", Some(*positive), None, None, None)
        }
        InverseRefutation::NegativeAccepted { negative, clause } => (
            "negative_accepted",
            None,
            Some(*negative),
            None,
            Some(*clause),
        ),
        InverseRefutation::ResidualRoundTrip { residual, clause } => (
            "residual_round_trip",
            None,
            None,
            Some(*residual),
            Some(*clause),
        ),
        InverseRefutation::ReverseReplay { positive, clause } => {
            ("reverse_replay", Some(*positive), None, None, Some(*clause))
        }
    };
    Ok(RewriteInverseRefutation {
        kind: kind.to_owned(),
        positive: positive.map(analysis_count).transpose()?,
        negative: negative.map(analysis_count).transpose()?,
        residual: residual.map(analysis_count).transpose()?,
        clause: clause.map(analysis_count).transpose()?,
    })
}

/// Convert an unknown reason into its typed wire form.
#[cfg(feature = "standard-probes")]
fn unknown_reason_dto(reason: &InverseUnknownReason) -> DiscoveryResult<RewriteInverseUnknown> {
    let (kind, detail, clause) = match reason {
        InverseUnknownReason::Underdetermined { detail } => (
            "underdetermined",
            Some(specialization_reason_name(*detail).to_owned()),
            None,
        ),
        InverseUnknownReason::ResidualUnderdetermined { clause } => {
            ("residual_underdetermined", None, Some(*clause))
        }
        InverseUnknownReason::Ambiguous { clause } => ("ambiguous", None, Some(*clause)),
    };
    Ok(RewriteInverseUnknown {
        kind: kind.to_owned(),
        detail,
        clause: clause.map(analysis_count).transpose()?,
    })
}

/// Base output for one specialization run.
#[cfg(feature = "standard-probes")]
fn specialize_output_base(positive_count: u64, negative_count: u64) -> RewriteSpecializeOutput {
    RewriteSpecializeOutput {
        outcome: "refined".to_owned(),
        operation: "specialize".to_owned(),
        positive_count,
        negative_count,
        rule_count: None,
        rules: Vec::new(),
        rules_truncated: None,
        coverage: Vec::new(),
        inconclusive_reason: None,
        failure_reason: None,
    }
}

/// Base output for one inverse-synthesis run.
#[cfg(feature = "standard-probes")]
fn inverse_output_base(
    positive_count: u64,
    negative_count: u64,
    residual_count: u64,
) -> RewriteInverseSynthesisOutput {
    RewriteInverseSynthesisOutput {
        outcome: "candidate".to_owned(),
        operation: "synthesize_inverse".to_owned(),
        positive_count,
        negative_count,
        residual_count,
        rule_count: None,
        clause_count: None,
        clauses: Vec::new(),
        clauses_truncated: None,
        evidence: None,
        refutation: None,
        unknown_reason: None,
        failure_reason: None,
    }
}

#[cfg(feature = "standard-probes")]
fn execute_specialize(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    // Meter first: JSON bounding, DTO validation, and domain-failure
    // paths are all accounted (PR #316 round 1).
    let mut resources = synthesis_resources(limits)?;
    let json_nodes = super::validate_json_bounds(input)?;
    resources
        .record_operations(usize::try_from(json_nodes).unwrap_or(usize::MAX))
        .map_err(|error| language_error("request validation", error))?;
    let request: RewriteSpecializeRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "specialization request has an invalid example shape: {error}"
            ))
        })?;
    let bounds = effective_bounds(limits);
    let request_bytes = encoded_bytes(&request, "specialization request")?;
    enforce("request bytes", request_bytes, bounds.max_request_bytes)?;
    if request.positives.is_empty() {
        return Err(DiscoveryError::InvalidInput(
            "specialization requires at least one positive example".to_owned(),
        ));
    }
    let (positives, positive_nodes) = validate_synthesis_examples(
        &request.positives,
        MAX_SYNTHESIS_EXAMPLES,
        "positive examples",
    )?;
    let (negatives, negative_nodes) = validate_synthesis_examples(
        request.negatives.as_deref().unwrap_or(&[]),
        MAX_SYNTHESIS_EXAMPLES,
        "negative examples",
    )?;
    let input_nodes = positive_nodes.checked_add(negative_nodes).ok_or_else(|| {
        DiscoveryError::LimitExceeded("specialization input node count overflow".to_owned())
    })?;
    let mut output = specialize_output_base(
        analysis_count(positives.len())?,
        analysis_count(negatives.len())?,
    );
    let config = specialization_config(limits)?;

    match specialize_rules(&positives, &negatives, &config) {
        Ok(SpecializationOutcome::Refined { rules, coverage }) => {
            output.outcome = "refined".to_owned();
            output.rule_count = Some(analysis_count(rules.len())?);
            output.rules_truncated = Some(rules.len() > MAX_SYNTHESIS_RULES);
            output.rules = rules
                .iter()
                .take(MAX_SYNTHESIS_RULES)
                .map(RewriteRule::from_rule)
                .collect();
            let mut entries = Vec::with_capacity(coverage.rules().len().min(MAX_SYNTHESIS_RULES));
            for entry in coverage.rules().iter().take(MAX_SYNTHESIS_RULES) {
                entries.push(RewriteCoverageEntry {
                    rule_index: analysis_count(entry.rule_index())?,
                    positives: entry
                        .positives()
                        .iter()
                        .map(|index| *index as u64)
                        .collect(),
                    negatives: entry
                        .negatives()
                        .iter()
                        .map(|index| *index as u64)
                        .collect(),
                });
            }
            output.coverage = entries;
        }
        Ok(SpecializationOutcome::Inconclusive { reason }) => {
            // Inconclusive reports that this bounded search separated no
            // candidate; it never asserts that none exists.
            output.outcome = "inconclusive".to_owned();
            output.inconclusive_reason = Some(specialization_reason_name(reason).to_owned());
        }
        Err(error) => record_domain_failure(
            &mut output.outcome,
            &mut output.failure_reason,
            "specialization",
            &error,
        ),
    }

    let _ = validate_encoded_output(&output, bounds)?;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations: analysis_count(resources.operations())?.max(1),
            nodes: input_nodes,
            iterations: 1,
            bytes: 0,
        },
        output: serde_json::to_value(output)?,
    })
}

#[cfg(feature = "standard-probes")]
fn execute_synthesize_inverse(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    // Meter first: JSON bounding, DTO validation, and domain-failure
    // paths are all accounted (PR #316 round 1).
    let mut resources = synthesis_resources(limits)?;
    let json_nodes = super::validate_json_bounds(input)?;
    resources
        .record_operations(usize::try_from(json_nodes).unwrap_or(usize::MAX))
        .map_err(|error| language_error("request validation", error))?;
    let request: RewriteInverseSynthesisRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "inverse synthesis request has an invalid example shape: {error}"
            ))
        })?;
    let bounds = effective_bounds(limits);
    let request_bytes = encoded_bytes(&request, "inverse synthesis request")?;
    enforce("request bytes", request_bytes, bounds.max_request_bytes)?;
    if request.positives.is_empty() {
        return Err(DiscoveryError::InvalidInput(
            "inverse synthesis requires at least one positive example".to_owned(),
        ));
    }
    let (positives, positive_nodes) = validate_synthesis_examples(
        &request.positives,
        MAX_SYNTHESIS_EXAMPLES,
        "positive examples",
    )?;
    let (negatives, negative_nodes) = validate_synthesis_examples(
        request.negatives.as_deref().unwrap_or(&[]),
        MAX_SYNTHESIS_EXAMPLES,
        "negative examples",
    )?;
    let (residuals, residual_nodes) = validate_synthesis_examples(
        request.residuals.as_deref().unwrap_or(&[]),
        MAX_SYNTHESIS_RESIDUALS,
        "residual examples",
    )?;
    let input_nodes = positive_nodes
        .checked_add(negative_nodes)
        .and_then(|total| total.checked_add(residual_nodes))
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("inverse synthesis input node count overflow".to_owned())
        })?;
    let mut output = inverse_output_base(
        analysis_count(positives.len())?,
        analysis_count(negatives.len())?,
        analysis_count(residuals.len())?,
    );
    let config = inverse_synthesis_config(limits)?;

    match synthesize_inverse(&positives, &negatives, &residuals, &config) {
        Ok(InverseSynthesisOutcome::Candidate {
            rules,
            clauses,
            evidence,
        }) => {
            output.outcome = "candidate".to_owned();
            output.rule_count = Some(analysis_count(rules.len())?);
            output.clause_count = Some(analysis_count(clauses.len())?);
            output.clauses_truncated = Some(clauses.len() > MAX_SYNTHESIS_CLAUSES);
            let mut entries = Vec::with_capacity(clauses.len().min(MAX_SYNTHESIS_CLAUSES));
            for clause in clauses.iter().take(MAX_SYNTHESIS_CLAUSES) {
                entries.push(inverse_clause_dto(clause)?);
            }
            output.clauses = entries;
            output.evidence = Some(inverse_evidence_dto(&evidence)?);
        }
        Ok(InverseSynthesisOutcome::Refuted { witness }) => {
            output.outcome = "refuted".to_owned();
            output.refutation = Some(refutation_dto(witness.refutation())?);
        }
        Ok(InverseSynthesisOutcome::Unknown { reason }) => {
            // Ambiguity and underdetermination are reported, never
            // resolved by choosing a branch.
            output.outcome = "unknown".to_owned();
            output.unknown_reason = Some(unknown_reason_dto(&reason)?);
        }
        Err(error) => record_domain_failure(
            &mut output.outcome,
            &mut output.failure_reason,
            "inverse synthesis",
            &error,
        ),
    }

    let _ = validate_encoded_output(&output, bounds)?;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations: analysis_count(resources.operations())?.max(1),
            nodes: input_nodes,
            iterations: 1,
            bytes: 0,
        },
        output: serde_json::to_value(output)?,
    })
}

#[cfg(feature = "standard-probes")]
pub(super) fn specialize_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:specialize:v1".parse()?,
        capability_id: "amari:amari-rewrite:synthesis:specialize".parse()?,
        input_schema: "amari.discovery/probe/rewrite-specialize/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-specialize/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_specialize,
    })
}

#[cfg(feature = "standard-probes")]
pub(super) fn synthesize_inverse_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:rewrite:synthesize-inverse:v1".parse()?,
        capability_id: "amari:amari-rewrite:synthesis:inverse".parse()?,
        input_schema: "amari.discovery/probe/rewrite-synthesize-inverse/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/rewrite-synthesize-inverse/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_synthesize_inverse,
    })
}
