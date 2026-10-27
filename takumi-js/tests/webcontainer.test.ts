import { expect, test } from "bun:test";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

// A WebContainer cannot load the native addon, so the node backend must render
// through the WASM binary it reads on demand. Runs under Node, which `#backend`
// resolves to the node backend, after `bun run build`.
test("renders through the WASM fallback when process.versions.webcontainer is set", () => {
  const script = `
    Object.defineProperty(process.versions, "webcontainer", { value: "1" });
    const { render } = await import(${JSON.stringify(pathToFileURL(join(import.meta.dir, "../dist/index.mjs")).href)});
    const png = await render(
      { type: "container", style: { width: 8, height: 8, backgroundColor: "red" }, children: [] },
      { width: 8, height: 8 },
    );
    process.stdout.write(Buffer.from(png.subarray(1, 4)).toString());
  `;
  const result = Bun.spawnSync(["node", "--input-type=module", "-e", script]);

  expect(result.stderr.toString()).toBe("");
  expect(result.stdout.toString()).toBe("PNG");
});
