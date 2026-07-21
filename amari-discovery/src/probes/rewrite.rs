// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded DTO validation shared by registered rewrite probes.

// Task 20A intentionally lands validation primitives before the executable
// adapters in Tasks 20B-20D; remove this transition allowance once registered.
#![cfg_attr(any(not(test), not(feature = "standard-probes")), allow(dead_code))]

#[cfg(feature = "standard-probes")]
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[cfg(feature = "standard-probes")]
use amari_rewrite::trs::{match_pattern, Rule, Term, TermSystem};
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

/// Serializable first-order term accepted by rewrite probes.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
}

/// Typed input for bounded ordered term normalization.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RewriteNormalizeRequest {
    /// Initial first-order term.
    pub term: RewriteTerm,
    /// Ordered checked rewrite rules.
    pub rules: Vec<RewriteRule>,
    /// Maximum successful rewrite steps.
    pub max_steps: u64,
}

/// Typed fixed-point result from bounded term normalization.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RewriteNormalizeOutput {
    /// Reached normal form.
    pub normal_form: RewriteTerm,
    /// Number of successful rewrite steps.
    pub steps: u64,
}

/// Typed input for bounded inverse-rewrite predecessor search.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RewritePredecessorsOutput {
    /// Unique predecessor terms in canonical DTO order.
    pub predecessors: Vec<RewriteTerm>,
    /// Whether the requested result cap omitted at least one predecessor.
    pub truncated: bool,
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
