import { expect, test } from "bun:test";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const dist = join(import.meta.dir, "../dist");

const loaders = [
  {
    format: "esm",
    load: `const { render } = await import(${JSON.stringify(pathToFileURL(join(dist, "index.mjs")).href)});
const { readFile } = await import("node:fs/promises");
const { default: wasmUrl } = await import("takumi-js/wasm-url");`,
  },
  {
    format: "cjs",
    load: `const { render } = require(${JSON.stringify(join(dist, "index.cjs"))});
const { readFile } = require("node:fs/promises");
const wasmUrl = require("takumi-js/wasm-url");`,
  },
];

// A WebContainer cannot load the native addon, so the node backend renders through the WASM
// binary it reads on demand. A `module` picks the WASM backend outright.
const runtimes = [
  {
    runtime: "a WebContainer",
    setup: `Object.defineProperty(process.versions, "webcontainer", { value: "1" });`,
    options: "",
  },
  { runtime: "a module", setup: "", options: "module: readFile(wasmUrl)," },
];

// Prints the PNG signature and whether the process loaded a native addon.
const renderSquare = (options: string) => `const dlopen = process.dlopen;
let native = false;
process.dlopen = (...args) => {
  native = true;
  return dlopen.apply(process, args);
};
render(
  { type: "container", style: { width: 8, height: 8, backgroundColor: "red" }, children: [] },
  { width: 8, height: 8, ${options} },
).then((png) => process.stdout.write(Buffer.from(png.subarray(1, 4)).toString() + " native=" + native))`;

// Runs under Node, after `bun run build`.
test.each(runtimes.flatMap((runtime) => loaders.map((loader) => ({ ...runtime, ...loader }))))(
  "renders through WASM under $runtime ($format)",
  ({ setup, options, format, load }) => {
    const result = Bun.spawnSync(
      [
        "node",
        `--input-type=${format === "esm" ? "module" : "commonjs"}`,
        "-e",
        `${setup}\n${load}\n${renderSquare(options)};`,
      ],
      { cwd: join(import.meta.dir, "..") },
    );

    expect(result.stderr.toString()).toBe("");
    expect(result.stdout.toString()).toBe("PNG native=false");
  },
);
