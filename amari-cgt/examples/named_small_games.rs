use amari_cgt::{GameArena, GameComparison};

fn main() -> Result<(), amari_cgt::CgtError> {
    let mut arena = GameArena::new();

    let zero = arena.zero();
    let star = arena.star()?;
    let one = arena.one()?;
    let minus_one = arena.minus_one()?;

    assert_eq!(arena.compare(one, zero)?, GameComparison::Greater);
    assert_eq!(arena.compare(minus_one, zero)?, GameComparison::Less);
    assert_eq!(arena.compare(star, zero)?, GameComparison::Fuzzy);

    println!("0 has birthday {:?}", arena.birthday(zero)?);
    println!("* has birthday {:?}", arena.birthday(star)?);
    println!("1 has birthday {:?}", arena.birthday(one)?);
    println!("-1 has birthday {:?}", arena.birthday(minus_one)?);

    Ok(())
}
