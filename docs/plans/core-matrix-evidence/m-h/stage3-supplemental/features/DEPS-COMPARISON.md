# Dependency-activation comparison (§3.1): alias rows vs canonical equivalents
# cargo tree -e normal --prefix none --depth 1, sorted, self+dev filtered below

## stable
alias-default ≡ canonical-default-equiv: DEP SETS IDENTICAL
alias-full-precision ≡ canonical-fullp-equiv: DEP SETS IDENTICAL
alias-wasm-precision ≡ canonical-wasmp-equiv: DEP SETS IDENTICAL
alias-all-features dep set (the maximal closure, no canonical equivalent required):
approx v0.5.1
bytemuck v1.25.2
creusot-contracts v0.8.0
dashu-float v0.4.5
num-traits v0.2.19
rayon v1.12.0
rug v1.30.0
serde v1.0.229
thiserror v2.0.21

## nightly
alias-default ≡ canonical-default-equiv: DEP SETS IDENTICAL
alias-full-precision ≡ canonical-fullp-equiv: DEP SETS IDENTICAL
alias-wasm-precision ≡ canonical-wasmp-equiv: DEP SETS IDENTICAL
alias-all-features dep set (the maximal closure, no canonical equivalent required):
approx v0.5.1
bytemuck v1.25.2
creusot-contracts v0.8.0
dashu-float v0.4.5
num-traits v0.2.19
rayon v1.12.0
rug v1.30.0
serde v1.0.229
thiserror v2.0.21

## Public precision type (§3.1 comparison, third leg)

The precision-related public exports are gated purely on feature
predicates (amari-core/src/lib.rs): `PrecisionFloat`/`StandardFloat`
unconditional; `ExtendedFloat` under
`any(high-precision, wasm-precision, native-precision)`. The
feature-sets comparison above shows each alias row's resolved feature
set equals its canonical equivalent's (modulo the alias name itself,
which expands to that same set). Equal feature sets + equal dependency
activation => equal cfg predicate values => equal public precision
surface, by construction. The maximal closure (all-features) activates
dashu-float + rug + both, exposing the full ExtendedFloat surface.
