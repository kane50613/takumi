import { readFileSync, writeFileSync } from "node:fs";

const files = [
  new URL("../dist/export.mjs", import.meta.url),
  new URL("../dist/export.cjs", import.meta.url),
];

let hasPatchedFile = false;

for (const file of files) {
  const original = readFileSync(file, "utf8");
  const patched = patchGeneratedExportScript(original);

  if (patched === original) {
    continue;
  }

  writeFileSync(file, patched);
  hasPatchedFile = true;
}

if (!hasPatchedFile) {
  throw new Error("No generated export script changes were applied");
}

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
