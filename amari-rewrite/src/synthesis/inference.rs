//! Rule inference from example rewrite steps.

use alloc::{collections::BTreeMap, string::String, vec::Vec};

use crate::{
    trs::{Rule, Term, Variable},
    RewriteError, RewriteResult,
};

/// Infer a single checked TRS rule from positive `(before, after)` examples.
///
/// This lightweight 0.23.0 inference path anti-unifies the before terms and
/// the after terms with a shared disagreement table. Shared concrete
/// disagreements across the left and right side therefore receive the same
/// generated variable, which is enough for common algebraic identities such as
/// `add(0, X) -> X`.
pub fn infer_rule(examples: &[(Term, Term)]) -> RewriteResult<Rule> {
    if examples.is_empty() {
        return Err(RewriteError::InvalidRule {
            message: String::from("cannot infer a rule from empty examples"),
        });
    }

    let mut generalizer = PairGeneralizer::new();
    let lhs_terms: Vec<Term> = examples.iter().map(|(lhs, _)| lhs.clone()).collect();
    let rhs_terms: Vec<Term> = examples.iter().map(|(_, rhs)| rhs.clone()).collect();
    let lhs = generalizer
        .anti_unify_all(&lhs_terms)
        .expect("non-empty lhs terms");
    let rhs = generalizer
        .anti_unify_all(&rhs_terms)
        .expect("non-empty rhs terms");

    Rule::new(lhs, rhs)
}

/// Infer one or more rules from positive and negative examples.
///
/// The first 0.23.0 implementation returns the positive-example rule when it
/// does not reproduce any negative example. More complete specialization is
/// intentionally deferred.
pub fn infer_rules(
    positives: &[(Term, Term)],
    negatives: &[(Term, Term)],
) -> RewriteResult<Vec<Rule>> {
    let rule = infer_rule(positives)?;
    let covers_negative = negatives.iter().any(|(lhs, rhs)| {
        rule.apply_root(lhs)
            .map(|produced| produced == *rhs)
            .unwrap_or(false)
    });

    if covers_negative {
        return Err(RewriteError::InvalidRule {
            message: String::from("inferred rule covers a negative example"),
        });
    }

    Ok(vec![rule])
}

#[derive(Clone, Debug, Default)]
struct PairGeneralizer {
    next: usize,
    disagreements: BTreeMap<(Term, Term), Variable>,
}

impl PairGeneralizer {
    fn new() -> Self {
        Self::default()
    }

    fn anti_unify_all(&mut self, terms: &[Term]) -> Option<Term> {
        let mut iter = terms.iter();
        let mut current = iter.next()?.clone();
        for term in iter {
            current = self.anti_unify(&current, term);
        }
        Some(current)
    }

    fn anti_unify(&mut self, left: &Term, right: &Term) -> Term {
        if left == right {
            return left.clone();
        }

        match (left, right) {
            (Term::Sym(left_symbol, left_args), Term::Sym(right_symbol, right_args))
                if left_symbol == right_symbol && left_args.len() == right_args.len() =>
            {
                Term::Sym(
                    left_symbol.clone(),
                    left_args
                        .iter()
                        .zip(right_args)
                        .map(|(left_arg, right_arg)| self.anti_unify(left_arg, right_arg))
                        .collect(),
                )
            }
            _ => Term::Var(self.variable_for(left, right)),
        }
    }

    fn variable_for(&mut self, left: &Term, right: &Term) -> Variable {
        let key = (left.clone(), right.clone());
        if let Some(var) = self.disagreements.get(&key) {
            return var.clone();
        }

        let var = Variable::new(format_alloc("_I", self.next));
        self.next += 1;
        self.disagreements.insert(key, var.clone());
        var
    }
}

fn format_alloc(prefix: &str, index: usize) -> String {
    let mut out = String::from(prefix);
    // Avoid requiring std formatting machinery in this small no_std-friendly helper.
    let digits = index.to_string();
    out.push_str(&digits);
    out
}
