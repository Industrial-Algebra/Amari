#![cfg(any(feature = "neural", feature = "smt"))]

#[cfg(feature = "neural")]
#[test]
fn differentiable_rule_trait_can_be_implemented() {
    use amari_rewrite::neural::DifferentiableRule;

    struct IdentityRule;
    impl DifferentiableRule<f64> for IdentityRule {
        type Parameters = ();
        type Gradient = ();
        type Error = core::convert::Infallible;

        fn forward(&self, state: &f64) -> Result<f64, Self::Error> { Ok(*state) }
        fn loss(&self, predicted: &f64, target: &f64) -> Result<f64, Self::Error> {
            Ok((predicted - target).abs())
        }
    }

    assert_eq!(IdentityRule.forward(&3.0).unwrap(), 3.0);
}

#[cfg(feature = "smt")]
#[test]
fn rewrite_solver_trait_can_be_implemented() {
    use amari_rewrite::smt::RewriteSolver;

    struct TrivialSolver;
    impl RewriteSolver for TrivialSolver {
        type Term = i32;
        type Certificate = bool;
        type Error = core::convert::Infallible;

        fn prove_equivalent(&self, lhs: &i32, rhs: &i32) -> Result<bool, Self::Error> {
            Ok(lhs == rhs)
        }
    }

    assert!(TrivialSolver.prove_equivalent(&1, &1).unwrap());
}
