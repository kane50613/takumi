import { readFileSync, writeFileSync } from "node:fs";
import { defineConfig, type UserConfig } from "tsdown";

const loaderOutputs = ["export.mjs", "export.cjs"];
const patchedOutputs = new Set<string>();

/** NAPI-RS's message for a missing binary, which blames an npm bug whatever the cause. */
const npmBugMessage = /(["'`])Cannot find native binding\. npm has a bug.*?\1/;

const troubleshootingUrl = "https://takumi.kane.tw/docs/troubleshooting#cannot-find-native-binding";

/** The platform package suffixes `@takumi-rs/core` publishes, such as `linux-x64-gnu`. */
const publishedTargets: string[] = JSON.parse(
  readFileSync(new URL("package.json", import.meta.url), "utf8"),
).napi.targets.map(packageSuffix);

/**
 * The message the loader throws instead, naming the platform package it looked for. `isMusl` is
 * the generated loader's own libc check.
 */
const missingBindingMessage = `(() => {
  const target =
    process.platform === "linux"
      ? \`linux-\${process.arch}-\${isMusl() ? "musl" : "gnu"}\`
      : process.platform === "win32"
        ? \`win32-\${process.arch}-msvc\`
        : \`\${process.platform}-\${process.arch}\`;

  return ${JSON.stringify(publishedTargets)}.includes(target)
    ? \`Cannot find @takumi-rs/core-\${target}. Keep @takumi-rs/core out of the bundle, and install @takumi-rs/core-\${target} where the app runs. See ${troubleshootingUrl}\`
    : \`@takumi-rs/core has no native binding for \${target}. Render with @takumi-rs/wasm instead. See ${troubleshootingUrl}\`;
})()`;

const config: UserConfig = defineConfig({
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
        if (patchedOutputs.has(name)) {
          continue;
        }

        const file = new URL(`dist/${name}`, import.meta.url);
        const original = readFileSync(file, "utf8");

        if (!npmBugMessage.test(original) || !/\bconst isMusl = /.test(original)) {
          throw new Error(`Generated loader has no missing-binding error to replace in: ${name}`);
        }

        const patched = patchGeneratedLoader(original);

        if (patched.includes("npm has a bug")) {
          throw new Error(`Generated loader still blames npm for a missing binding in: ${name}`);
        }

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

export default config;

function patchGeneratedLoader(content: string) {
  return content
    .replaceAll(/^[^\n]*new URL\((["'])\.\1,\s*import\.meta\.url\)\.pathname;?[^\n]*\n?/gm, "")
    .replaceAll("process.env.NAPI_RS_NATIVE_LIBRARY_PATH", "process.env.TAKUMI_CORE_TARGET")
    .replace(npmBugMessage, () => missingBindingMessage)
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

/** The package suffix NAPI-RS names a target triple's platform package with, such as `linux-x64-gnu`. */
function packageSuffix(triple: string): string {
  const [arch = "", , os = "", abi] = triple.split("-");
  const cpu = arch === "x86_64" ? "x64" : arch === "aarch64" ? "arm64" : arch;
  const platform = os === "windows" ? "win32" : os;

  return [platform, cpu, abi].filter(Boolean).join("-");
}
