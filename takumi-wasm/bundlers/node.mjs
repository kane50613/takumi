import { readFileSync } from "node:fs";
import { initSync } from "../pkg/takumi_wasm";

const wasmBytes = readFileSync(new URL("../pkg/takumi_wasm_bg.wasm", import.meta.url));

initSync({ module: wasmBytes });

export * from "../pkg/takumi_wasm";
export { default } from "../pkg/takumi_wasm";
