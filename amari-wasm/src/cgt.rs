use amari_cgt::{GameArena, GameComparison, GameId, GameInspection, OutcomeClass};
use std::sync::atomic::{AtomicU32, Ordering};
use wasm_bindgen::prelude::*;

static NEXT_ARENA_ID: AtomicU32 = AtomicU32::new(1);

fn next_arena_id() -> u32 {
    NEXT_ARENA_ID.fetch_add(1, Ordering::Relaxed)
}

fn cgt_error(error: amari_cgt::CgtError) -> JsValue {
    JsValue::from_str(&error.to_string())
}

fn outcome_name(outcome: OutcomeClass) -> &'static str {
    match outcome {
        OutcomeClass::LeftWins => "left-wins",
        OutcomeClass::RightWins => "right-wins",
        OutcomeClass::NextPlayerWins => "next-player-wins",
        OutcomeClass::PreviousPlayerWins => "previous-player-wins",
    }
}

fn comparison_name(comparison: GameComparison) -> &'static str {
    match comparison {
        GameComparison::Less => "less",
        GameComparison::Equal => "equal",
        GameComparison::Greater => "greater",
        GameComparison::Fuzzy => "fuzzy",
    }
}

/// Opaque handle for a short combinatorial game stored in a `WasmCgtArena`.
#[wasm_bindgen]
#[derive(Clone)]
pub struct WasmGameId {
    pub(crate) inner: GameId,
    pub(crate) arena_id: u32,
}

#[wasm_bindgen]
impl WasmGameId {
    /// Returns whether two game handles point at the same arena-local game.
    pub fn same(&self, other: &WasmGameId) -> bool {
        self.arena_id == other.arena_id && self.inner == other.inner
    }
}

/// Inspection summary for a short combinatorial game.
#[wasm_bindgen]
pub struct WasmGameInspection {
    inner: GameInspection,
    arena_id: u32,
}

#[wasm_bindgen]
impl WasmGameInspection {
    /// Birthday of the inspected game.
    pub fn birthday(&self) -> u32 {
        self.inner.birthday().0
    }

    /// Canonical representative handle.
    pub fn canonical(&self) -> WasmGameId {
        WasmGameId {
            inner: self.inner.canonical_game_id(),
            arena_id: self.arena_id,
        }
    }

    /// Canonical structural form as recursive cut notation.
    #[wasm_bindgen(js_name = canonicalForm)]
    pub fn canonical_form(&self) -> String {
        self.inner.canonical_form().to_string()
    }

    /// Normal-play outcome class.
    pub fn outcome(&self) -> String {
        outcome_name(self.inner.outcome()).to_string()
    }

    /// Whether the game is impartial.
    #[wasm_bindgen(js_name = isImpartial)]
    pub fn is_impartial(&self) -> bool {
        self.inner.is_impartial()
    }

    /// Whether the game is partizan.
    #[wasm_bindgen(js_name = isPartizan)]
    pub fn is_partizan(&self) -> bool {
        self.inner.is_partizan()
    }

    /// Whether the game is numeric.
    #[wasm_bindgen(js_name = isNumeric)]
    pub fn is_numeric(&self) -> bool {
        self.inner.is_numeric()
    }

    /// Whether the game is non-numeric.
    #[wasm_bindgen(js_name = isNonNumeric)]
    pub fn is_non_numeric(&self) -> bool {
        self.inner.is_non_numeric()
    }

    /// Whether the game already is its canonical representative.
    #[wasm_bindgen(js_name = isCanonical)]
    pub fn is_canonical(&self) -> bool {
        self.inner.is_canonical()
    }

    /// Number of reachable arena nodes in the game DAG.
    #[wasm_bindgen(js_name = reachableNodeCount)]
    pub fn reachable_node_count(&self) -> usize {
        self.inner.reachable_node_count()
    }
}

/// Arena-backed short combinatorial game engine for WebAssembly.
#[wasm_bindgen]
pub struct WasmCgtArena {
    pub(crate) inner: GameArena,
    pub(crate) arena_id: u32,
}

