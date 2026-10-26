#!/usr/bin/env python3
"""Portable capability closure for P-WASM (R2: 8 dimensions, 160 canonical sets).

Manifest-derived dimension space: the 7 named portable features PLUS the
creusot-contracts optional-dependency capability — `formal-verification =
["creusot-contracts", "phantom-types"]` has no `dep:` prefix, so cargo
exposes an implicit CLI-addressable feature activating the crate alone
(R1's 96-set space missed the 64 C-without-fv states). serde/rayon are
normalized aliases of serialize/parallel (single compile-capability each);
wasm-precision ≡ high-precision; native-precision/full-precision excluded
(rug native-only). Closure edge: formal-verification -> {creusot-contracts,
phantom-types}. Cross-checks the manifest so drift fails closed.
"""
import itertools, re, sys
from pathlib import Path

root = Path(__file__).resolve()
for _ in range(6): root = root.parent  # evidence/p-wasm -> ... -> worktree root
if not (root / 'amari-core' / 'Cargo.toml').exists():
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.cwd().resolve()
manifest = root / 'amari-core' / 'Cargo.toml'
m = manifest.read_text()
checks = [
    (r'formal-verification\s*=\s*\["creusot-contracts",\s*"phantom-types"\]', 'fv edge'),
    (r'serialize\s*=\s*\["serde"\]', 'serialize edge (serde normalized)'),
    (r'parallel\s*=\s*\["rayon"\]', 'parallel edge (rayon normalized)'),
    (r'wasm-precision\s*=\s*\["high-precision"\]', 'wasm alias'),
]
for pat, why in checks:
    if not re.search(pat, m):
        sys.exit(f'MANIFEST DRIFT: {why} no longer matches ({manifest})')

dims = ["std", "phantom-types", "high-precision", "gf2", "serialize", "parallel", "formal-verification", "creusot-contracts"]
implies = {"formal-verification": {"creusot-contracts", "phantom-types"}}
closed = {}
for r in range(len(dims) + 1):
    for sub in itertools.combinations(sorted(dims), r):
        closure = set(sub)
        for f in sub:
            closure |= implies.get(f, set())
        if closure == set(sub):
            closed[frozenset(sub)] = sub
rows = sorted(closed.values(), key=lambda s: (len(s), s))
print(f'# canonical portable capability sets: {len(rows)} (expected 160; got {len(rows)})', file=sys.stderr)
if len(rows) != 160:
    sys.exit('INVARIANT BROKEN: closure is not 160 sets — manifest semantics changed')
for i, r in enumerate(rows, 1):
    print(f"{i}\t{','.join(r) if r else '(minimal)'}")
