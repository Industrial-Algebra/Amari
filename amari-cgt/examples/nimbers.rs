use amari_cgt::{GameArena, Nimber};

fn main() -> Result<(), amari_cgt::CgtError> {
    let mut arena = GameArena::new();

    let heap_one = arena.nim_heap(1)?;
    let heap_two = arena.nim_heap(2)?;
    let heap_sum = arena.add(heap_one, heap_two)?;

    assert!(arena.is_impartial(heap_one)?);
    assert_eq!(arena.grundy(heap_one)?, Nimber(1));
    assert_eq!(arena.grundy(heap_two)?, Nimber(2));
    assert_eq!(arena.grundy(heap_sum)?, Nimber(3));

    println!("nimber(1) xor nimber(2) = nimber(3)");

    Ok(())
}
