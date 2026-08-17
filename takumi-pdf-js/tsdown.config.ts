import { readFileSync, writeFileSync } from "node:fs";
import { defineConfig } from "tsdown";

const exportOutputs = ["export.mjs", "export.cjs"];

export default defineConfig({
  entry: {
    export: "src/export.ts",
    primitives: "src/primitives.ts",
  },
  format: ["esm", "cjs"],
  dts: true,
  publint: {
    enabled: "ci-only",
    level: "error",
  },
  clean: true,
  outDir: "dist",
  outputOptions: {
    exports: "named",
  },
  hooks: {
    // Drop wasm-bindgen's default `module_or_path` fallback; the binary only
    // ever comes through the bundler entries. Verify every shipped output.
    "build:done": () => {
      for (const name of exportOutputs) {
        const file = new URL(`dist/${name}`, import.meta.url);
        const patched = patchGeneratedExportScript(readFileSync(file, "utf8"));
        writeFileSync(file, patched);

        if (patched.includes('new URL("takumi_pdf_wasm_bg.wasm"')) {
          throw new Error(`module_or_path fallback not removed from dist/${name}`);
        }
      }
    },
  },
});

function patchGeneratedExportScript(content: string) {
  return content
    .replace(
      'if (module_or_path === void 0) module_or_path = new URL("takumi_pdf_wasm_bg.wasm", import.meta.url);\n',
      "",
    )
    .replace(
      'if (module_or_path === void 0) module_or_path = new URL("takumi_pdf_wasm_bg.wasm", require("url").pathToFileURL(__filename).href);\n',
      "",
    );
}
