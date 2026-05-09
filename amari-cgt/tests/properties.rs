use amari_cgt::{GameArena, GameId};

fn canonical_ids_by_birthday(arena: &mut GameArena, max_birthday: u32) -> Vec<GameId> {
    arena
        .canonical_corpus_by_birthday(max_birthday)
        .unwrap()
        .as_slice()
        .iter()
        .map(|game| game.0)
        .collect()
}

fn canonical_ids_by_node_count(arena: &mut GameArena, max_nodes: usize) -> Vec<GameId> {
    arena
        .canonical_corpus_by_node_count(max_nodes)
        .unwrap()
        .as_slice()
        .iter()
        .map(|game| game.0)
        .collect()
}

#[test]
fn canonicalization_properties_hold_across_small_birthday_universe() {
    let mut arena = GameArena::new();
    let games = arena.generate_by_birthday(2).unwrap();

    for game in games {
        let canonical = arena.canonicalize(game).unwrap().0;
        let canonical_form = arena.to_form(canonical).unwrap();
        let inspection = arena.inspect(game).unwrap();

        assert!(arena.equivalent(game, canonical).unwrap());
        assert!(arena.is_canonical(canonical).unwrap());
        assert_eq!(arena.canonical_id(game).unwrap(), canonical);
        assert_eq!(arena.canonicalize(canonical).unwrap().0, canonical);
        assert_eq!(arena.canonical_form(game).unwrap(), canonical_form);
        assert_eq!(inspection.canonical_game_id(), canonical);
        assert_eq!(inspection.canonical_form(), &canonical_form);
        assert_eq!(inspection.is_canonical(), game == canonical);
    }
}

#[test]
fn negation_and_pairwise_addition_laws_hold_across_small_canonical_corpus() {
    let mut arena = GameArena::new();
    let games = canonical_ids_by_birthday(&mut arena, 2);
    let zero = arena.zero();

    for &game in &games {
        let neg_game = arena.neg(game).unwrap();
        let double_neg = arena.neg(neg_game).unwrap();
        let sum_with_zero = arena.add(game, zero).unwrap();
        let zero_with_sum = arena.add(zero, game).unwrap();
        let self_cancel = arena.add(game, neg_game).unwrap();

        assert!(arena.equivalent(double_neg, game).unwrap());
        assert!(arena.equivalent(sum_with_zero, game).unwrap());
        assert!(arena.equivalent(zero_with_sum, game).unwrap());
        assert!(arena.equivalent(self_cancel, zero).unwrap());

        if arena.is_impartial(game).unwrap() {
            let self_sum = arena.add(game, game).unwrap();
            assert!(arena.equivalent(self_sum, zero).unwrap());
        }
    }

    for &lhs in &games {
        for &rhs in &games {
            let sum_lr = arena.add(lhs, rhs).unwrap();
            let sum_rl = arena.add(rhs, lhs).unwrap();
            let neg_lhs = arena.neg(lhs).unwrap();
            let neg_rhs = arena.neg(rhs).unwrap();
            let neg_sum = arena.neg(sum_lr).unwrap();
            let sum_of_negs = arena.add(neg_lhs, neg_rhs).unwrap();
            let difference = arena.sub(lhs, rhs).unwrap();
            let difference_via_sum = arena.add(lhs, neg_rhs).unwrap();

            assert!(arena.equivalent(sum_lr, sum_rl).unwrap());
            assert!(arena.equivalent(neg_sum, sum_of_negs).unwrap());
            assert!(arena.equivalent(difference, difference_via_sum).unwrap());
            assert_eq!(
                arena.compare(lhs, rhs).unwrap(),
                arena.compare(difference, zero).unwrap()
            );
        }
    }
}

#[test]
fn addition_is_associative_across_small_canonical_node_corpus() {
    let mut arena = GameArena::new();
    let games = canonical_ids_by_node_count(&mut arena, 3);

    for &lhs in &games {
        for &rhs in &games {
            for &third in &games {
                let lhs_plus_rhs = arena.add(lhs, rhs).unwrap();
                let rhs_plus_third = arena.add(rhs, third).unwrap();
                let left_associated = arena.add(lhs_plus_rhs, third).unwrap();
                let right_associated = arena.add(lhs, rhs_plus_third).unwrap();

                assert!(arena.equivalent(left_associated, right_associated).unwrap());
            }
        }
    }
}
