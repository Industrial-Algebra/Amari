// SPDX-License-Identifier: MIT OR Apache-2.0

//! Alpha-canonical symbolic search states.
//!
//! A [`SymbolicState`] pairs a term (with freshened logic variables)
//! with its constraint set. Equality, ordering, and hashing use a
//! joint alpha-canonical digest: variables are renamed by first
//! occurrence across the term AND the constraints in one pass, so
//! alpha-equivalent states deduplicate while constraint differences
//! always distinguish states.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use core::cmp::Ordering;
use core::hash::{Hash, Hasher};

use crate::relation::digest::encode_term;
use crate::relation::{ConstraintSet, Sha256Digest, TermConstraint};
use crate::trs::Term;
use crate::RewriteError;

/// A symbolic search state: term plus residual constraints, compared
/// by joint alpha-canonical digest.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct SymbolicState {
    term: Term,
    constraints: ConstraintSet,
    digest: Sha256Digest,
}

impl SymbolicState {
    /// Construct a state, computing its joint alpha-canonical digest.
    pub fn new(term: Term, constraints: ConstraintSet) -> Self {
        let digest = canonical_state_digest(&term, &constraints);
        Self {
            term,
            constraints,
            digest,
        }
    }

    /// The state term (freshened variables as derived).
    pub fn term(&self) -> &Term {
        &self.term
    }

    /// Residual constraints on this state.
    pub fn constraints(&self) -> &ConstraintSet {
        &self.constraints
    }

    /// Joint alpha-canonical digest: the state's identity.
    pub fn canonical_digest(&self) -> Sha256Digest {
        self.digest
    }

    /// Accounting estimate of the bytes this state retains: the
    /// 32-byte digest plus a per-node term charge and a
    /// per-constraint charge. Used for frontier byte budgets.
    pub fn retained_bytes(&self) -> u64 {
        let nodes = self.term.positions().len() as u64;
        32 + 16 * nodes + 24 * self.constraints.len() as u64
    }
}

/// Joint alpha-canonical encoding: one variable-renaming pass shared
/// by the term and every constraint, with constraint sides
/// canonicalized (symmetric equality/disequality) and constraint
/// encodings sorted.
///
/// Constraint order must not influence the digest: the shared
/// renaming pass allocates variable indices in visitation order, so
/// constraints are first placed in a canonical order. Each
/// constraint gets a wildcard *shape key* (variables erased, sides
/// canonicalized); runs of equal shapes (whose relative order a
/// sort cannot decide) are enumerated exhaustively and the
/// lexicographically smallest full encoding wins. Enumeration is
/// capped at 720 arrangements; beyond that the (still deterministic,
/// still sound) baseline order is used. Order-dependence can only
/// ever cause a missed dedup, never a false equality: two states
/// are equal only when their final encodings are identical.
fn canonical_state_digest(term: &Term, constraints: &ConstraintSet) -> Sha256Digest {
    let constraints = constraints.constraints();
    // Decorate each constraint with its wildcard shape key and sort
    // stably: within a tie, the caller's iteration order is the
    // deterministic baseline.
    let mut decorated: Vec<(Vec<u8>, &TermConstraint)> = constraints
        .iter()
        .map(|constraint| (constraint_shape_key(constraint), constraint))
        .collect();
    decorated.sort_by(|left, right| left.0.cmp(&right.0));
    // Group maximal runs of equal shape keys.
    let mut runs: Vec<core::ops::Range<usize>> = Vec::new();
    let mut start = 0;
    for index in 1..=decorated.len() {
        if index == decorated.len() || decorated[index].0 != decorated[start].0 {
            runs.push(start..index);
            start = index;
        }
    }
    // Total arrangement count (product of run factorials), capped.
    let mut arrangements: u64 = 1;
    for run in &runs {
        let size = (run.end - run.start) as u64;
        let mut factorial: u64 = 1;
        for factor in 2..=size {
            factorial = factorial.saturating_mul(factor);
        }
        arrangements = arrangements.saturating_mul(factorial);
        if arrangements > 720 {
            break;
        }
    }
    let mut best: Option<Vec<u8>> = None;
    if arrangements <= 720 && !runs.is_empty() {
        // Enumerate arrangements: an odometer over per-run
        // permutation indices (permutations are lexicographic).
        let run_perms: Vec<Vec<Vec<usize>>> = runs
            .iter()
            .map(|run| permutations(run.end - run.start))
            .collect();
        let mut odometer = vec![0usize; runs.len()];
        'arrangements: loop {
            let mut order: Vec<&TermConstraint> = Vec::with_capacity(constraints.len());
            for (slot, run) in runs.iter().enumerate() {
                for &local in &run_perms[slot][odometer[slot]] {
                    order.push(decorated[run.start + local].1);
                }
            }
            let encoding = encode_state_with_order(term, &order);
            if best.as_ref().is_none_or(|current| encoding < *current) {
                best = Some(encoding);
            }
            // Advance the odometer; overflow at slot 0 ends the
            // enumeration.
            let mut slot = runs.len();
            while slot > 0 {
                slot -= 1;
                odometer[slot] += 1;
                if odometer[slot] < run_perms[slot].len() {
                    break;
                }
                odometer[slot] = 0;
                if slot == 0 {
                    break 'arrangements;
                }
            }
        }
    }
    if let Some(encoding) = best {
        return Sha256Digest::framed("amari.relation.symbolic-state/v1", &encoding);
    }
    // Fallback (only reachable above the enumeration cap): the
    // shape-sorted baseline order.
    let order: Vec<&TermConstraint> = decorated
        .iter()
        .map(|(_, constraint)| *constraint)
        .collect();
    let encoding = encode_state_with_order(term, &order);
    Sha256Digest::framed("amari.relation.symbolic-state/v1", &encoding)
}

