use amari_cgt::{Birthday, GameArena};
use amari_surreal::{Dyadic, ShortSurreal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arena = GameArena::new();

    let zero = arena.zero();
    let one = arena.one()?;
    let half_game = arena.from_options([zero], [one])?;
    let half = ShortSurreal::from_game(&mut arena, half_game)?;

    assert_eq!(half.to_dyadic(), Dyadic::new(1, 1));
    assert_eq!(half.birthday(), Birthday(2));
    assert_eq!(half.provenance(), Some(half_game));

    println!("{{0 | 1}} converts to {half}");

    Ok(())
}
