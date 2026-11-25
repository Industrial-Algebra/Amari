# Holographic memory and cleanup in WASM

`WasmHolographicMemory` uses 32 ordered Clifford factors (256 finite coefficients). Keys must pass the two-sided inverse check in every factor. Values need not be invertible. Storage adds bindings; recall returns raw coefficients, not a normalized display value.

```js
import init, { WasmHolographicMemory, WasmResonator } from '@justinelliottcobb/amari-wasm';
await init();
const memory = WasmHolographicMemory.withKeyTracking();
const key = Float64Array.from({length:256}, (_, i) => +(i % 8 === 0));
const value = Float64Array.from({length:256}, (_, i) => +(i % 8 === 1));
memory.store(key, value);
const raw = memory.retrieve(key);
const evidence = memory.membershipEvidence(key); // {kind:'present', indices:[0]}
const score = memory.nearestKeySimilarity(key); // separate readout
const book = new WasmResonator(value); // flattened array of 256-coefficient records
const nearest = book.cleanupNearest(raw);
const weighted = book.cleanupWeighted(raw, 10, 1, 100);
console.log(nearest.status, weighted.status, weighted.accepted);
book.free(); memory.free();
```

Untracked membership is `unknown`, not zero confidence. A tracked miss is `absent`; nearest similarity can still be high. Invalid keys/inputs throw. Nominal capacity and SNR are load heuristics, not query evidence. In a new H3 data namespace replay original ordered associations: old attention traces cannot be rescaled to additive traces. See `scripts/test-h3-wasm.mjs` for the executable binding checks.
