const wasm = require("../dist/export.cjs");
const { readFileSync } = require("node:fs");
const wasmUrl = require("./wasm-url.cjs");

const wasmBytes = readFileSync(wasmUrl);

wasm.initSync({ module: wasmBytes });

module.exports = wasm;
