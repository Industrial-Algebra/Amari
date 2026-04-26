use crate::error::{CgtError, Result};
use crate::form::GameForm;
use crate::game::{Birthday, CanonicalGame, GameComparison, GameId, OutcomeClass};
use crate::nimber::Nimber;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct NodeKey {
    left: Vec<GameId>,
    right: Vec<GameId>,
}

#[derive(Debug, Clone)]
struct GameNode {
    left: Vec<GameId>,
    right: Vec<GameId>,
    birthday: Birthday,
}

#[derive(Debug, Default, Clone)]
struct GameCaches {
    leq: HashMap<(GameId, GameId), bool>,
    negations: HashMap<GameId, GameId>,
    sums: HashMap<(GameId, GameId), GameId>,
    canonicals: HashMap<GameId, GameId>,
    impartial: HashMap<GameId, bool>,
    grundy: HashMap<GameId, Nimber>,
    numeric: HashMap<GameId, bool>,
    nim_heaps: HashMap<u32, GameId>,
}

/// Arena-backed storage for short combinatorial games.
#[derive(Debug, Default, Clone)]
pub struct GameArena {
    nodes: Vec<GameNode>,
    intern: HashMap<NodeKey, GameId>,
    caches: GameCaches,
    zero_id: Option<GameId>,
}

impl GameArena {
    /// Creates an empty game arena.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of distinct interned nodes in the arena.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the zero game `{ | }`.
    #[must_use]
    pub fn zero(&mut self) -> GameId {
        if let Some(id) = self.zero_id {
            return id;
        }

        let id = self
            .from_options::<Vec<GameId>, Vec<GameId>>(Vec::new(), Vec::new())
            .expect("constructing zero from empty options must succeed");
        self.zero_id = Some(id);
        id
    }

    /// Returns the star game `{ 0 | 0 }`.
    pub fn star(&mut self) -> Result<GameId> {
        let zero = self.zero();
        self.from_options([zero], [zero])
    }

    /// Returns the game `1 = { 0 | }`.
    pub fn one(&mut self) -> Result<GameId> {
        let zero = self.zero();
        self.from_options([zero], [])
    }

    /// Returns the game `-1 = { | 0 }`.
    pub fn minus_one(&mut self) -> Result<GameId> {
        let zero = self.zero();
        self.from_options([], [zero])
    }

    /// Creates or interns a short game from left and right option sets.
    pub fn from_options<L, R>(&mut self, left: L, right: R) -> Result<GameId>
    where
        L: IntoIterator<Item = GameId>,
        R: IntoIterator<Item = GameId>,
    {
        let mut left: Vec<GameId> = left.into_iter().collect();
        let mut right: Vec<GameId> = right.into_iter().collect();

        for id in left.iter().chain(right.iter()) {
            self.node(*id)?;
        }

        left.sort_unstable();
        left.dedup();
        right.sort_unstable();
        right.dedup();

        let key = NodeKey {
            left: left.clone(),
            right: right.clone(),
        };

        if let Some(id) = self.intern.get(&key) {
            return Ok(*id);
        }

        let max_birthday = left
            .iter()
            .chain(right.iter())
            .map(|id| self.nodes[id.0 as usize].birthday.0)
            .max()
            .unwrap_or(0);
        let birthday = if left.is_empty() && right.is_empty() {
            Birthday(0)
        } else {
            Birthday(max_birthday + 1)
        };

        let id = GameId(self.nodes.len() as u32);
        self.nodes.push(GameNode {
            left: key.left.clone(),
            right: key.right.clone(),
            birthday,
        });
        self.intern.insert(key, id);
        Ok(id)
    }

    /// Imports an arena-independent structural game form into the arena.
    pub fn intern_form(&mut self, form: &GameForm) -> Result<GameId> {
        let mut cache = HashMap::new();
        self.intern_form_cached(form, &mut cache)
    }

    /// Exports an arena-backed game into an arena-independent structural form.
    pub fn to_form(&self, game: GameId) -> Result<GameForm> {
        let mut cache = HashMap::new();
        self.to_form_cached(game, &mut cache)
    }

    /// Exports the canonical representative of a game as a structural form.
    pub fn canonical_form(&mut self, game: GameId) -> Result<GameForm> {
        let canonical = self.canonicalize(game)?.0;
        self.to_form(canonical)
    }

    /// Returns the birthday of a game.
    pub fn birthday(&self, game: GameId) -> Result<Birthday> {
        Ok(self.node(game)?.birthday)
    }

    /// Returns the left options of a game.
    pub fn left_options(&self, game: GameId) -> Result<&[GameId]> {
        Ok(&self.node(game)?.left)
    }

