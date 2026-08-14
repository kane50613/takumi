import * as wasm from "../dist/export.mjs";
import wasmPath from "../pkg/takumi_wasm_bg.wasm";

const wasmBytes = await Bun.file(new URL(wasmPath, import.meta.url)).arrayBuffer();

wasm.initSync({ module: wasmBytes });

export * from "../dist/export.mjs";
export default wasm.default;
