const wasm = require("../pkg/takumi_wasm.cjs");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");

const wasmPath = join(__dirname, "../pkg/takumi_wasm_bg.wasm");
const wasmBytes = readFileSync(wasmPath);

wasm.initSync({ module: wasmBytes });

module.exports = wasm;
