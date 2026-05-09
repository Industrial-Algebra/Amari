use amari_cgt::{GameArena, OutcomeClass};

fn main() -> Result<(), amari_cgt::CgtError> {
    let mut arena = GameArena::new();

    let zero = arena.zero();
    let star = arena.star()?;
    let one = arena.one()?;
    let minus_one = arena.minus_one()?;
    let cancelled = arena.add(one, minus_one)?;

    assert_eq!(arena.outcome(zero)?, OutcomeClass::PreviousPlayerWins);
    assert_eq!(arena.outcome(star)?, OutcomeClass::NextPlayerWins);
    assert_eq!(arena.outcome(one)?, OutcomeClass::LeftWins);
    assert_eq!(arena.outcome(minus_one)?, OutcomeClass::RightWins);
    assert!(arena.equivalent(cancelled, zero)?);

    println!("1 + (-1) is equivalent to 0");
    println!("* is a first-player win in normal play");

    Ok(())
}
