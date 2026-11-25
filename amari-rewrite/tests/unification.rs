//! Unification and checked substitution contract tests
//! (0.25 Cohort 2, Task 7).
//!
//! Deterministic first-order unification with occurs check, canonical
//! binding order, checked idempotent composition, and strict
//! term/operation limits. Shared by critical pairs, inverse frontier
//! meeting, constraints, and synthesis.

use amari_rewrite::analysis::{unify, unify_with};
use amari_rewrite::relation::{RelationLimits, RelationResources};
use amari_rewrite::trs::{Substitution, Term};
use amari_rewrite::RewriteError;

fn resources() -> RelationResources {
    RelationResources::new(&RelationLimits::default())
}

#[test]
fn mgu_binds_variables_deterministically() {
    let mut budget = resources();
    let subst = unify(&Term::var("X"), &Term::constant("a"), &mut budget).unwrap();
    assert_eq!(subst.get("X"), Some(&Term::constant("a")));
    assert_eq!(subst.len(), 1);

    // Variable-variable: first side binds to the second.
    let mut budget = resources();
    let subst = unify(&Term::var("X"), &Term::var("Y"), &mut budget).unwrap();
    assert_eq!(subst.get("X"), Some(&Term::var("Y")));

    // Structured terms bind both variables.
    let mut budget = resources();
    let subst = unify(
        &Term::sym("f", vec![Term::var("X"), Term::constant("b")]),
        &Term::sym("f", vec![Term::constant("a"), Term::var("Y")]),
        &mut budget,
    )
    .unwrap();
    assert_eq!(subst.get("X"), Some(&Term::constant("a")));
    assert_eq!(subst.get("Y"), Some(&Term::constant("b")));
}

#[test]
fn mgu_makes_sides_equal() {
    let left = Term::sym(
        "f",
        vec![Term::var("X"), Term::sym("g", vec![Term::var("X")])],
    );
    let right = Term::sym(
        "f",
        vec![Term::sym("h", vec![Term::var("Y")]), Term::var("Z")],
    );
    let mut budget = resources();
    let subst = unify(&left, &right, &mut budget).unwrap();
    assert_eq!(subst.apply(&left), subst.apply(&right));
}

#[test]
fn occurs_check_rejects_cyclic_bindings() {
    let mut budget = resources();
    let result = unify(
        &Term::var("X"),
        &Term::sym("f", vec![Term::var("X")]),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(RewriteError::UnificationFailure { .. })
    ));

    // Nested: X ~ f(Y), Y ~ g(X) is cyclic through the substitution.
    let mut budget = resources();
    let result = unify(
        &Term::sym("f", vec![Term::var("X"), Term::var("Y")]),
        &Term::sym(
            "f",
            vec![
                Term::sym("g", vec![Term::var("Y")]),
                Term::sym("h", vec![Term::var("X")]),
            ],
        ),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(RewriteError::UnificationFailure { .. })
    ));
}

#[test]
fn repeated_variables_and_shape_mismatches_fail() {
    // f(X, X) cannot equal f(a, b).
    let mut budget = resources();
    let result = unify(
        &Term::sym("f", vec![Term::var("X"), Term::var("X")]),
        &Term::sym("f", vec![Term::constant("a"), Term::constant("b")]),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(RewriteError::UnificationFailure { .. })
    ));

    // Arity mismatch.
    let mut budget = resources();
    let result = unify(
        &Term::sym("f", vec![Term::var("X")]),
        &Term::sym("f", vec![Term::var("X"), Term::var("Y")]),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(RewriteError::UnificationFailure { .. })
    ));

    // Head-symbol clash.
    let mut budget = resources();
    let result = unify(
        &Term::sym("f", vec![Term::var("X")]),
        &Term::sym("g", vec![Term::var("X")]),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(RewriteError::UnificationFailure { .. })
    ));
}

#[test]
fn alpha_renamed_pairs_unify_isomorphically() {
    let left = Term::sym("f", vec![Term::var("X"), Term::var("X")]);
    let right = Term::sym("f", vec![Term::var("Y"), Term::var("Y")]);
    let mut budget = resources();
    let subst = unify(&left, &right, &mut budget).unwrap();
    assert_eq!(subst.apply(&left), subst.apply(&right));
    // Exactly one binding: X ↦ Y.
    assert_eq!(subst.len(), 1);
}