#[wasm_bindgen]
impl WasmCgtArena {
    /// Create an empty arena with the zero game interned lazily.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: GameArena::new(),
            arena_id: next_arena_id(),
        }
    }

    /// Number of interned arena nodes.
    #[wasm_bindgen(js_name = nodeCount)]
    pub fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    /// Return the zero game `{ | }`.
    pub fn zero(&mut self) -> WasmGameId {
        let game = self.inner.zero();
        self.wrap(game)
    }

    /// Return the star game `{0 | 0}`.
    pub fn star(&mut self) -> Result<WasmGameId, JsValue> {
        let game = self.inner.star().map_err(cgt_error)?;
        Ok(self.wrap(game))
    }

    /// Return the game `1 = {0 | }`.
    pub fn one(&mut self) -> Result<WasmGameId, JsValue> {
        let game = self.inner.one().map_err(cgt_error)?;
        Ok(self.wrap(game))
    }

    /// Return the game `-1 = {| 0}`.
    #[wasm_bindgen(js_name = minusOne)]
    pub fn minus_one(&mut self) -> Result<WasmGameId, JsValue> {
        let game = self.inner.minus_one().map_err(cgt_error)?;
        Ok(self.wrap(game))
    }

    /// Build a one-left, one-right cut `{left | right}`.
    pub fn cut(&mut self, left: &WasmGameId, right: &WasmGameId) -> Result<WasmGameId, JsValue> {
        let left = self.game_id(left)?;
        let right = self.game_id(right)?;
        let game = self
            .inner
            .from_options([left], [right])
            .map_err(cgt_error)?;
        Ok(self.wrap(game))
    }

    /// Build a one-left-option game `{left | }`.
    #[wasm_bindgen(js_name = leftCut)]
    pub fn left_cut(&mut self, left: &WasmGameId) -> Result<WasmGameId, JsValue> {
        let left = self.game_id(left)?;
        let game = self
            .inner
            .from_options([left], std::iter::empty())
            .map_err(cgt_error)?;
        Ok(self.wrap(game))
    }

    /// Build a one-right-option game `{ | right}`.
    #[wasm_bindgen(js_name = rightCut)]
    pub fn right_cut(&mut self, right: &WasmGameId) -> Result<WasmGameId, JsValue> {
        let right = self.game_id(right)?;
        let game = self
            .inner
            .from_options(std::iter::empty(), [right])
            .map_err(cgt_error)?;
        Ok(self.wrap(game))
    }

    /// Add two short games.
    pub fn add(&mut self, lhs: &WasmGameId, rhs: &WasmGameId) -> Result<WasmGameId, JsValue> {
        let lhs = self.game_id(lhs)?;
        let rhs = self.game_id(rhs)?;
        let game = self.inner.add(lhs, rhs).map_err(cgt_error)?;
        Ok(self.wrap(game))
    }

    /// Subtract two short games.
    pub fn sub(&mut self, lhs: &WasmGameId, rhs: &WasmGameId) -> Result<WasmGameId, JsValue> {
        let lhs = self.game_id(lhs)?;
        let rhs = self.game_id(rhs)?;
        let game = self.inner.sub(lhs, rhs).map_err(cgt_error)?;
        Ok(self.wrap(game))
    }

    /// Negate a short game.
    pub fn neg(&mut self, game: &WasmGameId) -> Result<WasmGameId, JsValue> {
        let game = self.game_id(game)?;
        let negated = self.inner.neg(game).map_err(cgt_error)?;
        Ok(self.wrap(negated))
    }

    /// Compare two games in the partizan order.
    pub fn compare(&mut self, lhs: &WasmGameId, rhs: &WasmGameId) -> Result<String, JsValue> {
        let lhs = self.game_id(lhs)?;
        let rhs = self.game_id(rhs)?;
        Ok(comparison_name(self.inner.compare(lhs, rhs).map_err(cgt_error)?).to_string())
    }

    /// Return whether two games are equivalent.
    pub fn equivalent(&mut self, lhs: &WasmGameId, rhs: &WasmGameId) -> Result<bool, JsValue> {
        let lhs = self.game_id(lhs)?;
        let rhs = self.game_id(rhs)?;
        self.inner.equivalent(lhs, rhs).map_err(cgt_error)
    }

    /// Return the normal-play outcome class.
    pub fn outcome(&mut self, game: &WasmGameId) -> Result<String, JsValue> {
        let game = self.game_id(game)?;
        Ok(outcome_name(self.inner.outcome(game).map_err(cgt_error)?).to_string())
    }

    /// Return the game's birthday.
    pub fn birthday(&self, game: &WasmGameId) -> Result<u32, JsValue> {
        let game = self.game_id(game)?;
        Ok(self.inner.birthday(game).map_err(cgt_error)?.0)
    }

    /// Return whether a game is numeric.
    #[wasm_bindgen(js_name = isNumeric)]
    pub fn is_numeric(&mut self, game: &WasmGameId) -> Result<bool, JsValue> {
        let game = self.game_id(game)?;
        self.inner.is_numeric(game).map_err(cgt_error)
    }

    /// Return whether a game is impartial.
    #[wasm_bindgen(js_name = isImpartial)]
    pub fn is_impartial(&mut self, game: &WasmGameId) -> Result<bool, JsValue> {
        let game = self.game_id(game)?;
        self.inner.is_impartial(game).map_err(cgt_error)
    }

    /// Canonicalize a game and return the canonical representative handle.
    pub fn canonical(&mut self, game: &WasmGameId) -> Result<WasmGameId, JsValue> {
        let game = self.game_id(game)?;
        let canonical = self.inner.canonicalize(game).map_err(cgt_error)?;
        Ok(self.wrap(canonical.0))
    }

    /// Format a game as recursive cut notation.
    #[wasm_bindgen(js_name = formatGame)]
    pub fn format_game(&self, game: &WasmGameId) -> Result<String, JsValue> {
        let game = self.game_id(game)?;
        self.inner.format_game(game).map_err(cgt_error)
    }

    /// Format a game's canonical representative.
    #[wasm_bindgen(js_name = formatCanonicalGame)]
    pub fn format_canonical_game(&mut self, game: &WasmGameId) -> Result<String, JsValue> {
        let game = self.game_id(game)?;
        self.inner.format_canonical_game(game).map_err(cgt_error)
    }

    /// Compute the Grundy nimber of an impartial game.
    pub fn grundy(&mut self, game: &WasmGameId) -> Result<u32, JsValue> {
        let game = self.game_id(game)?;
        Ok(self.inner.grundy(game).map_err(cgt_error)?.0)
    }

    /// Build a Nim heap of the requested size.
    #[wasm_bindgen(js_name = nimHeap)]
    pub fn nim_heap(&mut self, size: u32) -> Result<WasmGameId, JsValue> {
        let game = self.inner.nim_heap(size).map_err(cgt_error)?;
        Ok(self.wrap(game))
    }

    /// Inspect canonical, numeric, impartial, outcome, and size metadata.
    pub fn inspect(&mut self, game: &WasmGameId) -> Result<WasmGameInspection, JsValue> {
        let game = self.game_id(game)?;
        Ok(WasmGameInspection {
            inner: self.inner.inspect(game).map_err(cgt_error)?,
            arena_id: self.arena_id,
        })
    }
}

