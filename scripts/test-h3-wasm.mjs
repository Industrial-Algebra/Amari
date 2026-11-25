// Public JS API: no host mocks; compare returned coefficients, not just scores.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
const dir = resolve(process.argv[2] ?? 'amari-wasm/pkg');
const { default: init, WasmHolographicMemory: Memory, WasmResonator: Resonator } =
  await import(pathToFileURL(`${dir}/amari_wasm.js`));
await init({module_or_path: await readFile(`${dir}/amari_wasm_bg.wasm`)});
const blade = (index) => Float64Array.from({length:256}, (_, i) => +(i % 8 === index));
const one=blade(0), e1=blade(1), e2=blade(2);
const tracked=Memory.withKeyTracking(), untracked=new Memory();
tracked.store(one,e1); tracked.store(one,e2);
assert.deepEqual([...tracked.retrieve(one)], [...e1.map((v,i)=>v+e2[i])], 'raw additive recall');
assert.deepEqual(tracked.membershipEvidence(one), {kind:'present',indices:[0,1]});
assert.deepEqual(tracked.membershipEvidence(e2), {kind:'absent'});
assert.equal(tracked.nearestKeySimilarity(e2),0);
untracked.store(one,e1);
assert.deepEqual(untracked.membershipEvidence(one),{kind:'unknown'});
assert.equal(untracked.nearestKeySimilarity(one),undefined);
const bad = one.slice(); bad.fill(0,248);
for(const f of [()=>tracked.store(bad,e1),()=>tracked.retrieve(bad),()=>tracked.membershipEvidence(bad)]) {
  assert.throws(f,e=>!(e instanceof WebAssembly.RuntimeError) && /key|singular|invert/i.test(String(e)));
}
assert.equal(tracked.itemCount(),2);
const book=new Resonator(new Float64Array([...e1,...e2]));
assert.equal(book.cleanupNearest(e1).status,'selected');
assert.equal(book.cleanupNearest(one).status,'no_acceptable_candidate');
const update=book.cleanupWeighted(e1,1,Math.log(3),Math.log(3));
assert.deepEqual(update.status,'budget_exhausted');
assert.equal(update.accepted,false);
for(const [i,c] of update.candidate.entries()) assert.ok(Math.abs(c-(0.75*e1[i]+0.25*e2[i]))<1e-12);
assert.throws(()=>book.cleanupNearest(new Float64Array(1)),e=>!(e instanceof WebAssembly.RuntimeError));
tracked.free(); untracked.free(); book.free();
console.log('H3 Node: additive recall, typed evidence, rejection and both cleanup modes passed');
