use amari_tropical::{CnfTerm, OrdinalArena, OrdinalWeight, TropicalError};

fn main() -> Result<(), TropicalError> {
    let mut arena = OrdinalArena::new();

    let one = arena.one();
    let omega = arena.omega();
    let omega_squared = arena.intern_cnf(vec![CnfTerm::new(omega, 1)])?;

    let candidate_a = arena.compose_weights(&[
        OrdinalWeight::from_ordinal(omega),
        OrdinalWeight::from_ordinal(one),
    ])?;
    let candidate_b = arena.compose_weights(&[OrdinalWeight::from_ordinal(omega_squared)])?;
    let candidate_c = arena.compose_weights(&[
        OrdinalWeight::from_ordinal(omega),
        OrdinalWeight::from_ordinal(omega),
    ])?;

    let best = arena.best_weight(&[candidate_a, candidate_b, candidate_c])?;

    println!("candidate A: {}", arena.format_weight(candidate_a)?);
    println!("candidate B: {}", arena.format_weight(candidate_b)?);
    println!("candidate C: {}", arena.format_weight(candidate_c)?);
    println!("best: {}", arena.format_weight(best)?);

    Ok(())
}
