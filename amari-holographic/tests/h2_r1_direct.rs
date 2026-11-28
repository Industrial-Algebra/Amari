use amari_holographic::{AlgebraError, BindingAlgebra, MAPAlgebra};

// The former direct calls are compile-fail regressions on BindingAlgebra::bundle_all.
#[test]
fn public_bundle_paths_reject_nan_before_singleton_shortcut() {
    let items = [MAPAlgebra::new([f64::NAN])];
    for beta in [0., 1., f64::INFINITY] {
        assert_eq!(
            MAPAlgebra::bundle_all(&items, beta).unwrap_err(),
            AlgebraError::NonFinite
        );
        assert_eq!(
            MAPAlgebra::soft_majority(&items, beta).unwrap_err(),
            AlgebraError::NonFinite
        );
    }
    assert_eq!(
        MAPAlgebra::majority(&items).unwrap_err(),
        AlgebraError::NonFinite
    );
}
