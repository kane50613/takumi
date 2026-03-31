import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    index: "src/index.ts",
    emoji: "src/emoji.ts",
    "jsx/jsx": "src/jsx/jsx.ts",
  },
  format: ["esm", "cjs"],
  dts: true,
  deps: {
    onlyBundle: ["ultrahtml"],
  },
  minify: true,
  clean: true,
  outDir: "dist",
});
