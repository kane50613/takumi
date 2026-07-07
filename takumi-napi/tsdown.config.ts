import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { defineConfig } from "tsdown";

const loaderOutputs = ["export.mjs", "export.cjs"];

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

        if (!existsSync(file)) continue;

        const original = readFileSync(file, "utf8");

        if (original.includes("function loadNativePackage")) continue;

        const patched = patchGeneratedLoader(original, name.endsWith(".mjs") ? "esm" : "cjs");

        if (!patched.includes("loadNativePackage('@takumi-rs/core-")) {
          throw new Error(
            `Loader patch did not rewrite the native binding in ${name}; napi output shape changed`,
          );
        }

        writeFileSync(file, patched);
      }
    },
  },
});

function patchGeneratedLoader(content: string, format: "esm" | "cjs") {
  const startDirCheck =
    format === "esm"
      ? `try { if (import.meta?.url) dirs.push(path.dirname(new URL(import.meta.url).pathname)); } catch (e) {}`
      : `try { if (typeof __dirname !== 'undefined') dirs.push(__dirname); } catch (e) {}`;

  const helpers = `
function resolveIsolated(pkgName, binaryName, localRequire) {
  try {
    const fs = localRequire('fs'), path = localRequire('path');
    const dirs = [process.cwd()];
    ${startDirCheck}
    const storePrefix = pkgName.replace('/', '+') + '@';
    for (let dir of dirs) {
      while (dir) {
        const nm = path.join(dir, 'node_modules');
        if (fs.existsSync(nm)) {
          const direct = path.join(nm, pkgName, binaryName);
          if (fs.existsSync(direct)) return direct;
          for (const storeName of ['.pnpm', '.bun']) {
            const storeDir = path.join(nm, storeName);
            if (fs.existsSync(storeDir)) {
              for (const f of fs.readdirSync(storeDir)) {
                if (f.startsWith(storePrefix)) {
                  const p = path.join(storeDir, f, 'node_modules', pkgName, binaryName);
                  if (fs.existsSync(p)) return p;
                }
              }
            }
          }
        }
        const parent = path.dirname(dir);
        if (parent === dir) break;
        dir = parent;
      }
    }
  } catch (e) {}
  return null;
}

function loadNativePackage(pkgName, binaryName, localRequire) {
  try {
    return { binding: localRequire(pkgName + '/' + binaryName), version: localRequire(pkgName + '/package.json').version };
  } catch (e) {
    const resolved = resolveIsolated(pkgName, binaryName, localRequire);
    if (resolved) {
      const path = localRequire('path');
      return { binding: localRequire(resolved), version: localRequire(path.join(path.dirname(resolved), 'package.json')).version };
    }
    throw e;
  }
}
`;

  return (
    content
      // Drop the unused __dirname shim; bundlers misread bare \`new URL(.., import.meta.url)\` as an asset.
      .replaceAll(/^[^\n]*new URL\((["'])\.\1,\s*import\.meta\.url\)\.pathname;?[^\n]*\n?/gm, "")
      .replaceAll("process.env.NAPI_RS_NATIVE_LIBRARY_PATH", "process.env.TAKUMI_CORE_TARGET")
      .replaceAll(/(["'])\.\/core(\.[^"']+\.node["'])/g, "$1../core$2")
      .replaceAll(
        /const binding = (require(?:\$\d+)?)\((?:\/\*[^*]*\*\/)?\s*(['"])(@takumi-rs\/core-([^'"]+))\2\);?\s*const bindingPackageVersion = \1\((?:\/\*[^*]*\*\/)?\s*\2\3\/package\.json\2\)\.version/g,
        "const { binding, version: bindingPackageVersion } = loadNativePackage('$3', 'core.$4.node', $1)",
      )
      .replaceAll(
        /(require(?:\$1)?)\((?!\/\* turbopackOptional: true \*\/ )/g,
        "$1(/* turbopackOptional: true */ ",
      ) + helpers
  );
}
