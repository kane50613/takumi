import { fileURLToPath } from "node:url";
import { $ } from "bun";
import { describe, it } from "bun:test";

// Bun tolerates extensionless imports, so the Node entries are exercised in a real node process.
const cwd = fileURLToPath(new URL("..", import.meta.url));

const body = `const png = await new Renderer().render(container({ style: { width: 1, height: 1 }, children: [] }), { width: 1, height: 1, format: "png" });
if (!(png instanceof Uint8Array) || png.length === 0) throw new Error("empty render");`;

const esm = (entry: string) =>
  `import { Renderer } from "${entry}";\nimport { container } from "@takumi-rs/helpers";\n${body}`;

const cjs = (entry: string) =>
  `const { Renderer } = require("${entry}");\nconst { container } = require("@takumi-rs/helpers");\n(async () => {\n${body}\n})();`;

const cases: [type: "module" | "commonjs", entry: string, script: string][] = [
  ["module", "@takumi-rs/wasm/node", esm("@takumi-rs/wasm/node")],
  ["module", "@takumi-rs/wasm/auto", esm("@takumi-rs/wasm/auto")],
  ["commonjs", "@takumi-rs/wasm/node", cjs("@takumi-rs/wasm/node")],
  ["commonjs", "@takumi-rs/wasm/auto", cjs("@takumi-rs/wasm/auto")],
];

describe("bundler entries resolve under Node", () => {
  for (const [type, entry, script] of cases) {
    it(`${type} ${entry}`, async () => {
      await $`node --input-type=${type} -e ${script}`.cwd(cwd).quiet();
    });
  }
});
