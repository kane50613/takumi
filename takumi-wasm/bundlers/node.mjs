import { readFileSync } from "node:fs";
import * as wasm from "../dist/export.mjs";

const wasmBytes = readFileSync(new URL("../pkg/takumi_wasm_bg.wasm", import.meta.url));

wasm.initSync({ module: wasmBytes });

export * from "../dist/export.mjs";
export default wasm.default;
