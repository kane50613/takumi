import { readFileSync, writeFileSync } from "node:fs";
import { defineConfig } from "tsdown";

const loaderOutputs = ["export.mjs", "export.cjs"];
// build:done fires per format; track each output across both fires so every
// shipped file is verified patched, not just one of them.
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
  deps: {
    neverBundle: ["csstype"],
  },
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
  return (
    content
      // Drop the unused __dirname shim; bundlers misread bare `new URL(.., import.meta.url)` as an asset.
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
      )
      .replaceAll(
        "Native module @takumi-rs/core-${target} is not being bunlded.",
        "Native module @takumi-rs/core-${target} is not being bundled.",
      )
  );
}
