import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    index: "src/index.ts",
    response: "src/response/index.ts",
    node: "src/node/index.ts",
    wasm: "src/wasm/index.ts",
    "helpers/index": "src/helpers/index.ts",
    "helpers/emoji": "src/helpers/emoji.ts",
    "helpers/jsx": "src/helpers/jsx.ts",
    "helpers/html": "src/helpers/html.ts",
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
