use amari_cgt::{GameArena, GameForm};

fn main() -> Result<(), amari_cgt::CgtError> {
    let half_form = GameForm::new([GameForm::zero()], [GameForm::one()]);

    let mut arena = GameArena::new();
    let half = arena.intern_form(&half_form)?;
    let round_trip = arena.to_form(half)?;

    assert_eq!(round_trip, half_form);
    assert!(arena.is_numeric(half)?);

    println!("structural form birthday: {:?}", round_trip.birthday());

    Ok(())
}
