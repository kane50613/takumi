import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { initSync } from "../pkg/takumi_wasm";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const wasmPath = join(__dirname, "../pkg/takumi_wasm_bg.wasm");
const wasmBytes = readFileSync(wasmPath);

initSync(wasmBytes);

export * from "../pkg/takumi_wasm";
export { default } from "../pkg/takumi_wasm";
