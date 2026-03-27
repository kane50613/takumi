import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    export: "src/export.ts",
  },
  format: ["esm", "cjs"],
  dts: true,
  clean: true,
  outDir: "dist",
  deps: {
    neverBundle: ["csstype"],
  },
});
