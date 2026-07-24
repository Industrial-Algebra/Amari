import { initSync } from "@justinelliottcobb/amari-wasm";

export function initializeForNode(bytes: Buffer) {
  process.env.AMARI_RUNTIME = "node";
  return initSync({ module: bytes });
}
