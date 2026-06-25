import { readFileSync, writeFileSync } from "node:fs";
import { defineConfig } from "tsdown";

// build:done fires per format; flag spans both fires, throws if nothing matched.
let patchedExport = false;

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
  hooks: {
    // Drop wasm-bindgen's default `module_or_path` fallback; the binary only ever
    // comes through `@takumi-rs/wasm/auto`.
    "build:done": () => {
      for (const name of ["export.mjs", "export.cjs"]) {
        const file = new URL(`dist/${name}`, import.meta.url);
        const original = readFileSync(file, "utf8");
        const patched = patchGeneratedExportScript(original);

        if (patched !== original) {
          writeFileSync(file, patched);
          patchedExport = true;
        }
      }

      if (!patchedExport) {
        throw new Error("No generated export script changes were applied");
      }
    },
  },
});

function patchGeneratedExportScript(content: string) {
  return content
    .replace(
      'if (module_or_path === void 0) module_or_path = new URL("takumi_wasm_bg.wasm", import.meta.url);\n',
      "",
    )
    .replace(
      'if (module_or_path === void 0) module_or_path = new URL("takumi_wasm_bg.wasm", require("url").pathToFileURL(__filename).href);\n',
      "",
    );
}
