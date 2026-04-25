use crate::arena::GameArena;
use crate::error::{CgtError, Result};
use crate::game::{CanonicalGame, GameId, OutcomeClass};
use std::collections::{BTreeMap, HashSet};

/// Maximum option-universe size used by the exhaustive bounded generators.
///
/// The birthday and node-count generators enumerate all pairs of option subsets
/// from the current candidate universe. That process grows as `4^n`, so these
/// utilities intentionally stop once the option universe becomes too large for a
/// small exhaustive search.
pub const MAX_EXHAUSTIVE_OPTION_UNIVERSE: usize = 12;

/// Outcome-class counts collected across a corpus.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutcomeCounts {
    left_wins: usize,
    right_wins: usize,
    next_player_wins: usize,
    previous_player_wins: usize,
}

impl OutcomeCounts {
    /// Returns the number of left-win games in the corpus.
    #[must_use]
    pub fn left_wins(&self) -> usize {
        self.left_wins
    }

    /// Returns the number of right-win games in the corpus.
    #[must_use]
    pub fn right_wins(&self) -> usize {
        self.right_wins
    }

    /// Returns the number of next-player-win games in the corpus.
    #[must_use]
    pub fn next_player_wins(&self) -> usize {
        self.next_player_wins
    }

    /// Returns the number of previous-player-win games in the corpus.
    #[must_use]
    pub fn previous_player_wins(&self) -> usize {
        self.previous_player_wins
    }

    /// Returns the total number of classified games.
    #[must_use]
    pub fn total(&self) -> usize {
        self.left_wins + self.right_wins + self.next_player_wins + self.previous_player_wins
    }

    fn record(&mut self, outcome: OutcomeClass) {
        match outcome {
            OutcomeClass::LeftWins => self.left_wins += 1,
            OutcomeClass::RightWins => self.right_wins += 1,
            OutcomeClass::NextPlayerWins => self.next_player_wins += 1,
            OutcomeClass::PreviousPlayerWins => self.previous_player_wins += 1,
        }
    }
}

/// Summary statistics for a canonical corpus.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CorpusStats {
    total_games: usize,
    birthday_histogram: BTreeMap<u32, usize>,
    reachable_node_histogram: BTreeMap<usize, usize>,
    outcome_counts: OutcomeCounts,
    impartial_games: usize,
    numeric_games: usize,
}

impl CorpusStats {
    /// Returns the number of canonical games represented in the corpus.
    #[must_use]
    pub fn total_games(&self) -> usize {
        self.total_games
    }

    /// Returns a histogram keyed by short-game birthday.
    #[must_use]
    pub fn birthday_histogram(&self) -> &BTreeMap<u32, usize> {
        &self.birthday_histogram
    }

    /// Returns a histogram keyed by reachable-node count.
    #[must_use]
    pub fn reachable_node_histogram(&self) -> &BTreeMap<usize, usize> {
        &self.reachable_node_histogram
    }

    /// Returns the outcome-class counts for the corpus.
    #[must_use]
    pub fn outcome_counts(&self) -> &OutcomeCounts {
        &self.outcome_counts
    }

    /// Returns the number of impartial canonical games.
    #[must_use]
    pub fn impartial_games(&self) -> usize {
        self.impartial_games
    }

    /// Returns the number of partizan canonical games.
    #[must_use]
    pub fn partizan_games(&self) -> usize {
        self.total_games.saturating_sub(self.impartial_games)
    }

    /// Returns the number of numeric canonical games.
    #[must_use]
    pub fn numeric_games(&self) -> usize {
        self.numeric_games
    }

    /// Returns the number of non-numeric canonical games.
    #[must_use]
    pub fn non_numeric_games(&self) -> usize {
        self.total_games.saturating_sub(self.numeric_games)
    }
}

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

    /// Returns whether the corpus contains a canonical game.
    #[must_use]
    pub fn contains(&self, game: CanonicalGame) -> bool {
        self.games
            .binary_search_by_key(&game.0, |candidate| candidate.0)
            .is_ok()
    }

    /// Returns canonical games bucketed by birthday.
    pub fn birthday_buckets(&self, arena: &GameArena) -> Result<BTreeMap<u32, Vec<CanonicalGame>>> {
        let mut buckets = BTreeMap::new();
        for &game in &self.games {
            let birthday = arena.birthday(game.0)?.0;
            buckets.entry(birthday).or_insert_with(Vec::new).push(game);
        }
        Ok(buckets)
    }

    /// Returns canonical games bucketed by reachable-node count.
    pub fn reachable_node_buckets(
        &self,
        arena: &GameArena,
    ) -> Result<BTreeMap<usize, Vec<CanonicalGame>>> {
        let mut buckets = BTreeMap::new();
        for &game in &self.games {
            let reachable_nodes = arena.reachable_node_count(game.0)?;
            buckets
                .entry(reachable_nodes)
                .or_insert_with(Vec::new)
                .push(game);
        }
        Ok(buckets)
    }

    /// Computes summary statistics for the canonical corpus.
    pub fn stats(&self, arena: &mut GameArena) -> Result<CorpusStats> {
        let mut stats = CorpusStats::default();

        for &game in &self.games {
            let id = game.0;
            stats.total_games += 1;
            Self::increment(&mut stats.birthday_histogram, arena.birthday(id)?.0);
            Self::increment(
                &mut stats.reachable_node_histogram,
                arena.reachable_node_count(id)?,
            );
            stats.outcome_counts.record(arena.outcome(id)?);
            if arena.is_impartial(id)? {
                stats.impartial_games += 1;
            }
            if arena.is_numeric(id)? {
                stats.numeric_games += 1;
            }
        }

        Ok(stats)
    }

    /// Consumes the corpus and returns its underlying vector.
    #[must_use]
    pub fn into_inner(self) -> Vec<CanonicalGame> {
        self.games
    }

    fn increment<K>(histogram: &mut BTreeMap<K, usize>, key: K)
    where
        K: Ord,
    {
        *histogram.entry(key).or_insert(0) += 1;
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
