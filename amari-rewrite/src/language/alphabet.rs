// SPDX-License-Identifier: MIT OR Apache-2.0

//! Ranked alphabets: symbols with fixed arities.

use crate::trs::Symbol;

/// A symbol together with its fixed arity. Every occurrence of a
/// symbol in a ranked alphabet carries the same rank; conflicts are
/// rejected at automaton construction.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct RankedSymbol {
    symbol: Symbol,
    arity: u16,
}

impl RankedSymbol {
    /// Create a ranked symbol. Rank zero is a constant.
    pub fn new(symbol: Symbol, arity: u16) -> Self {
        Self { symbol, arity }
    }

    /// The symbol.
    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }

    /// The fixed arity (rank).
    pub fn arity(&self) -> u16 {
        self.arity
    }
}
