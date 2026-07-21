// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded DTO validation shared by registered rewrite probes.

// Task 20A intentionally lands validation primitives before the executable
// adapters in Tasks 20B-20D; remove this transition allowance once registered.
#![cfg_attr(any(not(test), not(feature = "standard-probes")), allow(dead_code))]

#[cfg(feature = "standard-probes")]
use std::collections::BTreeMap;

#[cfg(feature = "standard-probes")]
use amari_rewrite::trs::{Rule, Term};
use serde::{Deserialize, Serialize};

#[cfg(feature = "standard-probes")]
use crate::{DiscoveryError, DiscoveryResult};

#[cfg(feature = "standard-probes")]
const MAX_NAME_BYTES: usize = 256;

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
