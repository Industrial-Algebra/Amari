//! First-order terms for term rewriting systems.

use alloc::{string::String, vec::Vec};
use core::fmt;

use crate::{Path, Rewritable, RewriteError, RewriteResult};

/// A first-order pattern variable.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Variable(String);

impl Variable {
    /// Create a variable from a name.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Borrow the variable name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Variable {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Variable {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// A first-order function or constant symbol.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(String);

impl Symbol {
    /// Create a symbol from a name.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Borrow the symbol name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Symbol {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Symbol {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// A first-order term.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Term {
    /// Pattern variable.
    Var(Variable),
    /// Function symbol with zero or more arguments.
    Sym(Symbol, Vec<Term>),
}

impl Term {
    /// Construct a variable term.
    pub fn var(name: impl Into<Variable>) -> Self {
        Self::Var(name.into())
    }

    /// Construct a constant symbol.
    pub fn constant(name: impl Into<Symbol>) -> Self {
        Self::Sym(name.into(), Vec::new())
    }

    /// Construct a symbol with arguments.
    pub fn sym(name: impl Into<Symbol>, args: impl IntoIterator<Item = Term>) -> Self {
        Self::Sym(name.into(), args.into_iter().collect())
    }

    /// Return true for variables.
    pub fn is_var(&self) -> bool {
        matches!(self, Self::Var(_))
    }

    /// Number of immediate arguments.
    pub fn arity(&self) -> usize {
        match self {
            Self::Var(_) => 0,
            Self::Sym(_, args) => args.len(),
        }
    }

    /// Borrow all valid positions in preorder.
    pub fn positions(&self) -> Vec<Path> {
        <Self as Rewritable>::positions(self)
    }

    /// Borrow a subterm by path.
    pub fn subterm(&self, path: &Path) -> Option<&Self> {
        <Self as Rewritable>::subterm(self, path)
    }

    /// Return a new term with a subterm replaced.
    pub fn replace_at(&self, path: &Path, replacement: Self) -> RewriteResult<Self> {
        <Self as Rewritable>::replace_at(self, path, replacement)
    }

    /// Collect variables occurring in this term.
    pub fn variables(&self) -> Vec<Variable> {
        let mut vars = Vec::new();
        self.collect_variables(&mut vars);
        vars.sort();
        vars.dedup();
        vars
    }

    fn collect_variables(&self, vars: &mut Vec<Variable>) {
        match self {
            Self::Var(var) => vars.push(var.clone()),
            Self::Sym(_, args) => {
                for arg in args {
                    arg.collect_variables(vars);
                }
            }
        }
    }
}

impl Rewritable for Term {
    fn child_count(&self) -> usize {
        self.arity()
    }

    fn child(&self, index: usize) -> Option<&Self> {
        match self {
            Self::Var(_) => None,
            Self::Sym(_, args) => args.get(index),
        }
    }

    fn replace_child(&self, index: usize, replacement: Self) -> RewriteResult<Self> {
        match self {
            Self::Var(_) => Err(RewriteError::InvalidChildIndex { index }),
            Self::Sym(symbol, args) => {
                if index >= args.len() {
                    return Err(RewriteError::InvalidChildIndex { index });
                }
                let mut next = args.clone();
                next[index] = replacement;
                Ok(Self::Sym(symbol.clone(), next))
            }
        }
    }
}
