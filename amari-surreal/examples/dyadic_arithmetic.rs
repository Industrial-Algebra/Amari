use amari_surreal::{Dyadic, ShortSurreal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let half = Dyadic::new(1, 1);
    let quarter = Dyadic::new(1, 2);
    let three_quarters = half.clone() + quarter.clone();
    assert_eq!(three_quarters, Dyadic::new(3, 2));

    let surreal = ShortSurreal::from_dyadic(half) + ShortSurreal::from_integer(1);
    assert_eq!(surreal.to_dyadic(), Dyadic::new(3, 1));

    println!("1/2 + 1/4 = {three_quarters}");
    println!("1 + 1/2 = {surreal}");

    Ok(())
}
