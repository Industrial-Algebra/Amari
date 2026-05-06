use amari_tropical::{CnfTerm, OrdinalArena, OrdinalWeight, TropicalError};

fn main() -> Result<(), TropicalError> {
    let mut arena = OrdinalArena::new();

    let zero = arena.zero();
    let one = arena.one();
    let two = arena.finite(2);
    let ordinal = arena.intern_cnf(vec![
        CnfTerm::new(two, 1),
        CnfTerm::new(one, 3),
        CnfTerm::new(zero, 5),
    ])?;

    let inspection = arena.inspect(ordinal)?;
    let weight = OrdinalWeight::from_ordinal(ordinal);
    let weight_inspection = arena.inspect_weight(weight)?;

    println!("ordinal: {}", inspection.rendered());
    println!("kind: {:?}", inspection.kind());
    println!("term count: {}", inspection.term_count());

    if let Some(exponent) = inspection.leading_exponent() {
        println!("leading exponent: {}", arena.format_ordinal(exponent)?);
    }

    if let Some(valuation) = weight_inspection.valuation() {
        println!("valuation: {}", arena.format_ordinal(valuation)?);
    }

    Ok(())
}
