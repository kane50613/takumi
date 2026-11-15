import { expect, test } from "bun:test";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const renderSquare = `render(
  { type: "container", style: { width: 8, height: 8, backgroundColor: "red" }, children: [] },
  { width: 8, height: 8 },
).then((png) => process.stdout.write(Buffer.from(png.subarray(1, 4)).toString()))`;

// A WebContainer cannot load the native addon, so the node backend must render
// through the WASM binary it reads on demand. Runs under Node, which `#backend`
// resolves to the node backend, after `bun run build`.
test.each([
  {
    format: "esm",
    script: `const { render } = await import(${JSON.stringify(pathToFileURL(join(import.meta.dir, "../dist/index.mjs")).href)});\n${renderSquare};`,
  },
  {
    format: "cjs",
    script: `const { render } = require(${JSON.stringify(join(import.meta.dir, "../dist/index.cjs"))});\n${renderSquare};`,
  },
])(
  "renders through the WASM fallback when process.versions.webcontainer is set ($format)",
  ({ format, script }) => {
    const result = Bun.spawnSync([
      "node",
      `--input-type=${format === "esm" ? "module" : "commonjs"}`,
      "-e",
      `Object.defineProperty(process.versions, "webcontainer", { value: "1" });\n${script}`,
    ]);

    expect(result.stderr.toString()).toBe("");
    expect(result.stdout.toString()).toBe("PNG");
  },
);