impl Default for WasmCgtArena {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmCgtArena {
    pub(crate) fn game_id(&self, game: &WasmGameId) -> Result<GameId, JsValue> {
        if game.arena_id != self.arena_id {
            Err(JsValue::from_str(
                "game handle belongs to a different WasmCgtArena",
            ))
        } else {
            Ok(game.inner)
        }
    }

    pub(crate) fn wrap(&self, game: GameId) -> WasmGameId {
        WasmGameId {
            inner: game,
            arena_id: self.arena_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_cgt_arena_builds_and_inspects_small_games() {
        let mut arena = WasmCgtArena::new();
        let zero = arena.zero();
        let one = arena.one().unwrap();
        let half = arena.cut(&zero, &one).unwrap();
        let star = arena.star().unwrap();

        assert_eq!(arena.node_count(), 4);
        assert_eq!(arena.format_game(&zero).unwrap(), "0");
        assert_eq!(arena.format_game(&one).unwrap(), "1");
        assert_eq!(arena.format_game(&half).unwrap(), "{0 | 1}");
        assert_eq!(arena.outcome(&star).unwrap(), "next-player-wins");
        assert_eq!(arena.compare(&zero, &one).unwrap(), "less");
        assert!(arena.is_numeric(&half).unwrap());
        assert!(!arena.is_numeric(&star).unwrap());
    }

    #[test]
    fn wasm_cgt_exposes_nimbers_and_canonical_forms() {
        let mut arena = WasmCgtArena::new();
        let heap = arena.nim_heap(3).unwrap();
        let canonical = arena.canonical(&heap).unwrap();
        let inspection = arena.inspect(&heap).unwrap();

        assert_eq!(arena.grundy(&heap).unwrap(), 3);
        assert!(arena.equivalent(&heap, &canonical).unwrap());
        assert_eq!(inspection.outcome(), "next-player-wins");
        assert!(inspection.is_impartial());
        assert!(!inspection.is_numeric());
        assert!(inspection.reachable_node_count() >= 4);
    }
}
