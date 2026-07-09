import { fileURLToPath } from "node:url";
import { $ } from "bun";
import { beforeAll, describe, it } from "bun:test";

// Bun tolerates extensionless imports, so the Node entries are exercised in a real node process.
const cwd = fileURLToPath(new URL("..", import.meta.url));

const body = `const png = await new Renderer().render(container({ style: { width: 1, height: 1 }, children: [] }), { width: 1, height: 1, format: "png" });
if (!(png instanceof Uint8Array) || png.length === 0) throw new Error("empty render");`;

const esm = `import { Renderer } from "@takumi-rs/wasm/node";
import { Renderer as AutoRenderer } from "@takumi-rs/wasm/auto";
import { container } from "@takumi-rs/helpers";
if (typeof AutoRenderer !== "function") throw new Error("auto entry broken");
${body}`;

const cjs = `const { Renderer } = require("@takumi-rs/wasm/node");
const { Renderer: AutoRenderer } = require("@takumi-rs/wasm/auto");
const { container } = require("@takumi-rs/helpers");
if (typeof AutoRenderer !== "function") throw new Error("auto entry broken");
(async () => {
${body}
})();`;

describe("bundler entries resolve under Node", () => {
  beforeAll(async () => {
    await $`node -e ${'require("node:fs").readFileSync("pkg/takumi_wasm_bg.wasm")'}`
      .cwd(cwd)
      .quiet();
  });

  const cases: [type: "module" | "commonjs", script: string][] = [
    ["module", esm],
    ["commonjs", cjs],
  ];

  for (const [type, script] of cases) {
    it(`${type} node + auto entries`, async () => {
      await $`node --input-type=${type} -e ${script}`.cwd(cwd).quiet();
    }, 30_000);
  }
});
