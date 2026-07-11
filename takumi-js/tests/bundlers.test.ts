import { afterAll, describe, expect, test } from "bun:test";
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

// The `#backend` import map must hand each bundler/runtime the right backend by
// condition alone: native napi on Node/Bun, WASM everywhere else, and the native
// `.node` binary must never reach a worker/edge bundle. We bundle a tiny consumer
// under each condition set and assert which backend got pulled in.
//
// Each case runs in its own `bun build` process. In-process Bun.build reuses the
// running process's module resolution, so once another test imports the package
// at runtime (resolving `#backend` to napi), Bun.build returns that cached backend
// regardless of `conditions`. A fresh process per case avoids that poisoning.
// Requires `bun run build` first — `#backend` resolves to ./dist/backend/*. The
// fixture lives inside the repo so module resolution reaches the workspace.

const dir = mkdtempSync(join(import.meta.dir, "..", "node_modules", ".bundler-fixture-"));
const entry = join(dir, "entry.ts");

// Reference `render` so tree-shaking (the package sets `sideEffects: false`)
// can't drop the backend import we're asserting on.
writeFileSync(entry, "import { render } from 'takumi-js';\nglobalThis.__keep = render;\n");

afterAll(() => rmSync(dir, { recursive: true, force: true }));

// Leave the backend packages as bare imports so we can assert on the specifier
// and skip the .wasm/.node loaders the host bundler would otherwise need.
const external = ["@takumi-rs/core", "@takumi-rs/wasm", "@takumi-rs/wasm/*"];

function bundle(opts: { target?: string; conditions?: string[] }): string {
  const args = ["build", entry];

  if (opts.target) {
    args.push(`--target=${opts.target}`);
  }

  for (const condition of opts.conditions ?? []) {
    args.push(`--conditions=${condition}`);
  }

  for (const pkg of external) {
    args.push(`--external=${pkg}`);
  }

  const result = Bun.spawnSync(["bun", ...args]);

  expect(result.exitCode).toBe(0);

  return result.stdout.toString();
}

/** Splitting build to a dir; returns the entry chunk and the other (lazy) chunks. */
function bundleSplit(opts: { target: string }): { entry: string; chunks: string } {
  const outdir = mkdtempSync(join(import.meta.dir, "..", "node_modules", ".bundler-out-"));
  const args = ["build", entry, `--target=${opts.target}`, "--splitting", `--outdir=${outdir}`];

  for (const pkg of external) {
    args.push(`--external=${pkg}`);
  }

  const result = Bun.spawnSync(["bun", ...args]);

  expect(result.exitCode).toBe(0);

  const entryChunk = readFileSync(join(outdir, "entry.js"), "utf8");
  const chunks = readdirSync(outdir)
    .filter((file) => file !== "entry.js")
    .map((file) => readFileSync(join(outdir, file), "utf8"))
    .join("\n");

  rmSync(outdir, { recursive: true, force: true });

  return { entry: entryChunk, chunks };
}

describe("#backend resolution by import condition", () => {
  // node/bun select napi as the primary backend; the WASM backend is still
  // reachable as the `module` escape hatch (a lazy chunk), so it may appear too.
  test("node → native core", () => {
    const code = bundle({ target: "node" });

    expect(code).toContain("@takumi-rs/core");
  });

  test("bun → native core", () => {
    const code = bundle({ target: "bun" });

    expect(code).toContain("@takumi-rs/core");
  });

  test("unwasm wins over node → WASM, never native core", () => {
    // Nitro sets both `unwasm` and `node` on its Node preset, so `unwasm` must
    // come first in the `#backend` map: its bundler can load a WASM binary, and
    // the native addon can't be counted on (WebContainer can't load one at all).
    const code = bundle({ target: "node", conditions: ["unwasm"] });

    expect(code).toContain("@takumi-rs/wasm/auto");
    expect(code).not.toContain("@takumi-rs/core");
  });

  test("workerd → WASM auto, never native core", () => {
    const code = bundle({
      target: "browser",
      conditions: ["workerd", "worker", "browser"],
    });

    expect(code).toContain("@takumi-rs/wasm/auto");
    expect(code).not.toContain("@takumi-rs/core");
  });

  test("edge-light → WASM, never native core", () => {
    // edge-light routes to the WASM backend (`@takumi-rs/wasm/auto`); the auto
    // entry's own conditions then pick the Next/Vercel edge loader downstream.
    const code = bundle({ target: "browser", conditions: ["edge-light"] });

    expect(code).toContain("@takumi-rs/wasm/auto");
    expect(code).not.toContain("@takumi-rs/core");
  });

  test("browser → WASM, never native core", () => {
    const code = bundle({ target: "browser" });

    expect(code).toContain("@takumi-rs/wasm");
    expect(code).not.toContain("@takumi-rs/core");
  });

  // The WASM backend is the `module` escape hatch, reached through a dynamic
  // import. On a node target it must stay out of the eager entry (napi only) and
  // live in a split chunk that loads only when a caller passes `module`.
  test("node WASM escape hatch is a lazy chunk, not in the eager entry", () => {
    const { entry: entryChunk, chunks } = bundleSplit({ target: "node" });

    expect(entryChunk).toContain("@takumi-rs/core");
    expect(entryChunk).not.toContain("@takumi-rs/wasm");
    expect(chunks).toContain("@takumi-rs/wasm");
  });
});
