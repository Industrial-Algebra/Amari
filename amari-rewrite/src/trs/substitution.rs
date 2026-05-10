//! Substitutions for first-order terms.

use alloc::{collections::BTreeMap, string::String};

use super::{Term, Variable};

/// A variable-to-term substitution.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Substitution {
    map: BTreeMap<Variable, Term>,
}

impl Substitution {
    /// Create an empty substitution.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a binding, returning the updated substitution.
    pub fn with(mut self, variable: impl Into<Variable>, term: Term) -> Self {
        self.insert(variable, term);
        self
    }

    /// Insert or replace a binding.
    pub fn insert(&mut self, variable: impl Into<Variable>, term: Term) -> Option<Term> {
        self.map.insert(variable.into(), term)
    }

    /// Borrow a binding by variable.
    pub fn get(&self, variable: &str) -> Option<&Term> {
        self.map.get(&Variable::new(String::from(variable)))
    }

    /// Borrow a binding by typed variable.
    pub fn get_var(&self, variable: &Variable) -> Option<&Term> {
        self.map.get(variable)
    }

    /// Iterate bindings.
    pub fn iter(&self) -> impl Iterator<Item = (&Variable, &Term)> {
        self.map.iter()
    }

    /// Apply this substitution recursively to `term`.
    pub fn apply(&self, term: &Term) -> Term {
        match term {
            Term::Var(var) => self.map.get(var).cloned().unwrap_or_else(|| term.clone()),
            Term::Sym(symbol, args) => Term::Sym(
                symbol.clone(),
                args.iter().map(|arg| self.apply(arg)).collect(),
            ),
        }
    }
}
