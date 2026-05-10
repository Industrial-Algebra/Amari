# amari-surcomplex

Exact rational surcomplex numbers for the [Amari](https://github.com/justinelliottcobb/Amari) mathematical computing library.

`amari-surcomplex` provides [`RationalSurcomplex`](crate::RationalSurcomplex), an exact complex number backed by the rational surreal scalars in `amari-surreal`. All arithmetic — addition, subtraction, multiplication, division, conjugation, and norm — is exact, with no floating-point rounding.

## Features

- **Exact complex arithmetic** over the rationals
- **Division** producing non-dyadic coefficients (e.g., `1 / (1 + 1/2 i) = 4/5 - 2/5 i`)
- **Conjugate** and **norm** operations
- Full `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, and `Display` support

## Usage

```rust
use amari_surcomplex::RationalSurcomplex;
use amari_surreal::RationalSurreal;

let i = RationalSurcomplex::i();
assert_eq!(i.clone() * i, RationalSurcomplex::from_integer(-1));

let one = RationalSurreal::one();
let half = RationalSurreal::from_ratio(1, 2).unwrap();
let z = RationalSurcomplex::from_parts(one, half);
let q = RationalSurcomplex::one().checked_div(&z).unwrap();
assert_eq!(q.real().to_string(), "4/5");
assert_eq!(q.imag().to_string(), "-2/5");
```

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.
