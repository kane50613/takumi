import { readFileSync, writeFileSync } from "node:fs";
import { defineConfig } from "tsdown";

// build:done fires per format; flag spans both fires, throws if nothing matched.
let patchedLoader = false;

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
      for (const name of ["export.mjs", "export.cjs"]) {
        const file = new URL(`dist/${name}`, import.meta.url);
        const original = readFileSync(file, "utf8");
        const patched = patchGeneratedLoader(original);

        if (patched !== original) {
          writeFileSync(file, patched);
          patchedLoader = true;
        }
      }

      if (!patchedLoader) {
        throw new Error("No generated loader changes were applied");
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
