//! Lightweight enumeration-analysis bridges for `amari-cgt`.
//!
//! This module adapts `amari-cgt`'s exact-layer generation reports into simple
//! enumerative sequences that are convenient for growth studies, plotting, and
//! downstream counting experiments.

use crate::EnumerativeResult;
use amari_cgt::{GameArena, LayerAnalysis, LayerAnalysisReport, OutcomeCounts};

/// Enumerative summary for one exact `amari-cgt` layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CgtEnumerationEntry<K> {
    layer: K,
    raw_count: usize,
    canonical_count: usize,
    canonical_reduction: usize,
    numeric_count: usize,
    non_numeric_count: usize,
    impartial_count: usize,
    partizan_count: usize,
    outcome_counts: OutcomeCounts,
}

impl<K: Copy> CgtEnumerationEntry<K> {
    /// Returns the exact layer key.
    #[must_use]
    pub fn layer(&self) -> K {
        self.layer
    }

    /// Returns the number of raw generated games in the layer.
    #[must_use]
    pub fn raw_count(&self) -> usize {
        self.raw_count
    }

    /// Returns the number of canonical classes in the layer.
    #[must_use]
    pub fn canonical_count(&self) -> usize {
        self.canonical_count
    }

    /// Returns the number of raw games eliminated by canonicalization.
    #[must_use]
    pub fn canonical_reduction(&self) -> usize {
        self.canonical_reduction
    }

    /// Returns the number of numeric canonical classes in the layer.
    #[must_use]
    pub fn numeric_count(&self) -> usize {
        self.numeric_count
    }

    /// Returns the number of non-numeric canonical classes in the layer.
    #[must_use]
    pub fn non_numeric_count(&self) -> usize {
        self.non_numeric_count
    }

    /// Returns the number of impartial canonical classes in the layer.
    #[must_use]
    pub fn impartial_count(&self) -> usize {
        self.impartial_count
    }

    /// Returns the number of partizan canonical classes in the layer.
    #[must_use]
    pub fn partizan_count(&self) -> usize {
        self.partizan_count
    }

    /// Returns the outcome-class counts for the layer.
    #[must_use]
    pub fn outcome_counts(&self) -> &OutcomeCounts {
        &self.outcome_counts
    }
}

/// Enumerative sequence summary derived from an `amari-cgt` layer report.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CgtEnumerationSummary<K> {
    entries: Vec<CgtEnumerationEntry<K>>,
}

impl<K: Copy + Ord> CgtEnumerationSummary<K> {
    /// Builds a summary from an `amari-cgt` layer report.
    #[must_use]
    pub fn from_layer_analysis_report(report: LayerAnalysisReport<K>) -> Self {
        let entries = report
            .iter()
            .map(|(layer, analysis)| Self::entry_from_analysis(*layer, analysis))
            .collect();
        Self { entries }
    }

    /// Returns the layer summaries.
    #[must_use]
    pub fn entries(&self) -> &[CgtEnumerationEntry<K>] {
        &self.entries
    }

    /// Returns the number of exact layers represented in the summary.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the summary is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns total raw generated games across all layers.
    #[must_use]
    pub fn total_raw_count(&self) -> usize {
        self.entries
            .iter()
            .map(CgtEnumerationEntry::raw_count)
            .sum()
    }

    /// Returns total canonical classes across all layers.
    #[must_use]
    pub fn total_canonical_count(&self) -> usize {
        self.entries
            .iter()
            .map(CgtEnumerationEntry::canonical_count)
            .sum()
    }

    /// Returns total canonical reduction across all layers.
    #[must_use]
    pub fn total_canonical_reduction(&self) -> usize {
        self.entries
            .iter()
            .map(CgtEnumerationEntry::canonical_reduction)
            .sum()
    }

    /// Returns the raw-game growth sequence.
    #[must_use]
    pub fn raw_sequence(&self) -> Vec<(K, usize)> {
        self.sequence(CgtEnumerationEntry::raw_count)
    }

    /// Returns the canonical-class growth sequence.
    #[must_use]
    pub fn canonical_sequence(&self) -> Vec<(K, usize)> {
        self.sequence(CgtEnumerationEntry::canonical_count)
    }

    /// Returns the canonical-reduction growth sequence.
    #[must_use]
    pub fn reduction_sequence(&self) -> Vec<(K, usize)> {
        self.sequence(CgtEnumerationEntry::canonical_reduction)
    }

    /// Returns the numeric-class growth sequence.
    #[must_use]
    pub fn numeric_sequence(&self) -> Vec<(K, usize)> {
        self.sequence(CgtEnumerationEntry::numeric_count)
    }

    /// Returns the non-numeric-class growth sequence.
    #[must_use]
    pub fn non_numeric_sequence(&self) -> Vec<(K, usize)> {
        self.sequence(CgtEnumerationEntry::non_numeric_count)
    }

    /// Returns the impartial-class growth sequence.
    #[must_use]
    pub fn impartial_sequence(&self) -> Vec<(K, usize)> {
        self.sequence(CgtEnumerationEntry::impartial_count)
    }

    /// Returns the partizan-class growth sequence.
    #[must_use]
    pub fn partizan_sequence(&self) -> Vec<(K, usize)> {
        self.sequence(CgtEnumerationEntry::partizan_count)
    }

    /// Returns the outcome-class growth sequence.
    #[must_use]
    pub fn outcome_sequence(&self) -> Vec<(K, OutcomeCounts)> {
        self.entries
            .iter()
            .map(|entry| (entry.layer(), entry.outcome_counts().clone()))
            .collect()
    }

