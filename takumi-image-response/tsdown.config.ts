import { defineConfig, type UserConfig } from "tsdown";

const config: UserConfig = defineConfig({
  entry: {
    index: "src/index.ts",
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

export default config;
