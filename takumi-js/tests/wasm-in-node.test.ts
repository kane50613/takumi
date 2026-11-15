import { expect, test } from "bun:test";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const loaders = [
  {
    format: "esm",
    load: `const { render } = await import(${JSON.stringify(pathToFileURL(join(import.meta.dir, "../dist/index.mjs")).href)});`,
  },
  {
    format: "cjs",
    load: `const { render } = require(${JSON.stringify(join(import.meta.dir, "../dist/index.cjs"))});`,
  },
];

// A WebContainer cannot load the native addon, so the node backend renders through the WASM
// binary it reads on demand. The `takumi-wasm` condition picks the WASM backend outright.
const runtimes = [
  {
    runtime: "a WebContainer",
    flags: [],
    setup: `Object.defineProperty(process.versions, "webcontainer", { value: "1" });`,
  },
  { runtime: "the takumi-wasm condition", flags: ["--conditions=takumi-wasm"], setup: "" },
];

// Prints the PNG signature and whether the process loaded a native addon.
const renderSquare = `const dlopen = process.dlopen;
let native = false;
process.dlopen = (...args) => {
  native = true;
  return dlopen.apply(process, args);
};
render(
  { type: "container", style: { width: 8, height: 8, backgroundColor: "red" }, children: [] },
  { width: 8, height: 8 },
).then((png) => process.stdout.write(Buffer.from(png.subarray(1, 4)).toString() + " native=" + native))`;

// Runs under Node, after `bun run build`.
test.each(runtimes.flatMap((runtime) => loaders.map((loader) => ({ ...runtime, ...loader }))))(
  "renders through WASM under $runtime ($format)",
  ({ flags, setup, format, load }) => {
    const result = Bun.spawnSync([
      "node",
      ...flags,
      `--input-type=${format === "esm" ? "module" : "commonjs"}`,
      "-e",
      `${setup}\n${load}\n${renderSquare};`,
    ]);

    expect(result.stderr.toString()).toBe("");
    expect(result.stdout.toString()).toBe("PNG native=false");
  },
);
