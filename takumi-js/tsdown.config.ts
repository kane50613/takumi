import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    index: "src/index.ts",
    response: "src/response/index.ts",
    node: "src/node/index.ts",
    wasm: "src/wasm/index.ts",
    "wasm/no-init": "src/wasm/no-init.ts",
    "backend/node": "src/backend/node.ts",
    "backend/wasm": "src/backend/wasm.ts",
    "helpers/index": "src/helpers/index.ts",
    "helpers/emoji": "src/helpers/emoji.ts",
    "helpers/jsx": "src/helpers/jsx.ts",
    "helpers/html": "src/helpers/html.ts",
  },
  deps: {
    neverBundle: ["#backend"],
  },
  format: ["esm", "cjs"],
  dts: true,
  publint: {
    enabled: "ci-only",
    level: "error",
  },
  clean: true,
  outDir: "dist",
});
