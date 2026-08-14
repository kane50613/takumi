import * as wasm from "../dist/export.mjs";
import wasmUrl from "./wasm-url.mjs";

const wasmBytes = await Bun.file(wasmUrl).arrayBuffer();

wasm.initSync({ module: wasmBytes });

export * from "../dist/export.mjs";
export default wasm.default;
