// Browser-side Amari WASM integration.
import {
  WasmMultivector300 as Multivector,
  WasmRotor300,
} from "@justinelliottcobb/amari-wasm";
import * as amari from "@justinelliottcobb/amari-wasm";

export async function loadAmari() {
  const lazy = await import("@justinelliottcobb/amari-wasm");
  const vector = new Multivector();
  window.requestAnimationFrame(() => WebAssembly.validate(new Uint8Array()));
  return { amari, lazy, vector, rotor: WasmRotor300 };
}
