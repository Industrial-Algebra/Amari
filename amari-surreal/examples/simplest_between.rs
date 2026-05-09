use amari_surreal::{Dyadic, ShortSurreal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let one = ShortSurreal::one();
    let two = ShortSurreal::from_integer(2);
    let simplest = ShortSurreal::simplest_between(&[one], &[two])?;

    assert_eq!(simplest.to_dyadic(), Dyadic::new(3, 1));
    assert_eq!(
        ShortSurreal::simplest_between(&[], &[])?,
        ShortSurreal::zero()
    );

    println!("the simplest short surreal strictly between 1 and 2 is {simplest}");

    Ok(())
}