/// Encode the term and the constraints (in the given order) with
/// one shared renaming pass, returning the full pre-digest bytes.
fn encode_state_with_order(term: &Term, order: &[&TermConstraint]) -> Vec<u8> {
    let mut variables: Vec<String> = Vec::new();
    let mut encoding = Vec::new();
    encode_term(term, &mut variables, &mut encoding);
    let mut constraint_blobs: Vec<Vec<u8>> = Vec::new();
    for constraint in order {
        let (tag, left, right) = match constraint {
            TermConstraint::Equal(left, right) => (0x10u8, left, right),
            TermConstraint::NotEqual(left, right) => (0x11u8, left, right),
        };
        let mut left_bytes = Vec::new();
        encode_term(left, &mut variables, &mut left_bytes);
        let mut right_bytes = Vec::new();
        encode_term(right, &mut variables, &mut right_bytes);
        let mut blob = vec![tag];
        if left_bytes <= right_bytes {
            blob.extend_from_slice(&left_bytes);
            blob.extend_from_slice(&right_bytes);
        } else {
            blob.extend_from_slice(&right_bytes);
            blob.extend_from_slice(&left_bytes);
        }
        constraint_blobs.push(blob);
    }
    constraint_blobs.sort();
    encoding.extend_from_slice(&(constraint_blobs.len() as u32).to_le_bytes());
    for blob in constraint_blobs {
        encoding.extend_from_slice(&(blob.len() as u32).to_le_bytes());
        encoding.extend_from_slice(&blob);
    }
    encoding
}

/// Wildcard shape key for a constraint: the same encoding scheme as
/// `encode_term`, but every variable maps to one marker and sides
/// are canonicalized, so the key is invariant under alpha-renaming
/// and insertion order.
fn constraint_shape_key(constraint: &TermConstraint) -> Vec<u8> {
    fn encode_shape(term: &Term, out: &mut Vec<u8>) {
        use alloc::string::ToString;
        match term {
            Term::Var(_) => {
                out.push(0x00);
                out.extend_from_slice(&0u32.to_le_bytes());
            }
            Term::Sym(symbol, args) => {
                out.push(0x01);
                let name = symbol.to_string();
                out.extend_from_slice(&(args.len() as u32).to_le_bytes());
                out.extend_from_slice(&(name.len() as u32).to_le_bytes());
                out.extend_from_slice(name.as_bytes());
                for arg in args {
                    encode_shape(arg, out);
                }
            }
        }
    }
    let (tag, left, right) = match constraint {
        TermConstraint::Equal(left, right) => (0x10u8, left, right),
        TermConstraint::NotEqual(left, right) => (0x11u8, left, right),
    };
    let mut left_bytes = Vec::new();
    encode_shape(left, &mut left_bytes);
    let mut right_bytes = Vec::new();
    encode_shape(right, &mut right_bytes);
    let mut key = vec![tag];
    if left_bytes <= right_bytes {
        key.extend_from_slice(&left_bytes);
        key.extend_from_slice(&right_bytes);
    } else {
        key.extend_from_slice(&right_bytes);
        key.extend_from_slice(&left_bytes);
    }
    key
}

