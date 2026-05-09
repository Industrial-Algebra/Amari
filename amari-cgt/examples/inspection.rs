use amari_cgt::GameArena;

fn main() -> Result<(), amari_cgt::CgtError> {
    let mut arena = GameArena::new();

    let zero = arena.zero();
    let one = arena.one()?;
    let half = arena.from_options([zero], [one])?;
    let inspection = arena.inspect(half)?;

    assert!(inspection.is_numeric());
    assert!(inspection.is_partizan());
    assert!(inspection.is_canonical());

    println!("game: {}", arena.format_game(half)?);
    println!("canonical form: {}", inspection.canonical_form());
    println!("birthday: {:?}", inspection.birthday());
    println!("outcome: {:?}", inspection.outcome());

    Ok(())
}