    /// Returns the right options of a game.
    pub fn right_options(&self, game: GameId) -> Result<&[GameId]> {
        Ok(&self.node(game)?.right)
    }

    /// Compares two short games in the partizan order.
    pub fn compare(&mut self, lhs: GameId, rhs: GameId) -> Result<GameComparison> {
        let lhs_le_rhs = self.leq(lhs, rhs)?;
        let rhs_le_lhs = self.leq(rhs, lhs)?;

        Ok(match (lhs_le_rhs, rhs_le_lhs) {
            (true, true) => GameComparison::Equal,
            (true, false) => GameComparison::Less,
            (false, true) => GameComparison::Greater,
            (false, false) => GameComparison::Fuzzy,
        })
    }

    /// Returns whether two games are equal in the partizan order.
    pub fn equivalent(&mut self, lhs: GameId, rhs: GameId) -> Result<bool> {
        Ok(self.compare(lhs, rhs)? == GameComparison::Equal)
    }

    /// Returns the outcome class of a game.
    pub fn outcome(&mut self, game: GameId) -> Result<OutcomeClass> {
        let zero = self.zero();
        Ok(match self.compare(game, zero)? {
            GameComparison::Greater => OutcomeClass::LeftWins,
            GameComparison::Less => OutcomeClass::RightWins,
            GameComparison::Equal => OutcomeClass::PreviousPlayerWins,
            GameComparison::Fuzzy => OutcomeClass::NextPlayerWins,
        })
    }

    /// Computes the negation of a short game.
    pub fn neg(&mut self, game: GameId) -> Result<GameId> {
        self.node(game)?;
        if let Some(id) = self.caches.negations.get(&game) {
            return Ok(*id);
        }

        let left = self.right_options(game)?.to_vec();
        let right = self.left_options(game)?.to_vec();

        let mut new_left = Vec::with_capacity(left.len());
        for option in left {
            new_left.push(self.neg(option)?);
        }

        let mut new_right = Vec::with_capacity(right.len());
        for option in right {
            new_right.push(self.neg(option)?);
        }

        let negated = self.from_options(new_left, new_right)?;
        self.caches.negations.insert(game, negated);
        Ok(negated)
    }

    /// Computes the disjunctive sum of two short games.
    pub fn add(&mut self, lhs: GameId, rhs: GameId) -> Result<GameId> {
        self.node(lhs)?;
        self.node(rhs)?;

        if let Some(id) = self.caches.sums.get(&(lhs, rhs)) {
            return Ok(*id);
        }
        if let Some(id) = self.caches.sums.get(&(rhs, lhs)) {
            return Ok(*id);
        }

        let lhs_left = self.left_options(lhs)?.to_vec();
        let lhs_right = self.right_options(lhs)?.to_vec();
        let rhs_left = self.left_options(rhs)?.to_vec();
        let rhs_right = self.right_options(rhs)?.to_vec();

        let mut left = Vec::with_capacity(lhs_left.len() + rhs_left.len());
        for option in lhs_left {
            left.push(self.add(option, rhs)?);
        }
        for option in rhs_left {
            left.push(self.add(lhs, option)?);
        }

        let mut right = Vec::with_capacity(lhs_right.len() + rhs_right.len());
        for option in lhs_right {
            right.push(self.add(option, rhs)?);
        }
        for option in rhs_right {
            right.push(self.add(lhs, option)?);
        }

        let sum = self.from_options(left, right)?;
        self.caches.sums.insert((lhs, rhs), sum);
        self.caches.sums.insert((rhs, lhs), sum);
        Ok(sum)
    }

    /// Computes `lhs - rhs`.
    pub fn sub(&mut self, lhs: GameId, rhs: GameId) -> Result<GameId> {
        let neg_rhs = self.neg(rhs)?;
        self.add(lhs, neg_rhs)
    }

    /// Returns a canonical wrapper for a known game id.
    pub fn canonical(&self, game: GameId) -> Result<CanonicalGame> {
        self.node(game)?;
        Ok(CanonicalGame(game))
    }

    /// Canonicalizes a short game by recursively reducing its option sets.
    pub fn canonicalize(&mut self, game: GameId) -> Result<CanonicalGame> {
        self.node(game)?;
        if let Some(id) = self.caches.canonicals.get(&game) {
            return Ok(CanonicalGame(*id));
        }

        let left_ids = self.left_options(game)?.to_vec();
        let right_ids = self.right_options(game)?.to_vec();

        let mut left = Vec::with_capacity(left_ids.len());
        for option in left_ids {
            left.push(self.canonicalize(option)?.0);
        }

        let mut right = Vec::with_capacity(right_ids.len());
        for option in right_ids {
            right.push(self.canonicalize(option)?.0);
        }

        let canonical_id = self.canonicalize_from_parts(left, right)?;
        self.caches.canonicals.insert(game, canonical_id);
        self.caches.canonicals.insert(canonical_id, canonical_id);
        Ok(CanonicalGame(canonical_id))
    }

