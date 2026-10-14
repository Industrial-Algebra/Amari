// SPDX-License-Identifier: MIT OR Apache-2.0

//! The typed residual and its validating replay.

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::relation::digest::encode_term;
use crate::relation::{
    BackwardClause, LogicVar, RelationLimits, RelationResources, RuleId, Sha256Digest,
};
use crate::rewritable::Path;
use crate::trs::{match_pattern, Substitution, Term, TermSystem, Variable};

/// Authority produced during a concrete forward step, sufficient for
/// exact backward replay.
///
/// Binding keys are schema positions (`LogicVar::new(0, i)` over the
/// rule's sorted erased-variable names), not query-freshened
/// variables.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct RewriteResidual {
    /// Identity of the checked rule that fired.
    pub rule_id: RuleId,
    /// Position of the replaced subterm.
    pub position: Path,
    /// Bindings for variables the forward step erased.
    pub erased_bindings: Vec<(LogicVar, Term)>,
    /// Canonical digest of the source term.
    pub source_hash: Sha256Digest,
    /// Canonical digest of the target term.
    pub target_hash: Sha256Digest,
}

fn mismatch(message: &str) -> RewriteError {
    RewriteError::ResidualMismatch {
        message: String::from(message),
    }
}

impl RewriteResidual {
    /// Deterministic authority digest of the residual itself.
    pub fn digest(&self) -> Sha256Digest {
        let mut encoding = Vec::new();
        encoding.extend_from_slice(self.rule_id.digest().as_bytes());
        let indices = self.position.as_slice();
        encoding.extend_from_slice(&(indices.len() as u32).to_le_bytes());
        for index in indices {
            encoding.extend_from_slice(&(*index as u32).to_le_bytes());
        }
        encoding.extend_from_slice(&(self.erased_bindings.len() as u32).to_le_bytes());
        for (key, term) in &self.erased_bindings {
            encoding.extend_from_slice(&key.scope().to_le_bytes());
            encoding.extend_from_slice(&key.index().to_le_bytes());
            let mut variables = Vec::new();
            encode_term(term, &mut variables, &mut encoding);
        }
        encoding.extend_from_slice(self.source_hash.as_bytes());
        encoding.extend_from_slice(self.target_hash.as_bytes());
        Sha256Digest::framed("amari.relation.residual/v1", &encoding)
    }

    /// Validating replay: reconstruct the source term from `target`.
    ///
    /// Validates (in order) target hash, rule identity, position,
    /// binding schema, binding term limits, and RHS match; then
    /// reconstructs privately and returns only when the canonical
    /// source hash matches exactly. Any mismatch is a hard typed
    /// [`RewriteError::ResidualMismatch`]; no degraded or partial
    /// reconstruction is ever returned.
    pub fn replay(
        &self,
        system: &TermSystem,
        target: &Term,
        limits: &RelationLimits,
    ) -> RewriteResult<Term> {
        let mut resources = RelationResources::new(limits);
        resources.record_operations(1)?;

        // Target authority.
        let target_hash = Sha256Digest::canonical_term("amari.relation.term/v1", target);
        if target_hash != self.target_hash {
            return Err(mismatch("target hash does not match"));
        }

        // Rule identity.
        let rule = system
            .rules()
            .iter()
            .find(|rule| RuleId::from_rule(rule) == self.rule_id)
            .ok_or_else(|| mismatch("rule identity not present in system"))?;

        // Position validity.
        let subterm = target
            .subterm(&self.position)
            .ok_or_else(|| mismatch("position is not valid in target"))?;

        // Binding schema: count, key positions, uniqueness.
        let clause = BackwardClause::compile(rule)?;
        let erased = clause.erased_variables();
        if self.erased_bindings.len() != erased.len() {
            return Err(mismatch("binding schema length mismatch"));
        }
        for (index, (key, bound)) in self.erased_bindings.iter().enumerate() {
            if *key != LogicVar::new(0, index as u32) {
                return Err(mismatch("binding schema key mismatch"));
            }
            resources.record_term(term_nodes(bound), term_depth(bound))?;
        }

        // Recover RHS variables by matching the target subterm.
        let recovered = match_pattern(rule.rhs(), subterm)
            .ok_or_else(|| mismatch("rule RHS does not match target subterm"))?;

        // Reconstruct privately: recovered RHS bindings ∪ erased.
        let mut full = Substitution::new();
        for (variable, term) in recovered.iter() {
            full.insert(variable.clone(), term.clone());
        }
        for ((_, bound), name) in self.erased_bindings.iter().zip(erased.iter()) {
            full.insert(Variable::new(name.clone()), bound.clone());
        }
        let source_subterm = full.apply_matcher(rule.lhs());
        let reconstructed = target
            .replace_at(&self.position, source_subterm)
            .map_err(|_| mismatch("position replacement failed"))?;

        // Exact authority comparison before returning anything.
        let source_hash = Sha256Digest::canonical_term("amari.relation.term/v1", &reconstructed);
        if source_hash != self.source_hash {
            return Err(mismatch("reconstructed source hash mismatch"));
        }
        Ok(reconstructed)
    }
}

fn term_nodes(term: &Term) -> usize {
    match term {
        Term::Var(_) => 1,
        Term::Sym(_, args) => 1 + args.iter().map(term_nodes).sum::<usize>(),
    }
}

fn term_depth(term: &Term) -> usize {
    match term {
        Term::Var(_) => 1,
        Term::Sym(_, args) => 1 + args.iter().map(term_depth).max().unwrap_or(0),
    }
}
