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

/// Analysis for one exact generation layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerAnalysis {
    raw_games: Vec<GameId>,
    canonical_corpus: CanonicalCorpus,
    stats: CorpusStats,
}

impl LayerAnalysis {
    /// Returns the raw games generated in the exact layer.
    #[must_use]
    pub fn raw_games(&self) -> &[GameId] {
        &self.raw_games
    }

    /// Returns the number of raw generated games in the exact layer.
    #[must_use]
    pub fn raw_game_count(&self) -> usize {
        self.raw_games.len()
    }

    /// Returns the canonical corpus for the exact layer.
    #[must_use]
    pub fn canonical_corpus(&self) -> &CanonicalCorpus {
        &self.canonical_corpus
    }

    /// Returns the number of canonical games represented in the layer.
    #[must_use]
    pub fn canonical_game_count(&self) -> usize {
        self.canonical_corpus.len()
    }

    /// Returns how many raw games collapsed under canonicalization.
    #[must_use]
    pub fn canonical_reduction(&self) -> usize {
        self.raw_game_count()
            .saturating_sub(self.canonical_game_count())
    }

    /// Returns summary statistics for the canonical layer corpus.
    #[must_use]
    pub fn stats(&self) -> &CorpusStats {
        &self.stats
    }
}

/// Layered analysis report keyed by an exact generation index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LayerAnalysisReport<K> {
    layers: BTreeMap<K, LayerAnalysis>,
}

impl<K: Ord> LayerAnalysisReport<K> {
    /// Creates a layered analysis report from exact layer analyses.
    #[must_use]
    pub fn new(layers: BTreeMap<K, LayerAnalysis>) -> Self {
        Self { layers }
    }

    /// Returns the exact layer analyses as a map.
    #[must_use]
    pub fn as_map(&self) -> &BTreeMap<K, LayerAnalysis> {
        &self.layers
    }

    /// Returns the analysis for a specific layer key.
    #[must_use]
    pub fn get(&self, layer: &K) -> Option<&LayerAnalysis> {
        self.layers.get(layer)
    }

    /// Returns an iterator over all exact layer analyses.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &LayerAnalysis)> {
        self.layers.iter()
    }

    /// Returns the number of exact layers in the report.
    #[must_use]
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Returns whether the report is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Returns the total number of raw games across all layers.
    #[must_use]
    pub fn raw_total_games(&self) -> usize {
        self.layers
            .values()
            .map(LayerAnalysis::raw_game_count)
            .sum()
    }

    /// Returns the total number of canonical games across all layers.
    #[must_use]
    pub fn canonical_total_games(&self) -> usize {
        self.layers
            .values()
            .map(LayerAnalysis::canonical_game_count)
            .sum()
    }

    /// Returns the total canonical reduction across all layers.
    #[must_use]
    pub fn canonical_reduction_total(&self) -> usize {
        self.layers
            .values()
            .map(LayerAnalysis::canonical_reduction)
            .sum()
    }
}

impl<K: Ord + Clone> LayerAnalysisReport<K> {
    /// Returns raw-game counts keyed by exact layer.
    #[must_use]
    pub fn raw_counts_by_layer(&self) -> BTreeMap<K, usize> {
        self.map_layer_values(LayerAnalysis::raw_game_count)
    }

    /// Returns canonical-game counts keyed by exact layer.
    #[must_use]
    pub fn canonical_counts_by_layer(&self) -> BTreeMap<K, usize> {
        self.map_layer_values(LayerAnalysis::canonical_game_count)
    }

    /// Returns canonical reduction counts keyed by exact layer.
    #[must_use]
    pub fn canonical_reductions_by_layer(&self) -> BTreeMap<K, usize> {
        self.map_layer_values(LayerAnalysis::canonical_reduction)
    }

    /// Returns numeric canonical-game counts keyed by exact layer.
    #[must_use]
    pub fn numeric_counts_by_layer(&self) -> BTreeMap<K, usize> {
        self.map_layer_values(|analysis| analysis.stats().numeric_games())
    }

