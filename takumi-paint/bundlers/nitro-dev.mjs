import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import * as wasm from "../dist/export.mjs";

// Node loads this file from the package, but `noExternals` bundles it into Nitro's dev output, so resolve the binary by package name.
const wasmPath = createRequire(import.meta.url).resolve("takumi-paint/takumi_paint_wasm_bg.wasm");

wasm.initSync({ module: readFileSync(wasmPath) });

export * from "../dist/export.mjs";
export default wasm.default;
