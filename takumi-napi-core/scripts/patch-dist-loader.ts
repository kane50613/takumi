import { readFileSync, writeFileSync } from "node:fs";

const filesToPatch = ["dist/export.mjs", "dist/export.cjs", "index.js"];
let patchedFileCount = 0;

for (const file of filesToPatch) {
  const original = readFileSync(new URL(`../${file}`, import.meta.url), "utf8");
  const patched = patchGeneratedLoader(original);

  if (patched === original) {
    continue;
  }

  writeFileSync(new URL(`../${file}`, import.meta.url), patched);
  patchedFileCount += 1;
}

if (patchedFileCount === 0) {
  throw new Error("No generated loader changes were applied");
}

function patchGeneratedLoader(content: string) {
  return content
    .replaceAll("process.env.NAPI_RS_NATIVE_LIBRARY_PATH", "process.env.TAKUMI_CORE_TARGET")
    .replaceAll(
      /return require(\$1)?\(("(?:\.?\.\/core\.[^"]+\.node|@takumi-rs\/core-[^"]+)")\);/g,
      "return require$1(/* turbopackOptional: true */ $2);",
    )
    .replace(
      "Native module @takumi-rs/core-${target} is not being bunlded.",
      "Native module @takumi-rs/core-${target} is not being bundled.",
    );
}
