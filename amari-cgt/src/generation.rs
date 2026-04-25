use crate::arena::GameArena;
use crate::error::{CgtError, Result};
use crate::game::{CanonicalGame, GameId};
use std::collections::HashSet;

/// Maximum option-universe size used by the exhaustive bounded generators.
///
/// The birthday and node-count generators enumerate all pairs of option subsets
/// from the current candidate universe. That process grows as `4^n`, so these
/// utilities intentionally stop once the option universe becomes too large for a
/// small exhaustive search.
pub const MAX_EXHAUSTIVE_OPTION_UNIVERSE: usize = 12;

/// Small deduplicated collection of canonical games.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CanonicalCorpus {
    games: Vec<CanonicalGame>,
}

impl CanonicalCorpus {
    /// Creates a canonical corpus from a set of canonical games.
    #[must_use]
    pub fn new(mut games: Vec<CanonicalGame>) -> Self {
        games.sort_unstable_by_key(|game| game.0);
        games.dedup();
        Self { games }
    }

    /// Returns the canonical games in the corpus.
    #[must_use]
    pub fn as_slice(&self) -> &[CanonicalGame] {
        &self.games
    }

    /// Returns an iterator over the canonical games.
    pub fn iter(&self) -> impl Iterator<Item = &CanonicalGame> {
        self.games.iter()
    }

    /// Returns the number of canonical games in the corpus.
    #[must_use]
    pub fn len(&self) -> usize {
        self.games.len()
    }

    /// Returns whether the corpus is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.games.is_empty()
    }

    /// Consumes the corpus and returns its underlying vector.
    #[must_use]
    pub fn into_inner(self) -> Vec<CanonicalGame> {
        self.games
    }
}

impl GameArena {
    /// Counts the number of distinct reachable arena nodes in a game's subgraph.
    pub fn reachable_node_count(&self, game: GameId) -> Result<usize> {
        self.birthday(game)?;
        let mut visited = HashSet::new();
        self.visit_reachable(game, &mut visited)?;
        Ok(visited.len())
    }

    /// Exhaustively generates all short games whose birthday is at most `max_birthday`.
    ///
    /// This is intentionally limited to very small bounds. Once the candidate
    /// option universe becomes too large for exhaustive subset-pair generation,
    /// the method returns [`CgtError::GenerationUniverseTooLarge`].
    pub fn generate_by_birthday(&mut self, max_birthday: u32) -> Result<Vec<GameId>> {
        let zero = self.zero();
        if max_birthday == 0 {
            return Ok(vec![zero]);
        }

        let mut generated = vec![zero];
        for target_birthday in 1..=max_birthday {
            let next = self.generate_from_option_universe(&generated, |arena, game| {
                Ok(arena.birthday(game)?.0 == target_birthday)
            })?;
            generated.extend(next);
            generated.sort_unstable();
            generated.dedup();
        }

        Ok(generated)
    }

    /// Exhaustively generates all short games whose reachable subgraph contains
    /// at most `max_nodes` distinct arena nodes.
    ///
    /// This is intentionally limited to very small bounds. Once the candidate
    /// option universe becomes too large for exhaustive subset-pair generation,
    /// the method returns [`CgtError::GenerationUniverseTooLarge`].
    pub fn generate_by_node_count(&mut self, max_nodes: usize) -> Result<Vec<GameId>> {
        if max_nodes == 0 {
            return Ok(Vec::new());
        }

        let zero = self.zero();
        if max_nodes == 1 {
            return Ok(vec![zero]);
        }

        let mut generated = vec![zero];
        for target_nodes in 2..=max_nodes {
            let next = self.generate_from_option_universe(&generated, |arena, game| {
                Ok(arena.reachable_node_count(game)? == target_nodes)
            })?;
            generated.extend(next);
            generated.sort_unstable();
            generated.dedup();
        }

        Ok(generated)
    }

    /// Builds a deduplicated canonical corpus from the birthday-bounded generator.
    pub fn canonical_corpus_by_birthday(&mut self, max_birthday: u32) -> Result<CanonicalCorpus> {
        let games = self.generate_by_birthday(max_birthday)?;
        self.canonical_corpus_from_games(games)
    }

    /// Builds a deduplicated canonical corpus from the node-count-bounded generator.
    pub fn canonical_corpus_by_node_count(&mut self, max_nodes: usize) -> Result<CanonicalCorpus> {
        let games = self.generate_by_node_count(max_nodes)?;
        self.canonical_corpus_from_games(games)
    }

    fn canonical_corpus_from_games(&mut self, games: Vec<GameId>) -> Result<CanonicalCorpus> {
        let mut canonical_games = Vec::with_capacity(games.len());
        for game in games {
            canonical_games.push(self.canonicalize(game)?);
        }
        Ok(CanonicalCorpus::new(canonical_games))
    }

    fn generate_from_option_universe<F>(
        &mut self,
        option_universe: &[GameId],
        mut include: F,
    ) -> Result<Vec<GameId>>
    where
        F: FnMut(&mut GameArena, GameId) -> Result<bool>,
    {
        self.ensure_generation_universe(option_universe)?;
        let subsets = Self::all_option_subsets(option_universe)?;
        let mut generated = HashSet::new();

        for left in &subsets {
            for right in &subsets {
                let game = self.from_options(left.iter().copied(), right.iter().copied())?;
                if include(self, game)? {
                    generated.insert(game);
                }
            }
        }

        let mut generated: Vec<_> = generated.into_iter().collect();
        generated.sort_unstable();
        Ok(generated)
    }

    fn ensure_generation_universe(&self, option_universe: &[GameId]) -> Result<()> {
        if option_universe.len() > MAX_EXHAUSTIVE_OPTION_UNIVERSE {
            return Err(CgtError::GenerationUniverseTooLarge(option_universe.len()));
        }

        for &game in option_universe {
            self.birthday(game)?;
        }

        Ok(())
    }

    fn all_option_subsets(option_universe: &[GameId]) -> Result<Vec<Vec<GameId>>> {
        if option_universe.len() >= usize::BITS as usize {
            return Err(CgtError::GenerationUniverseTooLarge(option_universe.len()));
        }

        let subset_count = 1usize << option_universe.len();
        let mut subsets = Vec::with_capacity(subset_count);
        for mask in 0..subset_count {
            subsets.push(Self::subset_from_mask(option_universe, mask));
        }
        Ok(subsets)
    }

    fn subset_from_mask(option_universe: &[GameId], mask: usize) -> Vec<GameId> {
        option_universe
            .iter()
            .enumerate()
            .filter_map(|(index, &game)| ((mask & (1usize << index)) != 0).then_some(game))
            .collect()
    }

    fn visit_reachable(&self, game: GameId, visited: &mut HashSet<GameId>) -> Result<()> {
        if !visited.insert(game) {
            return Ok(());
        }

        for &option in self.left_options(game)? {
            self.visit_reachable(option, visited)?;
        }
        for &option in self.right_options(game)? {
            self.visit_reachable(option, visited)?;
        }

        Ok(())
    }
}
