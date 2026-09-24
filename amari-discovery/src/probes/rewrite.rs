// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded DTO validation shared by registered rewrite probes.

#[cfg(feature = "standard-probes")]
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[cfg(feature = "standard-probes")]
use amari_rewrite::{
    language::{
        RankedSymbol, RegularTreeGrammar, TreeAutomaton, TreeAutomatonLimits, TreeState,
        TreeTransition,
    },
    synthesis::infer_rule,
    trs::{match_pattern, Rule, Term, TermSystem},
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
fn determinize_charge(automaton: &TreeAutomaton, discovered: u64) -> u64 {
    // Exact subset-construction charge: when macro-state m (0-indexed)
    // registers, the odometer walks (m+1)^arity steps per non-nullary
    // symbol and every step is charged. Discovered macro-states can
    // outnumber the source states, so the charge is indexed by the
    // RESULT state count, not the input's (saturating).
    let mut total = 0u64;
    for symbol in automaton.alphabet().iter().filter(|s| s.arity() >= 1) {
        let arity = u32::from(symbol.arity());
        for m in 1..=discovered {
            total = total.saturating_add(m.saturating_pow(arity));
        }
    }
    total
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
                .map_err(|error| DiscoveryError::InvalidInput(format!("grammar: {error}")))?
                .to_automaton()
                .map_err(|error| {
                    DiscoveryError::InvalidInput(format!("grammar conversion: {error}"))
                })?;
            validate_grammar_derived_names(&automaton)?;
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
    let source_transitions = source.transitions().len() as u64;
    let source_finals = source.finals().len() as u64;
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
            let accepted = source
                .accepts(&term)
                .map_err(|error| DiscoveryError::InvalidInput(format!("membership: {error}")))?;
            // accepting_run charges one operation per transition at
            // every term node.
            let operations = term_nodes.saturating_mul(source_transitions);
            decision_output(Some(accepted), None, None, operations, term_nodes)
        }
        "emptiness" => {
            // The witness fixpoint scans every transition in each of
            // at most `states` rounds.
            let operations = source_transitions.saturating_mul(source_states.max(1));
            let empty = source.language_is_empty();
            decision_output(None, Some(empty), None, operations, source_states)
        }
        "witness" => {
            let operations = source_transitions.saturating_mul(source_states.max(1));
            let witness = source
                .witness()
                .map_err(|error| DiscoveryError::InvalidInput(format!("witness: {error}")))?
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
                operations.saturating_add(witness_nodes),
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
            let other_states = other.states().len() as u64;
            let other_transitions = other.transitions().len() as u64;
            let other_finals = other.finals().len() as u64;
            let result = if request.operation == "union" {
                source.union(&other, &TreeAutomatonLimits::default())
            } else {
                source.intersection(&other, &TreeAutomatonLimits::default())
            }
            .map_err(|error| {
                DiscoveryError::InvalidInput(format!("{}: {error}", request.operation))
            })?;
            // Union concatenates transitions and enumerates the final
            // product; intersection pairs compatible transitions and
            // enumerates the final product (saturating).
            let operations = if request.operation == "union" {
                source_transitions
                    .saturating_add(other_transitions)
                    .saturating_add(source_finals.saturating_mul(other_finals))
            } else {
                source_transitions
                    .saturating_mul(other_transitions)
                    .saturating_add(source_finals.saturating_mul(other_finals))
            };
            let _ = other_states;
            certificate_output(&result, operations)
        }
        "determinize" => {
            let result = source
                .determinize(&TreeAutomatonLimits::default())
                .map_err(|error| DiscoveryError::InvalidInput(format!("determinize: {error}")))?;
            // Exact odometer-step charge over the registered
            // macro-states (seeding is uncharged).
            let operations = determinize_charge(&source, result.states().len() as u64);
            certificate_output(&result, operations)
        }
        "minimize" => {
            let result = source
                .minimized()
                .map_err(|error| DiscoveryError::InvalidInput(format!("minimize: {error}")))?;
            // Minimization requires deterministic input, then trims,
            // completes, refines, and canonically numbers the result:
            // - completion charges one odometer step per state TUPLE
            //   per symbol: sum over symbols of states^arity (the sink
            //   adds one state);
            // - refinement charges once per child position per round,
            //   with at most `states` rounds;
            // - canonical numbering charges at most once per
            //   transition.
            // All saturating.
            let completed_states = source_states.saturating_add(1).max(1);
            let completion = source.alphabet().iter().fold(0u64, |total, symbol| {
                total.saturating_add(completed_states.saturating_pow(u32::from(symbol.arity())))
            });
            let completed_transitions = source_transitions.saturating_add(completion);
            let max_arity = source
                .alphabet()
                .iter()
                .map(|symbol| u64::from(symbol.arity()))
                .max()
                .unwrap_or(0);
            let refinement = completed_states
                .saturating_mul(max_arity)
                .saturating_mul(completed_transitions);
            let operations = completion
                .saturating_add(refinement)
                .saturating_add(completed_transitions);
            certificate_output(&result, operations)
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