    /// Returns the cumulative raw-game growth sequence.
    #[must_use]
    pub fn cumulative_raw_sequence(&self) -> Vec<(K, usize)> {
        self.cumulative_sequence(CgtEnumerationEntry::raw_count)
    }

    /// Returns the cumulative canonical-class growth sequence.
    #[must_use]
    pub fn cumulative_canonical_sequence(&self) -> Vec<(K, usize)> {
        self.cumulative_sequence(CgtEnumerationEntry::canonical_count)
    }

    fn entry_from_analysis(layer: K, analysis: &LayerAnalysis) -> CgtEnumerationEntry<K> {
        let stats = analysis.stats();
        CgtEnumerationEntry {
            layer,
            raw_count: analysis.raw_game_count(),
            canonical_count: analysis.canonical_game_count(),
            canonical_reduction: analysis.canonical_reduction(),
            numeric_count: stats.numeric_games(),
            non_numeric_count: stats.non_numeric_games(),
            impartial_count: stats.impartial_games(),
            partizan_count: stats.partizan_games(),
            outcome_counts: stats.outcome_counts().clone(),
        }
    }

    fn sequence<F>(&self, mut value: F) -> Vec<(K, usize)>
    where
        F: FnMut(&CgtEnumerationEntry<K>) -> usize,
    {
        self.entries
            .iter()
            .map(|entry| (entry.layer(), value(entry)))
            .collect()
    }

    fn cumulative_sequence<F>(&self, mut value: F) -> Vec<(K, usize)>
    where
        F: FnMut(&CgtEnumerationEntry<K>) -> usize,
    {
        let mut running = 0usize;
        self.entries
            .iter()
            .map(|entry| {
                running += value(entry);
                (entry.layer(), running)
            })
            .collect()
    }
}

/// Builds a birthday-layer growth summary using a fresh `amari-cgt` arena.
pub fn cgt_birthday_growth_summary(
    max_birthday: u32,
) -> EnumerativeResult<CgtEnumerationSummary<u32>> {
    let mut arena = GameArena::new();
    cgt_birthday_growth_summary_in(&mut arena, max_birthday)
}

/// Builds a birthday-layer growth summary using an existing `amari-cgt` arena.
pub fn cgt_birthday_growth_summary_in(
    arena: &mut GameArena,
    max_birthday: u32,
) -> EnumerativeResult<CgtEnumerationSummary<u32>> {
    let report = arena.analyze_birthday_layers(max_birthday)?;
    Ok(CgtEnumerationSummary::from_layer_analysis_report(report))
}

/// Builds a reachable-node growth summary using a fresh `amari-cgt` arena.
pub fn cgt_node_count_growth_summary(
    max_nodes: usize,
) -> EnumerativeResult<CgtEnumerationSummary<usize>> {
    let mut arena = GameArena::new();
    cgt_node_count_growth_summary_in(&mut arena, max_nodes)
}

/// Builds a reachable-node growth summary using an existing `amari-cgt` arena.
pub fn cgt_node_count_growth_summary_in(
    arena: &mut GameArena,
    max_nodes: usize,
) -> EnumerativeResult<CgtEnumerationSummary<usize>> {
    let report = arena.analyze_node_count_layers(max_nodes)?;
    Ok(CgtEnumerationSummary::from_layer_analysis_report(report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn birthday_growth_summary_tracks_small_sequences() {
        let summary = cgt_birthday_growth_summary(1).unwrap();

        assert_eq!(summary.len(), 2);
        assert_eq!(summary.total_raw_count(), 4);
        assert_eq!(summary.total_canonical_count(), 4);
        assert_eq!(summary.total_canonical_reduction(), 0);
        assert_eq!(summary.raw_sequence(), vec![(0, 1), (1, 3)]);
        assert_eq!(summary.canonical_sequence(), vec![(0, 1), (1, 3)]);
        assert_eq!(summary.numeric_sequence(), vec![(0, 1), (1, 2)]);
        assert_eq!(summary.impartial_sequence(), vec![(0, 1), (1, 1)]);
        assert_eq!(summary.partizan_sequence(), vec![(0, 0), (1, 2)]);
        assert_eq!(summary.cumulative_raw_sequence(), vec![(0, 1), (1, 4)]);
        assert_eq!(
            summary.cumulative_canonical_sequence(),
            vec![(0, 1), (1, 4)]
        );
        assert_eq!(summary.outcome_sequence()[0].1.previous_player_wins(), 1);
        assert_eq!(summary.outcome_sequence()[1].1.next_player_wins(), 1);
    }

    #[test]
    fn deeper_birthday_summary_detects_canonical_reduction() {
        let summary = cgt_birthday_growth_summary(2).unwrap();

        assert!(summary.total_canonical_reduction() > 0);
        assert!(summary
            .reduction_sequence()
            .iter()
            .any(|&(_, count)| count > 0));
    }

    #[test]
    fn node_count_growth_summary_tracks_small_sequences() {
        let summary = cgt_node_count_growth_summary(2).unwrap();

        assert_eq!(summary.len(), 2);
        assert_eq!(summary.raw_sequence(), vec![(1, 1), (2, 3)]);
        assert_eq!(summary.canonical_sequence(), vec![(1, 1), (2, 3)]);
        assert_eq!(summary.numeric_sequence(), vec![(1, 1), (2, 2)]);
        assert_eq!(summary.impartial_sequence(), vec![(1, 1), (2, 1)]);
    }

    #[test]
    fn growth_summary_can_reuse_existing_arena() {
        let mut arena = GameArena::new();
        let summary = cgt_birthday_growth_summary_in(&mut arena, 1).unwrap();

        assert_eq!(summary.raw_sequence(), vec![(0, 1), (1, 3)]);
    }
}
