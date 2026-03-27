import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    index: "src/index.ts",
    "response/index": "src/response/index.ts",
    "node/index": "src/node/index.js",
    "wasm/index": "src/wasm/index.js",
    "helpers/index": "src/helpers/index.mjs",
    "helpers/emoji/index": "src/helpers/emoji/index.mjs",
    "helpers/jsx/index": "src/helpers/jsx/index.mjs",
  },
  format: ["esm", "cjs"],
  dts: true,
  clean: true,
  outDir: "dist",
});
