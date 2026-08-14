import { readFileSync } from "node:fs";
import * as wasm from "../dist/export.mjs";
import wasmUrl from "./wasm-url.mjs";

const wasmBytes = readFileSync(wasmUrl);

wasm.initSync({ module: wasmBytes });

export * from "../dist/export.mjs";
export default wasm.default;
