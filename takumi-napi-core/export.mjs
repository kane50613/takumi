import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

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
    case "linux": {
      const isMusl = process.report?.getReport().header.glibcVersionRuntime === undefined;

      switch (process.arch) {
        case "arm64":
          return isMusl ? "linux-arm64-musl" : "linux-arm64-gnu";
        case "x64":
          return isMusl ? "linux-x64-musl" : "linux-x64-gnu";
      }
      break;
    }
    case "win32":
      if (process.arch === "x64") {
        return "win32-x64-msvc";
      }

      return "win32-arm64-msvc";
  }

  return null;
}

function loadNativeModule(target) {
  if (process.env.TAKUMI_CORE_TARGET) {
    return require(/* turbopackOptional: true */ process.env.TAKUMI_CORE_TARGET);
  }

  switch (target) {
    case "darwin-arm64":
      try {
        return require(/* turbopackOptional: true */ "./core.darwin-arm64.node");
      } catch {}
      try {
        return require(/* turbopackOptional: true */ "@takumi-rs/core-darwin-arm64");
      } catch {}
      break;
    case "darwin-x64":
      try {
        return require(/* turbopackOptional: true */ "./core.darwin-x64.node");
      } catch {}
      try {
        return require(/* turbopackOptional: true */ "@takumi-rs/core-darwin-x64");
      } catch {}
      break;
    case "linux-arm64-gnu":
      try {
        return require(/* turbopackOptional: true */ "./core.linux-arm64-gnu.node");
      } catch {}
      try {
        return require(/* turbopackOptional: true */ "@takumi-rs/core-linux-arm64-gnu");
      } catch {}
      break;
    case "linux-arm64-musl":
      try {
        return require(/* turbopackOptional: true */ "./core.linux-arm64-musl.node");
      } catch {}
      try {
        return require(/* turbopackOptional: true */ "@takumi-rs/core-linux-arm64-musl");
      } catch {}
      break;
    case "linux-x64-gnu":
      try {
        return require(/* turbopackOptional: true */ "./core.linux-x64-gnu.node");
      } catch {}
      try {
        return require(/* turbopackOptional: true */ "@takumi-rs/core-linux-x64-gnu");
      } catch {}
      break;
    case "linux-x64-musl":
      try {
        return require(/* turbopackOptional: true */ "./core.linux-x64-musl.node");
      } catch {}
      try {
        return require(/* turbopackOptional: true */ "@takumi-rs/core-linux-x64-musl");
      } catch {}
      break;
    case "win32-arm64-msvc":
      try {
        return require(/* turbopackOptional: true */ "./core.win32-arm64-msvc.node");
      } catch {}
      try {
        return require(/* turbopackOptional: true */ "@takumi-rs/core-win32-arm64-msvc");
      } catch {}
      break;
    case "win32-x64-msvc":
      try {
        return require(/* turbopackOptional: true */ "./core.win32-x64-msvc.node");
      } catch {}
      try {
        return require(/* turbopackOptional: true */ "@takumi-rs/core-win32-x64-msvc");
      } catch {}
      break;
  }

  return null;
}

const target = resolveTarget();
const nativeModule = loadNativeModule(target);

if (!nativeModule) {
  if (!target) {
    throw new Error(`Unsupported platform or architecture: ${process.platform} ${process.arch}`);
  }

  if (process.env.NEXT_RUNTIME === "nodejs") {
    throw new Error(
      `Native module @takumi-rs/core-${target} is not being bunlded.
Add \`serverExternalPackages: ["@takumi-rs/core"]\` to your next.config.js.
If you deployed from a different platform, make sure to manually install @takumi-rs/core-${target}.`,
    );
  }

  throw new Error(
    `Failed to load native module @takumi-rs/core-${target}. If you deployed from a different platform, make sure to manually install @takumi-rs/core-${target}.`,
  );
}

const { Renderer, extractResourceUrls } = nativeModule;

export default nativeModule;

export { Renderer, extractResourceUrls };
