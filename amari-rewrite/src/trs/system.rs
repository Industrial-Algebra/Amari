//! Term rewriting-system normalization.

use alloc::vec::Vec;

use crate::{Path, RewriteError, RewriteResult};

use super::{Rule, Term};

/// A first-order term rewriting system.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TermSystem {
    rules: Vec<Rule>,
}

impl TermSystem {
    /// Create a term system from checked TRS rules.
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    /// Borrow the rules.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Enumerate one-step successors for every rule at every position.
    pub fn successors(&self, term: &Term) -> RewriteResult<Vec<Term>> {
        let mut out = Vec::new();
        for path in term.positions() {
            let Some(subterm) = term.subterm(&path) else {
                return Err(RewriteError::InvalidPath);
            };
            for rule in &self.rules {
                if let Some(replacement) = rule.apply_root(subterm) {
                    let rewritten = term.replace_at(&path, replacement)?;
                    if rewritten != *term {
                        out.push(rewritten);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Apply the first outermost one-step rewrite.
    pub fn apply_once(&self, term: &Term) -> RewriteResult<Option<Term>> {
        for path in term.positions() {
            if let Some(next) = self.apply_once_at(term, &path)? {
                return Ok(Some(next));
            }
        }
        Ok(None)
    }

    /// Apply the first matching rule at a specific path.
    pub fn apply_once_at(&self, term: &Term, path: &Path) -> RewriteResult<Option<Term>> {
        let Some(subterm) = term.subterm(path) else {
            return Err(RewriteError::InvalidPath);
        };
        for rule in &self.rules {
            if let Some(replacement) = rule.apply_root(subterm) {
                let rewritten = term.replace_at(path, replacement)?;
                if rewritten != *term {
                    return Ok(Some(rewritten));
                }
            }
        }
        Ok(None)
    }

    /// Normalize with a conservative default step limit.
    pub fn normalize(&self, term: &Term) -> RewriteResult<Term> {
        self.normalize_with_limit(term, 1024)
    }

    /// Normalize until fixed point or step limit.
    pub fn normalize_with_limit(&self, term: &Term, max_steps: usize) -> RewriteResult<Term> {
        let mut current = term.clone();
        for _ in 0..max_steps {
            match self.apply_once(&current)? {
                Some(next) => current = next,
                None => return Ok(current),
            }
        }

        if self.apply_once(&current)?.is_some() {
            Err(RewriteError::StepLimitReached)
        } else {
            Ok(current)
        }
    }
}
