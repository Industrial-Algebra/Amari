//! Anti-unification for first-order terms.
//!
//! Anti-unification computes a most-specific generalization of terms. Fresh
//! variables mark positions where input terms disagree.

use alloc::{format, vec::Vec};

use crate::trs::Term;

/// Fresh variable generator for anti-unification.
#[derive(Clone, Debug, Default)]
pub struct VarGen {
    next: usize,
}

impl VarGen {
    /// Create a new generator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a fresh generated variable term.
    pub fn fresh(&mut self) -> Term {
        let name = format!("_G{}", self.next);
        self.next += 1;
        Term::var(name)
    }

    /// Anti-unify two terms using this generator.
    pub fn anti_unify(&mut self, left: &Term, right: &Term) -> Term {
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
            _ => self.fresh(),
        }
    }
}

/// Anti-unify two terms.
pub fn anti_unify(left: &Term, right: &Term) -> Term {
    VarGen::new().anti_unify(left, right)
}

/// Anti-unify a non-empty slice of terms.
pub fn anti_unify_all(terms: &[Term]) -> Option<Term> {
    let mut iter = terms.iter();
    let mut current = iter.next()?.clone();
    let mut var_gen = VarGen::new();
    for term in iter {
        current = var_gen.anti_unify(&current, term);
    }
    Some(current)
}

/// Anti-unify all terms from an iterator by collecting them first.
pub fn anti_unify_iter(terms: impl IntoIterator<Item = Term>) -> Option<Term> {
    let terms: Vec<Term> = terms.into_iter().collect();
    anti_unify_all(&terms)
}