    /// Returns whether a game is impartial.
    pub fn is_impartial(&mut self, game: GameId) -> Result<bool> {
        self.node(game)?;
        if let Some(value) = self.caches.impartial.get(&game) {
            return Ok(*value);
        }

        let left = self.left_options(game)?.to_vec();
        let right = self.right_options(game)?.to_vec();
        if left != right {
            self.caches.impartial.insert(game, false);
            return Ok(false);
        }

        for option in left {
            if !self.is_impartial(option)? {
                self.caches.impartial.insert(game, false);
                return Ok(false);
            }
        }

        self.caches.impartial.insert(game, true);
        Ok(true)
    }

    /// Computes the Sprague-Grundy value of an impartial game.
    pub fn grundy(&mut self, game: GameId) -> Result<Nimber> {
        self.node(game)?;
        if let Some(value) = self.caches.grundy.get(&game) {
            return Ok(*value);
        }

        if !self.is_impartial(game)? {
            return Err(CgtError::NotImpartial(game));
        }

        let options = self.left_options(game)?.to_vec();
        let mut seen = HashSet::with_capacity(options.len());
        for option in options {
            seen.insert(self.grundy(option)?.0);
        }

        let value = self.mex(&seen);
        self.caches.grundy.insert(game, value);
        Ok(value)
    }

    /// Builds the impartial Nim heap of a given size.
    pub fn nim_heap(&mut self, size: u32) -> Result<GameId> {
        if let Some(id) = self.caches.nim_heaps.get(&size) {
            return Ok(*id);
        }

        let id = if size == 0 {
            self.zero()
        } else {
            let mut options = Vec::with_capacity(size as usize);
            for smaller in 0..size {
                options.push(self.nim_heap(smaller)?);
            }
            self.from_options(options.clone(), options)?
        };

        self.caches.nim_heaps.insert(size, id);
        Ok(id)
    }

    /// Returns whether a short game is numeric.
    pub fn is_numeric(&mut self, game: GameId) -> Result<bool> {
        self.node(game)?;
        if let Some(value) = self.caches.numeric.get(&game) {
            return Ok(*value);
        }

        let left = self.left_options(game)?.to_vec();
        let right = self.right_options(game)?.to_vec();

        for option in left.iter().chain(right.iter()) {
            if !self.is_numeric(*option)? {
                self.caches.numeric.insert(game, false);
                return Ok(false);
            }
        }

        for &lhs in &left {
            for &rhs in &right {
                if self.compare(lhs, rhs)? != GameComparison::Less {
                    self.caches.numeric.insert(game, false);
                    return Ok(false);
                }
            }
        }

        self.caches.numeric.insert(game, true);
        Ok(true)
    }

    fn intern_form_cached(
        &mut self,
        form: &GameForm,
        cache: &mut HashMap<GameForm, GameId>,
    ) -> Result<GameId> {
        let key = form.clone().normalized();
        if let Some(game) = cache.get(&key) {
            return Ok(*game);
        }

        let left_forms = key.left_options().to_vec();
        let right_forms = key.right_options().to_vec();

        let mut left = Vec::with_capacity(left_forms.len());
        for option in left_forms {
            left.push(self.intern_form_cached(&option, cache)?);
        }

        let mut right = Vec::with_capacity(right_forms.len());
        for option in right_forms {
            right.push(self.intern_form_cached(&option, cache)?);
        }

        let game = self.from_options(left, right)?;
        cache.insert(key, game);
        Ok(game)
    }

    fn to_form_cached(
        &self,
        game: GameId,
        cache: &mut HashMap<GameId, GameForm>,
    ) -> Result<GameForm> {
        self.node(game)?;
        if let Some(form) = cache.get(&game) {
            return Ok(form.clone());
        }

        let left_ids = self.left_options(game)?.to_vec();
        let right_ids = self.right_options(game)?.to_vec();

        let mut left = Vec::with_capacity(left_ids.len());
        for option in left_ids {
            left.push(self.to_form_cached(option, cache)?);
        }

        let mut right = Vec::with_capacity(right_ids.len());
        for option in right_ids {
            right.push(self.to_form_cached(option, cache)?);
        }

        let form = GameForm::new(left, right);
        cache.insert(game, form.clone());
        Ok(form)
    }