/// All permutations of `0..size`, in lexicographic order.
fn permutations(size: usize) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    let mut current: Vec<usize> = (0..size).collect();
    loop {
        result.push(current.clone());
        // Next lexicographic permutation.
        let mut pivot = None;
        for index in (0..current.len().saturating_sub(1)).rev() {
            if current[index] < current[index + 1] {
                pivot = Some(index);
                break;
            }
        }
        let Some(pivot) = pivot else { break };
        let mut successor = current.len() - 1;
        while current[successor] <= current[pivot] {
            successor -= 1;
        }
        current.swap(pivot, successor);
        current[pivot + 1..].reverse();
    }
    result
}

impl PartialEq for SymbolicState {
    fn eq(&self, other: &Self) -> bool {
        self.digest == other.digest
    }
}

impl Eq for SymbolicState {}

impl PartialOrd for SymbolicState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SymbolicState {
    fn cmp(&self, other: &Self) -> Ordering {
        self.digest.as_bytes().cmp(other.digest.as_bytes())
    }
}

impl Hash for SymbolicState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.digest.as_bytes().hash(state);
    }
}

/// Search resource counters with ceiling enforcement.
#[derive(Clone, Debug)]
pub struct SearchResources {
    max_states: u64,
    max_transitions: u64,
    max_retained_bytes: u64,
    max_trace_bytes: u64,
    max_operations: u64,
    states: u64,
    transitions: u64,
    retained_bytes: u64,
    trace_bytes: u64,
    operations: u64,
}

impl SearchResources {
    /// Counters bound to a search configuration.
    pub fn new(config: &crate::inverse::InverseSearchConfig) -> Self {
        Self {
            max_states: config.max_states(),
            max_transitions: config.max_transitions(),
            max_retained_bytes: config.max_frontier_bytes(),
            max_trace_bytes: config.max_trace_bytes(),
            max_operations: config.max_operations(),
            states: 0,
            transitions: 0,
            retained_bytes: 0,
            trace_bytes: 0,
            operations: 0,
        }
    }

    fn exceeded(resource: &'static str, limit: u64) -> RewriteError {
        RewriteError::RelationLimitExceeded {
            resource,
            limit: limit as usize,
        }
    }

    /// Account one retained state.
    pub fn record_state(&mut self) -> crate::RewriteResult<()> {
        if self.states >= self.max_states {
            return Err(Self::exceeded("search states", self.max_states));
        }
        self.states += 1;
        Ok(())
    }

    /// Account one explored transition.
    pub fn record_transition(&mut self) -> crate::RewriteResult<()> {
        if self.transitions >= self.max_transitions {
            return Err(Self::exceeded("search transitions", self.max_transitions));
        }
        self.transitions += 1;
        Ok(())
    }

    /// Account retained frontier bytes.
    pub fn record_bytes(&mut self, bytes: u64) -> crate::RewriteResult<()> {
        let next = self.retained_bytes.saturating_add(bytes);
        if next > self.max_retained_bytes {
            return Err(Self::exceeded(
                "search retained bytes",
                self.max_retained_bytes,
            ));
        }
        self.retained_bytes = next;
        Ok(())
    }

    /// Account trace bytes.
    pub fn record_trace(&mut self, bytes: u64) -> crate::RewriteResult<()> {
        let next = self.trace_bytes.saturating_add(bytes);
        if next > self.max_trace_bytes {
            return Err(Self::exceeded("search trace bytes", self.max_trace_bytes));
        }
        self.trace_bytes = next;
        Ok(())
    }

    /// Account abstract operations.
    pub fn record_operations(&mut self, count: u64) -> crate::RewriteResult<()> {
        let next = self.operations.saturating_add(count);
        if next > self.max_operations {
            return Err(Self::exceeded("search operations", self.max_operations));
        }
        self.operations = next;
        Ok(())
    }

    /// States retained so far.
    pub fn states(&self) -> u64 {
        self.states
    }

    /// Transitions explored so far.
    pub fn transitions(&self) -> u64 {
        self.transitions
    }

    /// Retained frontier bytes so far.
    pub fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }

    /// Trace bytes recorded so far.
    pub fn trace_bytes(&self) -> u64 {
        self.trace_bytes
    }

    /// Operations spent so far.
    pub fn operations(&self) -> u64 {
        self.operations
    }
}
