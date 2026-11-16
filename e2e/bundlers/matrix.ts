export interface Cell {
  name: string;
  /** Directory the build runs in, relative to this folder. */
  cwd?: string;
  env?: Record<string, string>;
  build: string[];
  /** Build output, relative to `cwd`. Inspected for the backend and run from. */
  out: string;
  runtime: "node" | "bun" | "workerd" | "none";
  /** File inside `out` to run, relative to it. */
  entry?: string;
  /** `script` prints `magic <hex>`. `server` listens on `PORT` and serves a PNG at `path`. */
  mode?: "script" | "server";
  path?: string;
  /** Runs from a copy of `out` with no parent node_modules. Off for outputs that import their dependencies. */
  isolated: boolean;
  backend: "native" | "wasm";
  /** Why the cell fails. A cell that passes while it has one fails the job. */
  expectedFailure?: string;
}

const nitro = (preset: string, rest: Partial<Cell>): Cell => ({
  name: `nitro ${preset}`,
  cwd: "nitro",
  env: { NITRO_PRESET: preset },
  build: ["../node_modules/.bin/nitro", "build"],
  out: ".output",
  mode: "server",
  isolated: true,
  runtime: "node",
  backend: "wasm",
  ...rest,
});

export const cells: Cell[] = [
  {
    name: "wrangler",
    cwd: "wrangler",
    build: ["../node_modules/.bin/wrangler", "deploy", "--dry-run", "--outdir", "out"],
    out: "out",
    runtime: "workerd",
    entry: "worker.js",
    isolated: false,
    backend: "wasm",
  },
  {
    name: "@cloudflare/vite-plugin",
    cwd: "cloudflare-vite",
    build: ["../node_modules/.bin/vite", "build"],
    out: "out",
    runtime: "workerd",
    entry: "e2e/index.js",
    isolated: false,
    backend: "wasm",
  },
  {
    name: "vite ssr noExternal",
    build: ["node_modules/.bin/vite", "build", "-c", "vite.ssr.config.ts"],
    out: "out/vite-ssr",
    runtime: "node",
    entry: "node.js",
    isolated: false,
    backend: "native",
  },
  {
    name: "esbuild platform=node",
    build: [
      "node_modules/.bin/esbuild",
      "src/node.ts",
      "--bundle",
      "--platform=node",
      "--format=esm",
      "--outfile=out/esbuild/node.js",
      "--log-level=warning",
    ],
    out: "out/esbuild",
    runtime: "node",
    entry: "node.js",
    isolated: false,
    backend: "native",
  },
  {
    name: "bun build --target=node",
    build: ["bun", "build", "src/node.ts", "--target=node", "--outdir", "out/bun"],
    out: "out/bun",
    runtime: "node",
    entry: "node.js",
    isolated: false,
    backend: "native",
  },
  nitro("node-server", {
    entry: "server/index.mjs",
    expectedFailure: "unwasm inlines the wasm as base64 into a JS file over 3MB",
  }),
  nitro("node-server", {
    name: "nitro node-server !unwasm",
    env: { NITRO_PRESET: "node-server", NITRO_EXPORT_CONDITIONS: "!unwasm" },
    entry: "server/index.mjs",
    backend: "native",
  }),
  nitro("bun", {
    runtime: "bun",
    entry: "server/index.mjs",
    expectedFailure: "unwasm inlines the wasm as base64 into a JS file over 3MB",
  }),
  nitro("cloudflare-module", {
    runtime: "workerd",
    entry: "server/index.mjs",
  }),
  {
    name: "next turbopack standalone",
    cwd: "next-node",
    build: ["../node_modules/.bin/next", "build", "--turbopack"],
    out: ".next/standalone",
    runtime: "node",
    entry: "next-node/server.js",
    mode: "server",
    path: "/og",
    isolated: true,
    backend: "native",
  },
  {
    name: "next edge runtime",
    cwd: "next-edge",
    build: ["../node_modules/.bin/next", "build", "--turbopack"],
    out: ".next/server",
    runtime: "none",
    isolated: false,
    backend: "wasm",
  },
];
