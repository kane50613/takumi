import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const loadModule = (specifier) =>
  Function("require", "specifier", "return require(specifier)")(require, specifier);

function resolveTarget() {
  switch (process.platform) {
    case "darwin":
      switch (process.arch) {
        case "arm64":
          return "darwin-arm64";
        case "x64":
          return "darwin-x64";
      }
      break;
    case "linux":
      switch (process.arch) {
        case "arm64":
          return process.report?.getReport().header.glibcVersionRuntime
            ? "linux-arm64-gnu"
            : "linux-arm64-musl";
        case "x64":
          return process.report?.getReport().header.glibcVersionRuntime
            ? "linux-x64-gnu"
            : "linux-x64-musl";
      }
      break;
    case "win32":
      if (process.arch === "x64") {
        return "win32-x64-msvc";
      }
      break;
  }

  return null;
}

function loadNativeModule() {
  if (process.env.TAKUMI_CORE_TARGET) {
    return loadModule(process.env.TAKUMI_CORE_TARGET);
  }

  const target = resolveTarget();
  if (!target) {
    return null;
  }

  try {
    return loadModule(`@takumi-rs/core-${target}`);
  } catch {}

  try {
    return loadModule(fileURLToPath(new URL(`./core.${target}.node`, import.meta.url)));
  } catch {}

  return null;
}

const nativeModule = loadNativeModule();

if (!nativeModule) {
  throw new Error(
    "@takumi-rs/core is only available in Node.js runtimes. Use @takumi-rs/wasm or a higher-level package that falls back to WASM automatically.",
  );
}

const { Renderer, extractResourceUrls } = nativeModule;

export default nativeModule;

export { Renderer, extractResourceUrls };
