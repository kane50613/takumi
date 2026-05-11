import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    export: "src/export.ts",
  },
  format: ["esm", "cjs"],
  dts: false, // https://github.com/rolldown/rolldown/pull/9197, https://github.com/rolldown/tsdown/issues/936
  clean: true,
  outDir: "dist",
  platform: "node",
  deps: {
    neverBundle: ["csstype"],
  },
});
