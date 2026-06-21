import { readFileSync, writeFileSync } from "node:fs";

const filesToPatch = ["dist/export.mjs", "dist/export.cjs", "index.js"];
let patchedFileCount = 0;

for (const file of filesToPatch) {
  const original = readFileSync(new URL(`../${file}`, import.meta.url), "utf8");
  const patched = patchGeneratedLoader(original, {
    nodePrefix: file.startsWith("dist/") ? "../core" : "./core",
  });

  if (patched === original) {
    continue;
  }

  writeFileSync(new URL(`../${file}`, import.meta.url), patched);
  patchedFileCount += 1;
}

if (patchedFileCount === 0) {
  throw new Error("No generated loader changes were applied");
}

function patchGeneratedLoader(
  content: string,
  options: {
    nodePrefix: string;
  },
) {
  return content
    .replaceAll("process.env.NAPI_RS_NATIVE_LIBRARY_PATH", "process.env.TAKUMI_CORE_TARGET")
    .replaceAll(/(["'])\.\/core(\.[^"']+\.node["'])/g, `$1${options.nodePrefix}$2`)
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
    );
}
