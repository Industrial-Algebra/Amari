// Exercise the generated web-target bindings, not a Rust host mock.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const dir = resolve(process.argv[2] ?? 'amari-wasm/pkg');
const {default: init, WasmHolographicMemory: Memory} = await import(pathToFileURL(`${dir}/amari_wasm.js`));
await init({module_or_path: await readFile(`${dir}/amari_wasm_bg.wasm`)});
const repeat = (...block) => Float64Array.from(Array.from({length:32},()=>block).flat());
const key = repeat(2,1,0,0,0,0,0,0), value=repeat(0,0,1,0,0,0,0,0), unit=repeat(1,0,0,0,0,0,0,0);
for(const k of [key,unit]){
 const m=new Memory();m.store(k,value);const recovered=m.retrieve(k);assert.equal(recovered.length,256);
 recovered.forEach((x,i)=>assert.ok(Math.abs(x-value[i]/Math.sqrt(32))<1e-12));
 const singular=k.slice(), nan=k.slice(); singular.fill(0,248); singular[248]=singular[249]=1; nan[255]=NaN;
 for(const bad of [singular,nan,k.slice(1)]){
  for(const op of [()=>m.store(bad,value),()=>m.retrieve(bad),()=>m.retrieveConfidence(bad)]){
   assert.throws(op,e=>!(e instanceof WebAssembly.RuntimeError) && /key|coefficients/i.test(String(e)));
  }
  assert.equal(m.itemCount(),1);
 }
 m.store(k,new Float64Array(256)); assert.equal(m.itemCount(),2); m.free();
}
console.log('H1 WASM fusion: mixed/unit recall and key error cases passed');
