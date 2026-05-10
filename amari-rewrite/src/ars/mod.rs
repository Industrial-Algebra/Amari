//! Abstract rewriting systems over user-owned rewritable values.

use alloc::{boxed::Box, string::String, vec::Vec};

use crate::{Path, Rewritable, RewriteError, RewriteResult};

/// A single abstract rewrite rule over values of type `T`.
pub struct Rule<T> {
    name: String,
    apply: Box<dyn Fn(&T) -> Option<T>>,
}

impl<T> Rule<T> {
    /// Create a rule from a name and pure rewrite closure.
    pub fn new(name: impl Into<String>, apply: impl Fn(&T) -> Option<T> + 'static) -> Self {
        Self {
            name: name.into(),
            apply: Box::new(apply),
        }
    }

    /// Rule name for diagnostics and traces.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Try to apply this rule at a root value.
    pub fn try_apply(&self, term: &T) -> Option<T> {
        (self.apply)(term)
    }
}

/// Strategy used when selecting a single rewrite step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    /// Try outer positions before inner positions.
    OuterFirst,
    /// Try inner positions before outer positions.
    InnerFirst,
    /// Try the first rule across positions using outer-first traversal.
    FirstRule,
    /// Enumerate all one-step successors rather than choosing one.
    All,
}

impl Default for Strategy {
    fn default() -> Self {
        Self::OuterFirst
    }
}

/// A concrete rewrite step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewriteStep<T> {
    /// Rule name that produced the step.
    pub rule: String,
    /// Position rewritten.
    pub path: Path,
    /// Resulting term.
    pub result: T,
}

/// Abstract rewrite system over values of type `T`.
pub struct System<T> {
    rules: Vec<Rule<T>>,
}

impl<T> System<T> {
    /// Create a system from a list of rules.
    pub fn new(rules: Vec<Rule<T>>) -> Self {
        Self { rules }
    }

    /// Borrow the rules.
    pub fn rules(&self) -> &[Rule<T>] {
        &self.rules
    }
}

impl<T: Rewritable> System<T> {
    /// Enumerate every one-step successor under every rule and position.
    pub fn successors(&self, term: &T) -> RewriteResult<Vec<T>> {
        let mut out = Vec::new();
        for path in term.positions() {
            let Some(subterm) = term.subterm(&path) else {
                return Err(RewriteError::InvalidPath);
            };
            for rule in &self.rules {
                if let Some(replacement) = rule.try_apply(subterm) {
                    let rewritten = term.replace_at(&path, replacement)?;
                    if rewritten != *term {
                        out.push(rewritten);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Enumerate detailed one-step successors.
    pub fn steps(&self, term: &T) -> RewriteResult<Vec<RewriteStep<T>>> {
        let mut out = Vec::new();
        for path in term.positions() {
            let Some(subterm) = term.subterm(&path) else {
                return Err(RewriteError::InvalidPath);
            };
            for rule in &self.rules {
                if let Some(replacement) = rule.try_apply(subterm) {
                    let rewritten = term.replace_at(&path, replacement)?;
                    if rewritten != *term {
                        out.push(RewriteStep {
                            rule: rule.name().into(),
                            path: path.clone(),
                            result: rewritten,
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    /// Apply one rewrite step using `strategy`.
    pub fn apply_once(&self, term: &T, strategy: Strategy) -> RewriteResult<Option<T>> {
        let mut positions = term.positions();
        if matches!(strategy, Strategy::InnerFirst) {
            positions.reverse();
        }

        if matches!(strategy, Strategy::FirstRule) {
            for rule in &self.rules {
                for path in &positions {
                    let Some(subterm) = term.subterm(path) else {
                        return Err(RewriteError::InvalidPath);
                    };
                    if let Some(replacement) = rule.try_apply(subterm) {
                        let rewritten = term.replace_at(path, replacement)?;
                        if rewritten != *term {
                            return Ok(Some(rewritten));
                        }
                    }
                }
            }
            return Ok(None);
        }

        for path in positions {
            let Some(subterm) = term.subterm(&path) else {
                return Err(RewriteError::InvalidPath);
            };
            for rule in &self.rules {
                if let Some(replacement) = rule.try_apply(subterm) {
                    let rewritten = term.replace_at(&path, replacement)?;
                    if rewritten != *term {
                        return Ok(Some(rewritten));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Normalize with the default outer-first strategy.
    pub fn normalize(&self, term: &T) -> RewriteResult<T> {
        self.normalize_with_limit(term, 1024)
    }

    /// Repeatedly rewrite until a fixed point is reached or the step limit is exhausted.
    pub fn normalize_with_limit(&self, term: &T, max_steps: usize) -> RewriteResult<T> {
        let mut current = term.clone();
        for _ in 0..max_steps {
            match self.apply_once(&current, Strategy::OuterFirst)? {
                Some(next) => current = next,
                None => return Ok(current),
            }
        }

        if self.apply_once(&current, Strategy::OuterFirst)?.is_some() {
            Err(RewriteError::StepLimitReached)
        } else {
            Ok(current)
        }
    }
}
