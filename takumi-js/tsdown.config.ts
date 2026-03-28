import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    index: "src/index.ts",
    response: "src/response/index.ts",
    node: "src/node/index.js",
    wasm: "src/wasm/index.js",
    "helpers/index": "src/helpers/index.ts",
    "helpers/emoji": "src/helpers/emoji.ts",
    "helpers/jsx": "src/helpers/jsx.ts",
  },
  format: ["esm", "cjs"],
  dts: true,
  clean: true,
  outDir: "dist",
});
