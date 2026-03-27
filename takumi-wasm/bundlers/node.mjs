import { readFileSync } from "node:fs";
import * as wasm from "../dist/export";

const wasmBytes = readFileSync(new URL("../pkg/takumi_wasm_bg.wasm", import.meta.url));

wasm.initSync({ module: wasmBytes });

export * from "../dist/export";
export default wasm.default;