    /// Returns non-numeric canonical-game counts keyed by exact layer.
    #[must_use]
    pub fn non_numeric_counts_by_layer(&self) -> BTreeMap<K, usize> {
        self.map_layer_values(|analysis| analysis.stats().non_numeric_games())
    }

    /// Returns impartial canonical-game counts keyed by exact layer.
    #[must_use]
    pub fn impartial_counts_by_layer(&self) -> BTreeMap<K, usize> {
        self.map_layer_values(|analysis| analysis.stats().impartial_games())
    }

    /// Returns partizan canonical-game counts keyed by exact layer.
    #[must_use]
    pub fn partizan_counts_by_layer(&self) -> BTreeMap<K, usize> {
        self.map_layer_values(|analysis| analysis.stats().partizan_games())
    }

    /// Returns outcome-class counts keyed by exact layer.
    #[must_use]
    pub fn outcome_counts_by_layer(&self) -> BTreeMap<K, OutcomeCounts> {
        self.map_layer_values(|analysis| analysis.stats().outcome_counts().clone())
    }

    fn map_layer_values<V, F>(&self, mut value: F) -> BTreeMap<K, V>
    where
        F: FnMut(&LayerAnalysis) -> V,
    {
        self.layers
            .iter()
            .map(|(key, analysis)| (key.clone(), value(analysis)))
            .collect()
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
        let mut buckets: BTreeMap<u32, Vec<CanonicalGame>> = BTreeMap::new();
        for &game in &self.games {
            let birthday = arena.birthday(game.0)?.0;
            buckets.entry(birthday).or_default().push(game);
        }
        Ok(buckets)
    }

