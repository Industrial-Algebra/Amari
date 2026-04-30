use amari_tropical::{fold_oplus, fold_otimes, TropicalNumber};

fn main() {
    let branch_a = [TropicalNumber::new(1.0), TropicalNumber::new(3.5)];
    let branch_b = [TropicalNumber::new(2.0), TropicalNumber::new(2.5)];

    let score_a = fold_otimes(branch_a);
    let score_b = fold_otimes(branch_b);
    let best = fold_oplus([score_a, score_b]);

    println!("branch A score: {}", score_a.value());
    println!("branch B score: {}", score_b.value());
    println!("best score: {}", best.value());
}
