//! Research dependency baseline proof for the 0.25 inverse-rewrite expansion.
//!
//! Pins the exact external surface each research feature is allowed to use:
//! `sha2` as a small no_std core dependency for canonical evidence digests,
//! vendored Z3 under `smt`, and the optional holographic/network guidance
//! crates under their features. (Candle was removed 2026-10-16 — see the
//! note in Cargo.toml; the neural feature stays trait-only until Katakuri.)

use sha2::{Digest, Sha256};

/// `sha2` is a core (always-on) dependency used for canonical relation,
/// language, replay, and research-backend evidence identity. This vector is
/// the well-known SHA-256 of the empty string.
#[test]
fn sha256_core_dependency_matches_known_vector() {
    let digest = Sha256::digest(b"");
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

/// `smt` wires vendored Z3: a trivial satisfiability check proves the
/// vendored solver builds and runs without a system Z3 installation.
#[cfg(feature = "smt")]
#[test]
fn smt_feature_provides_vendored_z3_solver() {
    use z3::{ast::Int, SatResult, Solver};

    let solver = Solver::new();
    let x = Int::new_const("x");
    let three = Int::from_i64(3);
    solver.assert(x.gt(&three));
    assert_eq!(solver.check(), SatResult::Sat);
}

/// `network` guidance keeps its existing optional `amari-network` surface.
#[cfg(feature = "network")]
#[test]
fn network_feature_provides_geometric_network_types() {
    let network = amari_network::GeometricNetwork::<3, 0, 0>::new();
    assert_eq!(network.num_nodes(), 0);
}

/// `holographic-guidance` adds the optional `amari-holographic` dependency
/// used by deterministic trace recall guidance.
#[cfg(feature = "holographic-guidance")]
#[test]
fn holographic_guidance_feature_provides_holographic_types() {
    use amari_holographic::BindingAlgebra;

    let element = amari_holographic::FHRRAlgebra::<8>::identity();
    assert_eq!(element.dimension(), 8);
}
