import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    export: "src/export.ts",
  },
  format: ["esm", "cjs"],
  dts: true,
  publint: {
    enabled: "ci-only",
    level: "error",
  },
  clean: true,
  outDir: "dist",
  deps: {
    neverBundle: ["csstype"],
  },
  outputOptions: {
    exports: "named",
  },
});
