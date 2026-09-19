// SPDX-License-Identifier: MIT OR Apache-2.0

//! Regular tree grammars: a checked authored API over the validated
//! tree-automaton core.
//!
//! A regular tree grammar is a set of productions
//! `A -> f A1 ... Ak` whose language is the set of ground terms
//! derivable from the start nonterminals. Grammars are the authored
//! surface; all language operations (membership, witness, emptiness,
//! closure constructions) route through the checked conversion to
//! [`TreeAutomaton`] rather than duplicating algorithms.
//!
//! ## Fixed text syntax
//!
//! ```text
//! amari-tree-grammar/v1
//! nonterminals: S A
//! start: S
//! S -> add S S
//! A -> num
//! ```
//!
//! The header line is mandatory. `nonterminals:` declares the
//! nonterminal set; every production lhs, production child, and
//! `start:` name must be declared. A production's symbol rank is the
//! number of children on that line; each symbol name has ONE rank per
//! grammar (the automaton alphabet's single-rank invariant), and
//! conflicting ranks are rejected at construction/parse.
//! Blank lines are ignored. [`RegularTreeGrammar::render`] emits the
//! canonical form: declarations first, nonterminals and starts
//! sorted, productions in canonical sorted order, exactly one
//! trailing newline.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::language::automaton::TreeAutomaton;
use crate::language::{RankedSymbol, TreeAutomatonLimits, TreeState, TreeTransition};
use crate::trs::{Symbol, Term};

/// The mandatory first line of the fixed grammar text syntax.
pub const GRAMMAR_SYNTAX_HEADER: &str = "amari-tree-grammar/v1";

/// Whether a name can appear in the fixed text syntax: non-empty,
/// free of whitespace, not the arrow token, and not carrying a
/// directive prefix (which would be misparsed as a `start:` or
/// `nonterminals:` declaration line).
fn renderable_name(name: &str) -> bool {
    !name.is_empty()
        && name != "->"
        && !name.starts_with("start:")
        && !name.starts_with("nonterminals:")
        && !name.chars().any(char::is_whitespace)
}

/// A grammar nonterminal. Names must be renderable in the fixed
/// syntax: non-empty, free of whitespace, not the arrow token, and
/// without a directive prefix.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct Nonterminal(Symbol);

impl Nonterminal {
    /// Create a nonterminal from a name. Construction of the name
    /// itself is infallible; renderability is validated by
    /// [`RegularTreeGrammar::new`].
    pub fn new(name: impl Into<String>) -> Self {
        Self(Symbol::new(name.into()))
    }

    /// Borrow the nonterminal name.
    pub fn name(&self) -> &Symbol {
        &self.0
    }

    /// Whether the name can appear in the fixed text syntax.
    fn is_renderable(&self) -> bool {
        renderable_name(self.0.as_str())
    }
}

/// A production `lhs -> symbol(children...)`. Checked construction
/// requires `children.len() == symbol.arity()`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct GrammarProduction {
    lhs: Nonterminal,
    symbol: RankedSymbol,
    children: Vec<Nonterminal>,
}

impl GrammarProduction {
    /// Checked construction: the child count must match the symbol
    /// rank.
    pub fn new(
        lhs: Nonterminal,
        symbol: RankedSymbol,
        children: Vec<Nonterminal>,
    ) -> RewriteResult<Self> {
        if children.len() != usize::from(symbol.arity()) {
            return Err(RewriteError::MalformedGrammar {
                message: format!(
                    "production {} -> {} has {} children but rank {}",
                    lhs.name(),
                    symbol.symbol(),
                    children.len(),
                    symbol.arity()
                ),
            });
        }
        if !renderable_name(symbol.symbol().as_str()) {
            return Err(RewriteError::MalformedGrammar {
                message: format!(
                    "terminal symbol {:?} is not renderable in the grammar syntax",
                    symbol.symbol().as_str()
                ),
            });
        }
        Ok(Self {
            lhs,
            symbol,
            children,
        })
    }

    /// The left-hand side.
    pub fn lhs(&self) -> &Nonterminal {
        &self.lhs
    }

    /// The production symbol with its rank.
    pub fn symbol(&self) -> &RankedSymbol {
        &self.symbol
    }

    /// The right-hand side children.
    pub fn children(&self) -> &[Nonterminal] {
        &self.children
    }
}

/// A validated regular tree grammar with canonical sorted storage.
/// Limits are stored and applied to every automaton conversion.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct RegularTreeGrammar {
    nonterminals: Vec<Nonterminal>,
    productions: Vec<GrammarProduction>,
    starts: Vec<Nonterminal>,
    limits: TreeAutomatonLimits,
}

