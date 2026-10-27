import { $ } from "bun";

const BINARYEN_VERSION = "133";

const flags = [
  "-Oz",
  "--converge",
  "--code-folding",
  // https://github.com/rust-lang/rust/issues/93294
  "--enable-bulk-memory",
  "--enable-mutable-globals",
  "--enable-sign-ext",
  "--strip-debug",
  "--strip-dwarf",
  "--strip-producers",
  "--strip-target-features",
];

const [wasmFile, ...extraFlags] = process.argv.slice(2);

if (!wasmFile) {
  throw new Error("usage: bun scripts/wasm-opt.mts <file.wasm> [extra wasm-opt flags]");
}

const version = await $`wasm-opt --version`.nothrow().quiet();

if (!version.stdout.toString().includes(`version ${BINARYEN_VERSION}`)) {
  throw new Error(
    `wasm-opt from binaryen version_${BINARYEN_VERSION} is required on PATH ` +
      `(https://github.com/WebAssembly/binaryen/releases/tag/version_${BINARYEN_VERSION}).`,
  );
}

await $`wasm-opt ${flags} ${extraFlags} ${wasmFile} -o ${wasmFile}`;
