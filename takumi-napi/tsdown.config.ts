import { readFileSync, writeFileSync } from "node:fs";
import { defineConfig } from "tsdown";

const loaderOutputs = ["export.mjs", "export.cjs"];
const patchedOutputs = new Set<string>();

export default defineConfig({
  entry: {
    export: "src/export.ts",
  },
  format: ["esm", "cjs"],
  dts: true,
  clean: true,
  outDir: "dist",
  platform: "node",
  hooks: {
    // Make NAPI-RS's generated loader bundler-safe. Only `dist/export.*` ships.
    "build:done": () => {
      for (const name of loaderOutputs) {
        const file = new URL(`dist/${name}`, import.meta.url);
        const original = readFileSync(file, "utf8");
        const patched = patchGeneratedLoader(original);

        if (patched !== original) {
          writeFileSync(file, patched);
          patchedOutputs.add(name);
        }
      }

      const unpatched = loaderOutputs.filter((name) => !patchedOutputs.has(name));

      if (unpatched.length > 0) {
        throw new Error(`Generated loader patch matched nothing in: ${unpatched.join(", ")}`);
      }
    },
  },
});

function patchGeneratedLoader(content: string) {
  return content
    .replaceAll(/^[^\n]*new URL\((["'])\.\1,\s*import\.meta\.url\)\.pathname;?[^\n]*\n?/gm, "")
    .replaceAll("process.env.NAPI_RS_NATIVE_LIBRARY_PATH", "process.env.TAKUMI_CORE_TARGET")
    .replaceAll(/(["'])\.\/core(\.[^"']+\.node["'])/g, "$1../core$2")
    .replaceAll(
      /(require(?:\$1)?)\((['"])@takumi-rs\/core-([^/"']+)\2\)/g,
      "$1($2@takumi-rs/core-$3/core.$3.node$2)",
    )
    .replaceAll(
      /(require(?:\$1)?)\((?!\/\* turbopackOptional: true \*\/ )/g,
      "$1(/* turbopackOptional: true */ ",
    );
}
