import { readFileSync, writeFileSync } from "node:fs";

const file = new URL("../pkg/takumi_wasm.js", import.meta.url);
const original = readFileSync(file, "utf8");
const patched = patchGeneratedLoader(original);

if (patched === original) {
  throw new Error("No generated loader changes were applied");
}

writeFileSync(file, patched);

function patchGeneratedLoader(content: string) {
  return content.replaceAll(
    "new URL('takumi_wasm_bg.wasm', import.meta.url)",
    "new URL(/* @vite-ignore */ 'takumi_wasm_bg.wasm', import.meta.url)",
  );
}
