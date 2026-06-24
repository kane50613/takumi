import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    index: "src/index.ts",
    response: "src/response/index.ts",
    node: "src/node/index.ts",
    wasm: "src/wasm/index.ts",
    "backend/node": "src/backend/node.ts",
    "backend/wasm": "src/backend/wasm.ts",
    "backend/wasm-next": "src/backend/wasm-next.ts",
    "helpers/index": "src/helpers/index.ts",
    "helpers/emoji": "src/helpers/emoji.ts",
    "helpers/jsx": "src/helpers/jsx.ts",
    "helpers/html": "src/helpers/html.ts",
  },
  // Keep the `#backend` specifier in the output so the consumer's bundler/runtime
  // resolves it through the import conditions instead of us baking in one backend.
  external: ["#backend"],
  format: ["esm", "cjs"],
  dts: true,
  publint: {
    enabled: "ci-only",
    level: "error",
  },
  clean: true,
  outDir: "dist",
});
