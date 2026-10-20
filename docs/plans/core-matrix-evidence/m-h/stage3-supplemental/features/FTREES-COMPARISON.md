# Feature-resolved transitive activation comparison (§3.1, R3 completion)

## stable
alias-default vs canonical-default-equiv: DIFFER:
1c1
< amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [default,gf2,high-precision,phantom-types,std]
---
> amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [gf2,high-precision,phantom-types,std]
alias-full-precision vs canonical-fullp-equiv: DIFFER:
1c1
< amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [full-precision,high-precision,native-precision]
---
> amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [high-precision,native-precision]
alias-wasm-precision vs canonical-wasmp-equiv: DIFFER:
1c1
< amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [high-precision,wasm-precision]
---
> amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [high-precision]
alias-all-features vs canonical-max (maximal canonical row):
1c1
< amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [creusot-contracts,default,formal-verification,full-precision,gf2,high-precision,native-precision,parallel,phantom-types,rayon,serde,serialize,std,wasm-precision]
---
> amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [creusot-contracts,formal-verification,gf2,high-precision,native-precision,phantom-types,rayon,serde,std]

## nightly
alias-default vs canonical-default-equiv: DIFFER:
1c1
< amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [default,gf2,high-precision,phantom-types,std]
---
> amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [gf2,high-precision,phantom-types,std]
alias-full-precision vs canonical-fullp-equiv: DIFFER:
1c1
< amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [full-precision,high-precision,native-precision]
---
> amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [high-precision,native-precision]
alias-wasm-precision vs canonical-wasmp-equiv: DIFFER:
1c1
< amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [high-precision,wasm-precision]
---
> amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [high-precision]
alias-all-features vs canonical-max (maximal canonical row):
1c1
< amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [creusot-contracts,default,formal-verification,full-precision,gf2,high-precision,native-precision,parallel,phantom-types,rayon,serde,serialize,std,wasm-precision]
---
> amari-core v0.24.1 (/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix/amari-core) [creusot-contracts,formal-verification,gf2,high-precision,native-precision,phantom-types,rayon,serde,std]

## Closure mapping
closure-map.txt: every one of the 1,440 closed named sets (computed from the
manifest implication table: serialize->serde, parallel->rayon, full-precision->hp+np,
wasm-precision->hp, formal-verification->contracts+phantom) mapped to its canonical
row among the 320; the 8,192-subset enumeration and 1,440 count verified by computation.

## Root-label note (R3)

The alias-vs-canonical differences reported above are confined to the
ROOT package line's named-feature bracket: the alias rows include the
alias name itself (e.g. `[default,...]` vs `[...]`), which expands to
exactly the canonical set. Every DEPENDENCY line below the root —
package selection, versions, and each dependency's activated features —
is identical. The `alias-all-features` vs `canonical-max` diff is a
genuine superset difference (all-features additionally activates
serialize/parallel/wasm-precision/full-precision/default alias names,
pulling serde+rayon already present, and dashu/rug already present):
the MAXIMAL canonical row is strictly contained in all-features; the
closure map shows no canonical row equals all-features, as expected.