    fn leq(&mut self, lhs: GameId, rhs: GameId) -> Result<bool> {
        self.node(lhs)?;
        self.node(rhs)?;
        if let Some(value) = self.caches.leq.get(&(lhs, rhs)) {
            return Ok(*value);
        }

        let left = self.left_options(lhs)?.to_vec();
        for option in left {
            if self.leq(rhs, option)? {
                self.caches.leq.insert((lhs, rhs), false);
                return Ok(false);
            }
        }

        let right = self.right_options(rhs)?.to_vec();
        for option in right {
            if self.leq(option, lhs)? {
                self.caches.leq.insert((lhs, rhs), false);
                return Ok(false);
            }
        }

        self.caches.leq.insert((lhs, rhs), true);
        Ok(true)
    }

    fn canonicalize_from_parts(&mut self, left: Vec<GameId>, right: Vec<GameId>) -> Result<GameId> {
        let mut left = self.normalized_options(left)?;
        let mut right = self.normalized_options(right)?;

        loop {
            let left_dominated = self.remove_dominated_left(&left)?;
            let right_dominated = self.remove_dominated_right(&right)?;
            let candidate = self.from_options(left_dominated.clone(), right_dominated.clone())?;

            let next_left = self.reduce_reversible_left(candidate, &left_dominated)?;
            let next_left = self.normalized_options(next_left)?;
            let next_right = self.reduce_reversible_right(candidate, &right_dominated)?;
            let next_right = self.normalized_options(next_right)?;

            if next_left == left && next_right == right {
                return Ok(candidate);
            }

            left = next_left;
            right = next_right;
        }
    }

    fn normalized_options(&self, mut options: Vec<GameId>) -> Result<Vec<GameId>> {
        for id in &options {
            self.node(*id)?;
        }

        options.sort_unstable();
        options.dedup();
        Ok(options)
    }

    fn remove_dominated_left(&mut self, options: &[GameId]) -> Result<Vec<GameId>> {
        let mut reduced = Vec::with_capacity(options.len());

        'candidate: for (index, &option) in options.iter().enumerate() {
            for (other_index, &other) in options.iter().enumerate() {
                if index == other_index {
                    continue;
                }

                if matches!(
                    self.compare(option, other)?,
                    GameComparison::Less | GameComparison::Equal
                ) {
                    continue 'candidate;
                }
            }

            reduced.push(option);
        }

        Ok(reduced)
    }

    fn remove_dominated_right(&mut self, options: &[GameId]) -> Result<Vec<GameId>> {
        let mut reduced = Vec::with_capacity(options.len());

        'candidate: for (index, &option) in options.iter().enumerate() {
            for (other_index, &other) in options.iter().enumerate() {
                if index == other_index {
                    continue;
                }

                if matches!(
                    self.compare(option, other)?,
                    GameComparison::Greater | GameComparison::Equal
                ) {
                    continue 'candidate;
                }
            }

            reduced.push(option);
        }

        Ok(reduced)
    }

    fn reduce_reversible_left(
        &mut self,
        parent: GameId,
        options: &[GameId],
    ) -> Result<Vec<GameId>> {
        let mut reduced = Vec::with_capacity(options.len());

        for &option in options {
            if let Some(reply) = self.reversible_left_reply(parent, option)? {
                let replacements = self.left_options(reply)?.to_vec();
                reduced.extend(replacements);
            } else {
                reduced.push(option);
            }
        }

        Ok(reduced)
    }

    fn reduce_reversible_right(
        &mut self,
        parent: GameId,
        options: &[GameId],
    ) -> Result<Vec<GameId>> {
        let mut reduced = Vec::with_capacity(options.len());

        for &option in options {
            if let Some(reply) = self.reversible_right_reply(parent, option)? {
                let replacements = self.right_options(reply)?.to_vec();
                reduced.extend(replacements);
            } else {
                reduced.push(option);
            }
        }

        Ok(reduced)
    }

    fn reversible_left_reply(&mut self, parent: GameId, option: GameId) -> Result<Option<GameId>> {
        let replies = self.right_options(option)?.to_vec();
        for reply in replies {
            if matches!(
                self.compare(reply, parent)?,
                GameComparison::Less | GameComparison::Equal
            ) {
                return Ok(Some(reply));
            }
        }

        Ok(None)
    }

    fn reversible_right_reply(&mut self, parent: GameId, option: GameId) -> Result<Option<GameId>> {
        let replies = self.left_options(option)?.to_vec();
        for reply in replies {
            if matches!(
                self.compare(reply, parent)?,
                GameComparison::Greater | GameComparison::Equal
            ) {
                return Ok(Some(reply));
            }
        }

        Ok(None)
    }

    fn mex(&self, seen: &HashSet<u32>) -> Nimber {
        let mut value = 0;
        while seen.contains(&value) {
            value += 1;
        }

        Nimber(value)
    }

    fn node(&self, game: GameId) -> Result<&GameNode> {
        self.nodes
            .get(game.0 as usize)
            .ok_or(CgtError::InvalidGameId(game))
    }
}