    /// Returns canonical games bucketed by reachable-node count.
    pub fn reachable_node_buckets(
        &self,
        arena: &GameArena,
    ) -> Result<BTreeMap<usize, Vec<CanonicalGame>>> {
        let mut buckets: BTreeMap<usize, Vec<CanonicalGame>> = BTreeMap::new();
        for &game in &self.games {
            let reachable_nodes = arena.reachable_node_count(game.0)?;
            buckets.entry(reachable_nodes).or_default().push(game);
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

    /// Analyzes the exact birthday layer `target_birthday`.
    pub fn analyze_birthday_layer(&mut self, target_birthday: u32) -> Result<LayerAnalysis> {
        let raw_games = self.generate_birthday_layer(target_birthday)?;
        self.analyze_layer(raw_games)
    }

    /// Analyzes all exact birthday layers up to `max_birthday`.
    pub fn analyze_birthday_layers(
        &mut self,
        max_birthday: u32,
    ) -> Result<LayerAnalysisReport<u32>> {
        let layers = self.generate_birthday_layers(max_birthday)?;
        self.analyze_layer_map(layers)
    }

    /// Analyzes the exact reachable-node layer `target_nodes`.
    pub fn analyze_node_count_layer(&mut self, target_nodes: usize) -> Result<LayerAnalysis> {
        let raw_games = self.generate_node_count_layer(target_nodes)?;
        self.analyze_layer(raw_games)
    }

    /// Analyzes all exact reachable-node layers up to `max_nodes`.
    pub fn analyze_node_count_layers(
        &mut self,
        max_nodes: usize,
    ) -> Result<LayerAnalysisReport<usize>> {
        let layers = self.generate_node_count_layers(max_nodes)?;
        self.analyze_layer_map(layers)
    }

    /// Exhaustively generates the exact birthday layer `target_birthday`.
    ///
    /// This returns only games whose birthday is exactly `target_birthday`.
    /// The implementation is intentionally limited to very small exhaustive
    /// searches and returns [`CgtError::GenerationUniverseTooLarge`] once the
    /// option universe becomes too large.
    pub fn generate_birthday_layer(&mut self, target_birthday: u32) -> Result<Vec<GameId>> {
        let mut layers = self.generate_birthday_layers(target_birthday)?;
        Ok(layers.remove(&target_birthday).unwrap_or_default())
    }

    /// Exhaustively generates birthday layers up to `max_birthday`.
    ///
    /// Each map entry contains only games whose birthday is exactly the key.
    pub fn generate_birthday_layers(
        &mut self,
        max_birthday: u32,
    ) -> Result<BTreeMap<u32, Vec<GameId>>> {
        let zero = self.zero();
        let mut layers = BTreeMap::new();
        layers.insert(0, vec![zero]);

        let mut cumulative = vec![zero];
        for target_birthday in 1..=max_birthday {
            let layer = self.generate_from_option_universe(&cumulative, |arena, game| {
                Ok(arena.birthday(game)?.0 == target_birthday)
            })?;
            cumulative.extend(layer.iter().copied());
            cumulative.sort_unstable();
            cumulative.dedup();
            layers.insert(target_birthday, layer);
        }

        Ok(layers)
    }

    /// Exhaustively generates all short games whose birthday is at most `max_birthday`.
    ///
    /// This is intentionally limited to very small bounds. Once the candidate
    /// option universe becomes too large for exhaustive subset-pair generation,
    /// the method returns [`CgtError::GenerationUniverseTooLarge`].
    pub fn generate_by_birthday(&mut self, max_birthday: u32) -> Result<Vec<GameId>> {
        let layers = self.generate_birthday_layers(max_birthday)?;
        Ok(Self::flatten_game_layers(layers))
    }

    /// Exhaustively generates the exact reachable-node layer `target_nodes`.
    ///
    /// This returns only games whose reachable subgraph contains exactly
    /// `target_nodes` distinct arena nodes.
    pub fn generate_node_count_layer(&mut self, target_nodes: usize) -> Result<Vec<GameId>> {
        let mut layers = self.generate_node_count_layers(target_nodes)?;
        Ok(layers.remove(&target_nodes).unwrap_or_default())
    }

    /// Exhaustively generates reachable-node layers up to `max_nodes`.
    ///
    /// Each map entry contains only games whose reachable subgraph contains
    /// exactly the key number of distinct arena nodes.
    pub fn generate_node_count_layers(
        &mut self,
        max_nodes: usize,
    ) -> Result<BTreeMap<usize, Vec<GameId>>> {
        let mut layers = BTreeMap::new();
        if max_nodes == 0 {
            return Ok(layers);
        }

        let zero = self.zero();
        layers.insert(1, vec![zero]);

        let mut cumulative = vec![zero];
        for target_nodes in 2..=max_nodes {
            let layer = self.generate_from_option_universe(&cumulative, |arena, game| {
                Ok(arena.reachable_node_count(game)? == target_nodes)
            })?;
            cumulative.extend(layer.iter().copied());
            cumulative.sort_unstable();
            cumulative.dedup();
            layers.insert(target_nodes, layer);
        }

        Ok(layers)
    }

    /// Exhaustively generates all short games whose reachable subgraph contains
    /// at most `max_nodes` distinct arena nodes.
    ///
    /// This is intentionally limited to very small bounds. Once the candidate
    /// option universe becomes too large for exhaustive subset-pair generation,
    /// the method returns [`CgtError::GenerationUniverseTooLarge`].
    pub fn generate_by_node_count(&mut self, max_nodes: usize) -> Result<Vec<GameId>> {
        let layers = self.generate_node_count_layers(max_nodes)?;
        Ok(Self::flatten_game_layers(layers))
    }

    /// Builds a deduplicated canonical corpus from the exact birthday layer.
    pub fn canonical_corpus_birthday_layer(
        &mut self,
        target_birthday: u32,
    ) -> Result<CanonicalCorpus> {
        let games = self.generate_birthday_layer(target_birthday)?;
        self.canonical_corpus_from_games(games)
    }

    /// Builds canonical corpora for each exact birthday layer up to `max_birthday`.
    pub fn canonical_corpus_birthday_layers(
        &mut self,
        max_birthday: u32,
    ) -> Result<BTreeMap<u32, CanonicalCorpus>> {
        let layers = self.generate_birthday_layers(max_birthday)?;
        self.canonical_corpora_from_layers(layers)
    }

    /// Builds a deduplicated canonical corpus from the birthday-bounded generator.
    pub fn canonical_corpus_by_birthday(&mut self, max_birthday: u32) -> Result<CanonicalCorpus> {
        let layers = self.canonical_corpus_birthday_layers(max_birthday)?;
        Ok(Self::flatten_canonical_corpora(layers))
    }

    /// Builds a deduplicated canonical corpus from the exact reachable-node layer.
    pub fn canonical_corpus_node_count_layer(
        &mut self,
        target_nodes: usize,
    ) -> Result<CanonicalCorpus> {
        let games = self.generate_node_count_layer(target_nodes)?;
        self.canonical_corpus_from_games(games)
    }

    /// Builds canonical corpora for each exact reachable-node layer up to `max_nodes`.
    pub fn canonical_corpus_node_count_layers(
        &mut self,
        max_nodes: usize,
    ) -> Result<BTreeMap<usize, CanonicalCorpus>> {
        let layers = self.generate_node_count_layers(max_nodes)?;
        self.canonical_corpora_from_layers(layers)
    }

    /// Builds a deduplicated canonical corpus from the node-count-bounded generator.
    pub fn canonical_corpus_by_node_count(&mut self, max_nodes: usize) -> Result<CanonicalCorpus> {
        let layers = self.canonical_corpus_node_count_layers(max_nodes)?;
        Ok(Self::flatten_canonical_corpora(layers))
    }

    fn analyze_layer(&mut self, raw_games: Vec<GameId>) -> Result<LayerAnalysis> {
        let canonical_corpus = self.canonical_corpus_from_games(raw_games.clone())?;
        let stats = canonical_corpus.stats(self)?;
        Ok(LayerAnalysis {
            raw_games,
            canonical_corpus,
            stats,
        })
    }

    fn analyze_layer_map<K>(
        &mut self,
        layers: BTreeMap<K, Vec<GameId>>,
    ) -> Result<LayerAnalysisReport<K>>
    where
        K: Ord,
    {
        let mut analyses = BTreeMap::new();
        for (key, raw_games) in layers {
            analyses.insert(key, self.analyze_layer(raw_games)?);
        }
        Ok(LayerAnalysisReport::new(analyses))
    }

    fn canonical_corpus_from_games(&mut self, games: Vec<GameId>) -> Result<CanonicalCorpus> {
        let mut canonical_games = Vec::with_capacity(games.len());
        for game in games {
            canonical_games.push(self.canonicalize(game)?);
        }
        Ok(CanonicalCorpus::new(canonical_games))
    }

    fn canonical_corpora_from_layers<K>(
        &mut self,
        layers: BTreeMap<K, Vec<GameId>>,
    ) -> Result<BTreeMap<K, CanonicalCorpus>>
    where
        K: Ord,
    {
        let mut corpora = BTreeMap::new();
        for (key, games) in layers {
            corpora.insert(key, self.canonical_corpus_from_games(games)?);
        }
        Ok(corpora)
    }

    fn flatten_game_layers<K>(layers: BTreeMap<K, Vec<GameId>>) -> Vec<GameId>
    where
        K: Ord,
    {
        let mut games: Vec<_> = layers.into_values().flatten().collect();
        games.sort_unstable();
        games.dedup();
        games
    }

    fn flatten_canonical_corpora<K>(layers: BTreeMap<K, CanonicalCorpus>) -> CanonicalCorpus
    where
        K: Ord,
    {
        let games = layers
            .into_values()
            .flat_map(CanonicalCorpus::into_inner)
            .collect();
        CanonicalCorpus::new(games)
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
