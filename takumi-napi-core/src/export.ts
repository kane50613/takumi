import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { Font, FontDetails, ImageSource } from "../index";

export type * from "../index";

const require = createRequire(import.meta.url);

async function checkIsMusl() {
  const fileResult = await isMuslFromFile();

  if (fileResult !== undefined) {
    return fileResult;
  }

  return await isMuslFromReport();
}

async function isMuslFromFile() {
  try {
    const content = await readFile("/etc/ld.so.conf", "utf8");
    return content.includes("musl");
  } catch {}
}

async function isMuslFromReport() {
  try {
    if ("excludeNetwork" in process.report) {
      process.report.excludeNetwork = true;
    }

    const report = process.report.getReport() as {
      header?: {
        glibcVersionRuntime?: string;
      };
      sharedObjects?: string[];
    };

    if (report.header?.glibcVersionRuntime) {
      return false;
    }

    if (Array.isArray(report.sharedObjects)) {
      if (report.sharedObjects.some((f) => f.includes("libc.musl-") || f.includes("ld-musl-"))) {
        return true;
      }
    }

    return false;
  } catch {}
}

async function resolveTarget() {
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
      const isMusl = await checkIsMusl();

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

function loadNativeModule(target: string | null) {
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

const target = await resolveTarget();
const nativeModule: typeof import("../index") = loadNativeModule(target);

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

const { extractResourceUrls } = nativeModule;

export default nativeModule;

export { extractResourceUrls };

export type ImageSourceLoader = Omit<ImageSource, "data"> & {
  data: ImageSource["data"] | (() => Promise<ImageSource["data"]> | ImageSource["data"]);
};

export type FontLoader =
  | Font
  | (Omit<FontDetails, "data"> & {
      key?: string;
      data: () => Promise<FontDetails["data"]> | FontDetails["data"];
    });

export type ImageSourceLoaderSync = Omit<ImageSource, "data"> & {
  data: ImageSource["data"] | (() => ImageSource["data"]);
};

export type FontLoaderSync =
  | Font
  | (Omit<FontDetails, "data"> & {
      key?: string;
      data: () => FontDetails["data"];
    });

export class Renderer extends nativeModule.Renderer {
  private fontsMark = new Set<string>();
  private fontBuffersMark = new WeakSet<FontDetails["data"]>();

  override async putPersistentImage(
    source: ImageSourceLoader,
    signal?: AbortSignal,
  ): Promise<void> {
    const resolved = await resolveImageLoader(source);
    return super.putPersistentImage(resolved, signal);
  }

  override async loadFonts(fonts: FontLoader[], signal?: AbortSignal): Promise<number> {
    const targetFonts = fonts.filter(this.checkAndMarkFont.bind(this));

    const resolvedFonts = await Promise.all(targetFonts.map(resolveFontLoader));

    return super.loadFonts(resolvedFonts, signal);
  }

  override async loadFont(data: FontLoader, signal?: AbortSignal): Promise<number> {
    const isNew = this.checkAndMarkFont(data);

    if (!isNew) {
      return Promise.resolve(0);
    }

    const resolved = await resolveFontLoader(data);
    return super.loadFont(resolved, signal);
  }

  override loadFontSync(font: FontLoaderSync): void {
    const isNew = this.checkAndMarkFont(font);

    if (!isNew) {
      return;
    }

    const resolved = resolveSyncFontLoader(font);
    return super.loadFontSync(resolved);
  }

  private checkAndMarkFont(font: FontLoader | FontLoaderSync) {
    const key = createFontKey(font);

    if (isBuffer(key)) {
      const isNew = !this.fontBuffersMark.has(key);

      this.fontBuffersMark.add(key);
      return isNew;
    }

    const isNew = !this.fontsMark.has(key);

    this.fontsMark.add(key);

    return isNew;
  }
}

function createFontKey(font: FontLoader | FontLoaderSync) {
  if ("key" in font && font.key) {
    return font.key;
  }

  if (isBuffer(font)) {
    return font;
  }

  return `${font.name ?? ""}-${font.style ?? ""}-${font.weight ?? ""}-${isBuffer(font.data) ? font.data : ""}`;
}

async function resolveFontLoader(font: FontLoader) {
  if ("data" in font && typeof font.data === "function") {
    return {
      ...font,
      data: await font.data(),
    };
  }

  return font as Font;
}

async function resolveImageLoader(source: ImageSourceLoader): Promise<ImageSource> {
  if (typeof source.data === "function") {
    return {
      ...source,
      data: await source.data(),
    };
  }

  return source as ImageSource;
}

function resolveSyncFontLoader(font: FontLoaderSync) {
  if ("data" in font && typeof font.data === "function") {
    return {
      ...font,
      data: font.data(),
    };
  }

  return font as Font;
}

function isBuffer(data: unknown): data is Uint8Array | ArrayBuffer {
  return data instanceof Uint8Array || data instanceof ArrayBuffer;
}