#[test]
fn checked_composition_combines_and_validates() {
    // {Y ↦ X} then {X ↦ a} composes to {Y ↦ a, X ↦ a}.
    let inner = Substitution::new().with("Y", Term::var("X"));
    let outer = Substitution::new().with("X", Term::constant("a"));
    let composed = outer.compose(&inner).unwrap();
    assert_eq!(composed.get("Y"), Some(&Term::constant("a")));
    assert_eq!(composed.get("X"), Some(&Term::constant("a")));

    // Incompatible bindings for the same variable are rejected.
    let a = Substitution::new().with("X", Term::constant("a"));
    let b = Substitution::new().with("X", Term::constant("b"));
    assert!(matches!(
        a.compose(&b),
        Err(RewriteError::InvalidSubstitution { .. })
    ));

    // Non-idempotent results are rejected: {X ↦ f(Y)} ∘ {Y ↦ X}.
    let inner = Substitution::new().with("Y", Term::var("X"));
    let outer = Substitution::new().with("X", Term::sym("f", vec![Term::var("Y")]));
    assert!(matches!(
        outer.compose(&inner),
        Err(RewriteError::InvalidSubstitution { .. })
    ));
}

#[test]
fn unification_respects_operation_limits() {
    let limits = RelationLimits::new(4_096, 64, 4_096, 2).unwrap();
    let mut budget = RelationResources::new(&limits);
    // Three argument pairs exceed a two-operation budget.
    let result = unify(
        &Term::sym(
            "f",
            vec![
                Term::constant("a"),
                Term::constant("b"),
                Term::constant("c"),
            ],
        ),
        &Term::sym(
            "f",
            vec![
                Term::constant("a"),
                Term::constant("b"),
                Term::constant("c"),
            ],
        ),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
}

/// The doubling chain of the F16 replay finding: leaf `f(Z, X0, ..., Xn)` and
/// goal `f(X0, pair(X1,X1), ..., pair(Xn,Xn), a)`.
fn doubling_pair(arity: usize) -> (Term, Term) {
    let mut leaf_args = vec![Term::var("Z")];
    leaf_args.extend((0..arity).map(|index| Term::var(format!("X{index}"))));
    let mut goal_args = vec![Term::var("X0")];
    // Ascending order: `pair(X1,X1)` pairs with the leaf's `X0`, and so on.
    for index in 1..arity {
        let variable = Term::var(format!("X{index}"));
        goal_args.push(Term::sym("pair", vec![variable.clone(), variable]));
    }
    goal_args.push(Term::constant("a"));
    (Term::sym("f", leaf_args), Term::sym("f", goal_args))
}

/// F16 (P1): resolve/occurs work is charged against the unified budget.
///
/// The outer worklist of the doubling chain is tiny (one pair per argument), so
/// a budget that the worklist alone fits used to be irrelevant: the occurs
/// checks resolved the whole doubling expansion unmetered. Resolution and
/// occurs inspection now share the caller's [`RelationResources`], so a budget
/// that cannot pay for the traversal is a typed operations rejection instead of
/// unbounded work.
#[test]
fn resolution_and_occurs_work_is_metered() {
    let (leaf, goal) = doubling_pair(21);
    let limits = RelationLimits::new(4_096, 64, 4_096, 256).expect("valid tightened limits");
    let mut budget = RelationResources::new(&limits);
    let result = unify(&leaf, &goal, &mut budget);
    assert!(
        matches!(
            result,
            Err(RewriteError::RelationLimitExceeded {
                resource: "operations",
                ..
            })
        ),
        "an under-budget traversal is a typed operations rejection, got {result:?}"
    );
    // Negative control: the traversal is not charged per *expansion* node, so
    // the same unification inside the default budget still succeeds with the
    // same unifier and a linear meter.
    let mut budget = resources();
    let substitution = unify(&leaf, &goal, &mut budget).expect("the chain unifies");
    assert_eq!(
        substitution.len(),
        22,
        "one binding per chain link, plus the free leading variable"
    );
    assert!(
        budget.operations() <= 4_096,
        "each reachable binding is inspected once: {} operations",
        budget.operations()
    );
}

/// F23 (P1): a successful unification IS a unifier under `Substitution::apply`.
///
/// The old pin asserted that a pre-existing cycle `Y |-> g(Y)` merely "resolves
/// to its fixed point" and accepted `X |-> g(Y)`. That substitution is not a
/// unifier: `apply(X)` is `g(g(Y))` while `apply(Y)` is `g(Y)`, so the two sides
/// the caller asked to be equal are still different terms. Any successful
/// `unify`/`unify_with` must satisfy `apply(left) == apply(right)`; a
/// pre-existing non-identity cycle cannot produce such a substitution and is a
/// typed rejection instead.
#[test]
fn a_pre_existing_non_identity_cycle_is_a_typed_rejection() {
    // `Y |-> g(Y)`: the cycle the old test accepted.
    let mut cyclic = Substitution::new().with("Y", Term::sym("g", vec![Term::var("Y")]));
    let mut budget = resources();
    let error = unify_with(&mut cyclic, &Term::var("X"), &Term::var("Y"), &mut budget)
        .expect_err("a non-identity cycle cannot be unified soundly");
    assert!(
        matches!(error, RewriteError::InvalidSubstitution { .. }),
        "the typed rejection names the substitution, got {error:?}"
    );
    // Rejected before any binding was made.
    assert_eq!(cyclic.get("X"), None, "no partial binding is installed");

    // The same cycle reached through a range rather than a root: X |-> g(Y),
    // Y |-> X. `apply(X)` is `g(X)` and `apply(Y)` is `g(Y)`.
    let mut through_range = Substitution::new()
        .with("X", Term::sym("g", vec![Term::var("Y")]))
        .with("Y", Term::var("X"));
    let mut budget = resources();
    let error = unify_with(
        &mut through_range,
        &Term::var("X"),
        &Term::var("Y"),
        &mut budget,
    )
    .expect_err("a range cycle cannot be unified soundly");
    assert!(matches!(error, RewriteError::InvalidSubstitution { .. }));

    // A pure variable cycle is rejected too: X |-> Y, Y |-> X leaves
    // `apply(X) = X` and `apply(Y) = Y`, which are still different terms.
    let mut variable_cycle = Substitution::new()
        .with("X", Term::var("Y"))
        .with("Y", Term::var("X"));
    let mut budget = resources();
    let error = unify_with(
        &mut variable_cycle,
        &Term::var("X"),
        &Term::var("Z"),
        &mut budget,
    )
    .expect_err("a variable cycle cannot be unified soundly");
    assert!(matches!(error, RewriteError::InvalidSubstitution { .. }));
}

/// F23: an IDENTITY cycle is a supported fixed point.
///
/// `W |-> W` leaves `W` in place under application, so `W` and `W` denote the
/// same term and binding another variable to it is sound.
#[test]
fn an_identity_cycle_remains_a_supported_fixed_point() {
    let mut identity = Substitution::new().with("W", Term::var("W"));
    let mut budget = resources();
    unify_with(&mut identity, &Term::var("V"), &Term::var("W"), &mut budget)
        .expect("W := W is the identity");
    assert_eq!(identity.get("V"), Some(&Term::var("W")));
    assert_eq!(
        identity.apply(&Term::var("V")),
        identity.apply(&Term::var("W")),
        "the postcondition holds for the identity cycle"
    );
    // A reflexive equation over the identity cycle adds nothing.
    let mut budget = resources();
    unify_with(&mut identity, &Term::var("W"), &Term::var("W"), &mut budget)
        .expect("W = W is decided");
    assert_eq!(identity.len(), 2);
}

/// F23 postcondition: every successful unification makes both sides equal under
/// `apply`, including over a pre-existing acyclic substitution.
#[test]
fn every_successful_unification_satisfies_the_apply_postcondition() {
    let acyclic_cases = vec![
        (Term::var("X"), Term::constant("a")),
        (Term::var("X"), Term::var("Y")),
        (
            Term::sym(
                "f",
                vec![Term::var("X"), Term::sym("g", vec![Term::var("X")])],
            ),
            Term::sym(
                "f",
                vec![Term::sym("h", vec![Term::var("Y")]), Term::var("Z")],
            ),
        ),
        (
            Term::sym("f", vec![Term::var("X"), Term::var("X")]),
            Term::sym("f", vec![Term::var("Y"), Term::var("Y")]),
        ),
        (
            Term::sym("f", vec![Term::var("X"), Term::constant("b")]),
            Term::sym("f", vec![Term::constant("a"), Term::var("Y")]),
        ),
    ];
    for (left, right) in acyclic_cases {
        let mut budget = resources();
        let substitution = unify(&left, &right, &mut budget).expect("the pair unifies");
        assert_eq!(
            substitution.apply(&left),
            substitution.apply(&right),
            "a successful unification must make both sides equal"
        );
    }

    // The same postcondition through a pre-existing ACYCLIC substitution:
    // {A |-> B, B |-> f(C)} extended by unifying f(A) with f(D).
    let mut chained = Substitution::new()
        .with("A", Term::var("B"))
        .with("B", Term::sym("f", vec![Term::var("C")]));
    let left = Term::sym("f", vec![Term::var("A")]);
    let right = Term::sym("f", vec![Term::var("D")]);
    let mut budget = resources();
    unify_with(&mut chained, &left, &right, &mut budget).expect("an acyclic chain unifies");
    assert_eq!(chained.apply(&left), chained.apply(&right));
    assert_eq!(
        chained.get("D"),
        Some(&Term::sym("f", vec![Term::var("C")]))
    );
}

/// F25 (P2): a reflexive equation over a doubling substitution is decided
/// without unfolding the chain.
///
/// `X20 |-> a`, `X19 |-> pair(X20,X20)`, ... `X0 |-> pair(X1,X1)`: the equation
/// `X0 = X0` used to resolve both sides to `pair(X1,X1)`, enqueue that pair
/// TWICE, and recurse, attempting `2^21 - 1` duplicate pairs until the operation
/// budget ran out. The same-variable fast path decides it before resolution:
/// one charged pop, and no binding at all.
#[test]
fn a_reflexive_equation_over_a_doubling_substitution_does_not_unfold() {
    let mut substitution = doubling_substitution(20);
    let bindings = substitution.len();
    let mut budget = resources();
    let result = unify_with(
        &mut substitution,
        &Term::var("X0"),
        &Term::var("X0"),
        &mut budget,
    );
    assert!(
        result.is_ok(),
        "X0 = X0 must be decidable without unfolding the chain, got {result:?}"
    );
    assert_eq!(
        substitution.len(),
        bindings,
        "a reflexive equation binds nothing"
    );
    // The measured work is admission only: the borrowed shape of the
    // pre-existing ranges (one operation per node: 1 + 20 pairs of 3 nodes =
    // 61), the acyclic binding-graph walk (21 bindings plus their 20 range
    // occurrences), the two one-node inputs, the charged enqueue and the one
    // pop. It is a constant, not a function of the chain's expansion.
    assert!(
        budget.operations() <= 128,
        "the reflexive equation costs admission plus one pop: {} operations",
        budget.operations()
    );
}

/// The doubling substitution of the F25/F16 doubling chain.
fn doubling_substitution(levels: usize) -> Substitution {
    let mut substitution = Substitution::new();
    substitution.insert(format!("X{levels}"), Term::constant("a"));
    for index in (0..levels).rev() {
        let next = Term::var(format!("X{}", index + 1));
        substitution.insert(
            format!("X{index}"),
            Term::sym("pair", vec![next.clone(), next]),
        );
    }
    substitution
}

/// A pre-existing chain `P0 |-> P1 |-> ... |-> P{links} |-> c`.
fn chain_substitution(prefix: &str, links: usize) -> Substitution {
    let mut substitution = Substitution::new();
    for index in 0..links {
        substitution.insert(
            format!("{prefix}{index}"),
            Term::var(format!("{prefix}{}", index + 1)),
        );
    }
    substitution.insert(format!("{prefix}{links}"), Term::constant("c"));
    substitution
}

/// The union of two chains under distinct names.
fn two_chains(links: usize) -> Substitution {
    let mut substitution = chain_substitution("A", links);
    for (variable, term) in chain_substitution("C", links).iter() {
        substitution.insert(variable.clone(), term.clone());
    }
    substitution
}

/// F25: duplicate equation pairs are decided once while the substitution is
/// stable.
///
/// Both sides of the duplicated pair are ROOTS of a 41-link pre-existing chain,
/// so deciding one occurrence costs a 41-step resolution walk per side. Two
/// structurally distinct pairs of the same size are the baseline (each must be
/// resolved); the same pair twice under an unchanged substitution is remembered,
/// so the duplicate costs one charged pop instead of a second walk.
#[test]
fn duplicate_equations_are_deduplicated_while_the_substitution_is_stable() {
    let links = 40;
    let distinct = {
        let mut substitution = two_chains(links);
        let left = Term::sym("f", vec![Term::var("A0"), Term::var("A1")]);
        let right = Term::sym("f", vec![Term::var("C0"), Term::var("C1")]);
        let mut budget = resources();
        unify_with(&mut substitution, &left, &right, &mut budget).expect("the chain roots meet");
        budget.operations()
    };

    let mut substitution = two_chains(links);
    let left = Term::sym("f", vec![Term::var("A0"), Term::var("A0")]);
    let right = Term::sym("f", vec![Term::var("C0"), Term::var("C0")]);
    let mut budget = resources();
    unify_with(&mut substitution, &left, &right, &mut budget).expect("the chain roots meet");
    let duplicated = budget.operations();
    assert!(
        duplicated + 64 <= distinct,
        "the duplicate equation pair is not re-walked: {duplicated} operations for the \
         same pair twice against {distinct} for two distinct pairs"
    );
}

/// F25 binding-mutation control: a remembered pair is never reused across a
/// mutation.
///
/// The same chain-root pair appears at positions 0 and 2; the LIFO worklist
/// reaches position 2 first (remembering the pair), then the binding `Y := a` at
/// position 1, and only then the identical pair at position 0. The mutation must
/// invalidate the cache, so the second copy pays its resolution walk again.
#[test]
fn a_binding_mutation_invalidates_the_deduplication_cache() {
    let links = 40;
    let without_duplicate = {
        let mut substitution = two_chains(links);
        let left = Term::sym("f", vec![Term::var("A0"), Term::var("Y")]);
        let right = Term::sym("f", vec![Term::var("C0"), Term::constant("a")]);
        let mut budget = resources();
        unify_with(&mut substitution, &left, &right, &mut budget).expect("the chain roots meet");
        budget.operations()
    };

    let mut substitution = two_chains(links);
    let left = Term::sym("f", vec![Term::var("A0"), Term::var("Y"), Term::var("A0")]);
    let right = Term::sym(
        "f",
        vec![Term::var("C0"), Term::constant("a"), Term::var("C0")],
    );
    let mut budget = resources();
    unify_with(&mut substitution, &left, &right, &mut budget).expect("the chain roots meet");
    assert_eq!(substitution.get("Y"), Some(&Term::constant("a")));
    assert_eq!(
        substitution.apply(&left),
        substitution.apply(&right),
        "the postcondition still holds after the invalidation"
    );
    assert!(
        budget.operations() >= without_duplicate + 40,
        "the post-mutation copy is walked again: {} operations against {} without the \
         duplicate",
        budget.operations(),
        without_duplicate
    );
}

/// Resolution precedes decomposition: a BOUND variable whose range is a symbol
/// is decomposed into the range's arguments, not into itself.
///
/// `{A |-> f(X,X)}` against `f(a,b)`: the roots resolve to agreeing `f` heads, so
/// the argument equations `X = a` and `X = b` are what decide the result. A
/// decomposition that pushed the unresolved sides would compare `A` with `f(a,b)`
/// (a variable against a symbol, binding `A` to it) and lose the clash.
#[test]
fn a_bound_variable_is_decomposed_after_resolution() {
    let bound_range = Term::sym("f", vec![Term::var("X"), Term::var("X")]);

    let mut substitution = Substitution::new().with("A", bound_range.clone());
    let mut budget = resources();
    let error = unify_with(
        &mut substitution,
        &Term::var("A"),
        &Term::sym("f", vec![Term::constant("a"), Term::constant("b")]),
        &mut budget,
    )
    .expect_err("the resolved arguments cannot agree");
    assert!(matches!(error, RewriteError::UnificationFailure { .. }));

    // The satisfiable control: `A = f(a,a)` binds the range's variable.
    let mut substitution = Substitution::new().with("A", bound_range);
    let right = Term::sym("f", vec![Term::constant("a"), Term::constant("a")]);
    let mut budget = resources();
    unify_with(&mut substitution, &Term::var("A"), &right, &mut budget)
        .expect("the resolved arguments agree");
    assert_eq!(substitution.get("X"), Some(&Term::constant("a")));
    assert_eq!(
        substitution.apply(&Term::var("A")),
        substitution.apply(&right),
        "the postcondition holds through the bound range"
    );
}