impl RegularTreeGrammar {
    /// Checked construction: validates nonterminal names, start and
    /// production references, rank consistency, and ceilings
    /// (nonterminals count as states, productions as transitions),
    /// then stores everything in canonical sorted order.
    pub fn new(
        nonterminals: Vec<Nonterminal>,
        productions: Vec<GrammarProduction>,
        starts: Vec<Nonterminal>,
        limits: &TreeAutomatonLimits,
    ) -> RewriteResult<Self> {
        if nonterminals.len() > limits.max_states() {
            return Err(RewriteError::InvalidLimit {
                resource: "tree grammar nonterminals",
                value: nonterminals.len(),
                ceiling: limits.max_states(),
            });
        }
        if productions.len() > limits.max_transitions() {
            return Err(RewriteError::InvalidLimit {
                resource: "tree grammar productions",
                value: productions.len(),
                ceiling: limits.max_transitions(),
            });
        }
        let declared: BTreeSet<&Nonterminal> = nonterminals.iter().collect();
        if declared.len() != nonterminals.len() {
            return Err(RewriteError::MalformedGrammar {
                message: "duplicate nonterminal declaration".into(),
            });
        }
        for nonterminal in &nonterminals {
            if !nonterminal.is_renderable() {
                return Err(RewriteError::MalformedGrammar {
                    message: format!(
                        "nonterminal name {:?} is not renderable in the grammar syntax",
                        nonterminal.name().as_str()
                    ),
                });
            }
        }
        for start in &starts {
            if !declared.contains(start) {
                return Err(RewriteError::MalformedGrammar {
                    message: format!("start nonterminal {} is not declared", start.name()),
                });
            }
        }
        let mut start_set = BTreeSet::new();
        for start in &starts {
            if !start_set.insert(start) {
                return Err(RewriteError::MalformedGrammar {
                    message: format!("duplicate start nonterminal {}", start.name()),
                });
            }
        }
        for production in &productions {
            if production.symbol().arity() as usize > limits.max_rank() {
                return Err(RewriteError::InvalidLimit {
                    resource: "tree automaton rank",
                    value: usize::from(production.symbol().arity()),
                    ceiling: limits.max_rank(),
                });
            }
            if !declared.contains(&production.lhs) {
                return Err(RewriteError::MalformedGrammar {
                    message: format!(
                        "production lhs {} is not a declared nonterminal",
                        production.lhs().name()
                    ),
                });
            }
            for child in production.children() {
                if !declared.contains(child) {
                    return Err(RewriteError::MalformedGrammar {
                        message: format!(
                            "production child {} is not a declared nonterminal",
                            child.name()
                        ),
                    });
                }
            }
        }
        // One rank per symbol name (the automaton alphabet's
        // single-rank invariant); conflicting ranks are a grammar
        // error, not a surprise at automaton conversion time.
        let mut ranks: alloc::collections::BTreeMap<&Symbol, u16> =
            alloc::collections::BTreeMap::new();
        for production in &productions {
            let name = production.symbol().symbol();
            let arity = production.symbol().arity();
            match ranks.insert(name, arity) {
                None => {}
                Some(previous) if previous == arity => {}
                Some(previous) => {
                    return Err(RewriteError::MalformedGrammar {
                        message: format!(
                            "rank conflict for symbol {}: {previous} vs {arity}",
                            name.as_str()
                        ),
                    });
                }
            }
        }
        let mut nonterminals = nonterminals;
        let mut productions = productions;
        let mut starts = starts;
        nonterminals.sort();
        productions.sort();
        if productions.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(RewriteError::MalformedGrammar {
                message: "duplicate production".into(),
            });
        }
        starts.sort();
        starts.dedup();
        Ok(Self {
            nonterminals,
            productions,
            starts,
            limits: *limits,
        })
    }

    /// The declared nonterminals (canonical order).
    pub fn nonterminals(&self) -> &[Nonterminal] {
        &self.nonterminals
    }

    /// The productions (canonical order).
    pub fn productions(&self) -> &[GrammarProduction] {
        &self.productions
    }

    /// The start nonterminals (canonical order).
    pub fn starts(&self) -> &[Nonterminal] {
        &self.starts
    }

    /// The limits this grammar was validated against.
    pub fn limits(&self) -> &TreeAutomatonLimits {
        &self.limits
    }

    /// Lossless conversion to a tree automaton: nonterminals become
    /// states, productions become transitions (lhs is the parent),
    /// start nonterminals become final states.
    ///
    /// Documented normalization: a grammar carries no explicit
    /// alphabet, so the automaton's alphabet is DERIVED from the
    /// production symbols. An automaton whose declared alphabet
    /// contains symbols unused by any transition is normalized by an
    /// automaton→grammar→automaton round trip (unused symbols are
    /// dropped); states, transitions, and finals are preserved
    /// exactly.
    pub fn to_automaton(&self) -> RewriteResult<TreeAutomaton> {
        let mut alphabet: Vec<RankedSymbol> = self
            .productions
            .iter()
            .map(|production| production.symbol().clone())
            .collect();
        alphabet.sort();
        alphabet.dedup();
        let states: Vec<TreeState> = self
            .nonterminals
            .iter()
            .map(|nonterminal| TreeState::new(nonterminal.name().clone()))
            .collect();
        let transitions: Vec<TreeTransition> = self
            .productions
            .iter()
            .map(|production| {
                TreeTransition::new(
                    production.symbol().symbol().clone(),
                    production
                        .children()
                        .iter()
                        .map(|child| TreeState::new(child.name().clone()))
                        .collect(),
                    TreeState::new(production.lhs().name().clone()),
                )
            })
            .collect();
        let finals: Vec<TreeState> = self
            .starts
            .iter()
            .map(|start| TreeState::new(start.name().clone()))
            .collect();
        TreeAutomaton::new(alphabet, states, transitions, finals, self.limits)
    }

    /// Lossless conversion from a tree automaton using the
    /// automaton's own limits.
    pub fn from_automaton(automaton: &TreeAutomaton) -> RewriteResult<Self> {
        Self::from_automaton_with_limits(automaton, automaton.limits())
    }

    /// Lossless conversion from a tree automaton under explicit
    /// limits. Automaton state names must be renderable nonterminal
    /// names (a typed error otherwise).
    pub fn from_automaton_with_limits(
        automaton: &TreeAutomaton,
        limits: &TreeAutomatonLimits,
    ) -> RewriteResult<Self> {
        let nonterminals: Vec<Nonterminal> = automaton
            .states()
            .iter()
            .map(|state| Nonterminal::new(state.name().as_str()))
            .collect();
        let mut productions = Vec::with_capacity(automaton.transitions().len());
        for transition in automaton.transitions() {
            let arity = u16::try_from(transition.children().len()).map_err(|_| {
                RewriteError::MalformedGrammar {
                    message: "transition rank exceeds u16".into(),
                }
            })?;
            productions.push(GrammarProduction::new(
                Nonterminal::new(transition.parent().name().as_str()),
                RankedSymbol::new(transition.symbol().clone(), arity),
                transition
                    .children()
                    .iter()
                    .map(|child| Nonterminal::new(child.name().as_str()))
                    .collect(),
            )?);
        }
        let starts: Vec<Nonterminal> = automaton
            .finals()
            .iter()
            .map(|state| Nonterminal::new(state.name().as_str()))
            .collect();
        Self::new(nonterminals, productions, starts, limits)
    }

    /// Membership: does this grammar generate the term? Routes
    /// through the automaton conversion (ground terms only).
    pub fn accepts(&self, term: &Term) -> RewriteResult<bool> {
        self.to_automaton()?.accepts(term)
    }

    /// The lexicographically smallest generated term, or `None` if
    /// the language is empty. Routes through the automaton witness.
    pub fn witness(&self) -> RewriteResult<Option<Term>> {
        self.to_automaton()?.witness()
    }

    /// Whether the generated language is empty.
    pub fn language_is_empty(&self) -> RewriteResult<bool> {
        Ok(self.to_automaton()?.language_is_empty())
    }

    /// The canonical text rendering in the fixed syntax. Parsing
    /// this output reproduces the grammar exactly.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(GRAMMAR_SYNTAX_HEADER);
        out.push('\n');
        out.push_str("nonterminals:");
        for nonterminal in &self.nonterminals {
            out.push(' ');
            out.push_str(nonterminal.name().as_str());
        }
        out.push('\n');
        for start in &self.starts {
            out.push_str("start: ");
            out.push_str(start.name().as_str());
            out.push('\n');
        }
        for production in &self.productions {
            out.push_str(production.lhs().name().as_str());
            out.push_str(" -> ");
            out.push_str(production.symbol().symbol().as_str());
            for child in production.children() {
                out.push(' ');
                out.push_str(child.name().as_str());
            }
            out.push('\n');
        }
        out
    }

    /// Parse the fixed text syntax. Every malformed line is a typed
    /// error carrying its 1-based line number; ceilings are enforced
    /// against `limits`.
    pub fn parse(text: &str, limits: &TreeAutomatonLimits) -> RewriteResult<Self> {
        let malformed = |line: usize, detail: String| RewriteError::MalformedGrammar {
            message: format!("line {line}: {detail}"),
        };
        let mut lines = text.lines().enumerate();
        let Some((_, header)) = lines.next() else {
            return Err(RewriteError::MalformedGrammar {
                message: "line 1: empty grammar text (missing syntax header)".into(),
            });
        };
        if header.trim() != GRAMMAR_SYNTAX_HEADER {
            return Err(RewriteError::MalformedGrammar {
                message: format!(
                    "line 1: expected syntax header {GRAMMAR_SYNTAX_HEADER:?}, found {header:?}"
                ),
            });
        }
        // Ceilings are enforced INCREMENTALLY as lines are read —
        // memory and work stay bounded by the caller's limits rather
        // than by the input size — and duplicates are rejected as
        // they are read.
        let mut declared: BTreeSet<Nonterminal> = BTreeSet::new();
        let mut start_set: BTreeSet<Nonterminal> = BTreeSet::new();
        let mut seen_productions: BTreeSet<GrammarProduction> = BTreeSet::new();
        for (index, raw) in lines {
            let line_number = index + 1;
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("nonterminals:") {
                for token in rest.split_whitespace() {
                    let nonterminal = Nonterminal::new(token);
                    if declared.contains(&nonterminal) {
                        return Err(malformed(
                            line_number,
                            format!("duplicate nonterminal declaration {token:?}"),
                        ));
                    }
                    if declared.len() >= limits.max_states() {
                        return Err(RewriteError::InvalidLimit {
                            resource: "tree grammar nonterminals",
                            value: declared.len() + 1,
                            ceiling: limits.max_states(),
                        });
                    }
                    declared.insert(nonterminal);
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("start:") {
                let mut found = false;
                for token in rest.split_whitespace() {
                    let start = Nonterminal::new(token);
                    if !start_set.insert(start) {
                        return Err(malformed(
                            line_number,
                            format!("duplicate start nonterminal {token:?}"),
                        ));
                    }
                    found = true;
                }
                if !found {
                    return Err(malformed(line_number, "start: names no nonterminal".into()));
                }
                continue;
            }
            // Production: <lhs> -> <symbol> <children...>.
            // Tokenization is bounded by the rank ceiling: children
            // are collected lazily and the line is rejected as soon
            // as it exceeds the ceiling.
            let mut tokens = line.split_whitespace();
            let lhs_token = tokens.next();
            let arrow = tokens.next();
            let symbol_token = tokens.next();
            let (Some(lhs_token), Some("->"), Some(symbol_token)) =
                (lhs_token, arrow, symbol_token)
            else {
                return Err(malformed(
                    line_number,
                    format!(
                        "expected a production `<lhs> -> <symbol> <children...>`, found {line:?}"
                    ),
                ));
            };
            let mut children: Vec<Nonterminal> = Vec::new();
            for token in tokens {
                if children.len() >= limits.max_rank() {
                    return Err(RewriteError::InvalidLimit {
                        resource: "tree grammar production rank",
                        value: children.len() + 1,
                        ceiling: limits.max_rank(),
                    });
                }
                children.push(Nonterminal::new(token));
            }
            let arity = u16::try_from(children.len())
                .map_err(|_| malformed(line_number, "rank exceeds u16".into()))?;
            let symbol = RankedSymbol::new(Symbol::new(symbol_token), arity);
            let production = GrammarProduction::new(Nonterminal::new(lhs_token), symbol, children)
                .map_err(|error| match error {
                    RewriteError::MalformedGrammar { message } => malformed(line_number, message),
                    other => other,
                })?;
            if seen_productions.contains(&production) {
                return Err(malformed(line_number, "duplicate production".into()));
            }
            if seen_productions.len() >= limits.max_transitions() {
                return Err(RewriteError::InvalidLimit {
                    resource: "tree grammar productions",
                    value: seen_productions.len() + 1,
                    ceiling: limits.max_transitions(),
                });
            }
            seen_productions.insert(production);
        }
        let nonterminals: Vec<Nonterminal> = declared.into_iter().collect();
        let starts: Vec<Nonterminal> = start_set.into_iter().collect();
        let productions: Vec<GrammarProduction> = seen_productions.into_iter().collect();
        Self::new(nonterminals, productions, starts, limits).map_err(|error| match error {
            // Keep limit errors exact; re-attach a line hint to
            // validation failures (construction has already checked
            // them with precise messages).
            RewriteError::MalformedGrammar { message } => RewriteError::MalformedGrammar {
                message: format!("line 1+: {message}"),
            },
            other => other,
        })
    }
}

impl core::fmt::Display for Nonterminal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}
