import { spawnSync } from "node:child_process";
import { describe, expect, it } from "bun:test";

// These entries must resolve under plain Node ESM, where nothing rewrites
// extensionless/bare imports. bun:test runs under Bun, which tolerates them, so
// the only way to guard the Node path is to exercise it in a real `node` process.
const nodeEntries = ["@takumi-rs/wasm/node", "@takumi-rs/wasm/auto"];

const cwd = new URL("..", import.meta.url).pathname;

function loadAndRender(specifier: string) {
  const script = `
import { Renderer } from ${JSON.stringify(specifier)};
import { container } from "@takumi-rs/helpers";
const renderer = new Renderer();
const png = await renderer.render(container({ style: { width: 1, height: 1 }, children: [] }), {
  width: 1,
  height: 1,
  format: "png",
});
renderer.free();
if (!(png instanceof Uint8Array) || png.length === 0) {
  throw new Error("empty render output");
}
`;

  return spawnSync("node", ["--input-type=module", "-e", script], { cwd, encoding: "utf8" });
}

describe("bundler entries resolve under Node ESM", () => {
  for (const entry of nodeEntries) {
    it(`${entry} imports + renders in a node process`, () => {
      const { status, stderr } = loadAndRender(entry);

      if (status !== 0) {
        throw new Error(`node failed for ${entry} (exit ${status}):\n${stderr}`);
      }
    });
  }
});
