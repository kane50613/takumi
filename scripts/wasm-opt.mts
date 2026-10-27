import { $ } from "bun";

const BINARYEN_VERSION = "133";

const flags = [
  "-Oz",
  "--converge",
  "--code-folding",
  "--strip-debug",
  "--strip-dwarf",
  "--strip-producers",
  "--strip-target-features",
];

const [wasmFile] = process.argv.slice(2);

if (!wasmFile) {
  throw new Error("usage: bun scripts/wasm-opt.mts <file.wasm>");
}

const version = await $`wasm-opt --version`.nothrow().quiet();

if (!version.stdout.toString().includes(`version ${BINARYEN_VERSION}`)) {
  throw new Error(
    `wasm-opt from binaryen version_${BINARYEN_VERSION} is required on PATH ` +
      `(https://github.com/WebAssembly/binaryen/releases/tag/version_${BINARYEN_VERSION}).`,
  );
}

await $`wasm-opt ${flags} ${wasmFile} -o ${wasmFile}`;
