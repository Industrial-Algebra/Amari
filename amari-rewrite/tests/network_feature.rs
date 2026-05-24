#![cfg(feature = "network")]

use amari_rewrite::{network::RewriteGraphSummary, trs::Term};

#[test]
fn rewrite_graph_summary_tracks_terms_and_steps() {
    let summary = RewriteGraphSummary::from_trace(&[
        Term::constant("a"),
        Term::sym("f", [Term::constant("a")]),
    ]);

    assert_eq!(summary.nodes, 2);
    assert_eq!(summary.edges, 1);
}
