import type { Font, FontDetails, ImageSource } from "../index";
import { Renderer as NativeRenderer, extractResourceUrls } from "../index";
import * as nativeModule from "../index";

export type * from "../index";
export { extractResourceUrls };

export type ImageSourceLoader = Omit<ImageSource, "data"> & {
  data: ImageSource["data"] | (() => Promise<ImageSource["data"]> | ImageSource["data"]);
};

export type FontLoader =
  | Font
  | (Omit<FontDetails, "data"> & {
      key?: string;
      data: (() => Promise<FontDetails["data"]>) | (() => FontDetails["data"]);
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

export class Renderer extends NativeRenderer {
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
    const batchFontsMark = new Set<string>();
    const batchFontBuffersMark = new WeakSet<FontDetails["data"]>();
    const targetFonts = fonts.filter((font) => {
      const key = createFontKey(font);

      if (isBuffer(key)) {
        if (this.fontBuffersMark.has(key) || batchFontBuffersMark.has(key)) {
          return false;
        }

        batchFontBuffersMark.add(key);
        return true;
      }

      if (this.fontsMark.has(key) || batchFontsMark.has(key)) {
        return false;
      }

      batchFontsMark.add(key);
      return true;
    });

    const resolvedFonts = await Promise.all(targetFonts.map(resolveFontLoader));
    const loadedCount = await super.loadFonts(resolvedFonts, signal);

    targetFonts.forEach((font) => this.checkAndMarkFont(font));

    return loadedCount;
  }

  override async loadFont(data: FontLoader, signal?: AbortSignal): Promise<number> {
    if (!this.isNewFont(data)) {
      return 0;
    }

    const resolved = await resolveFontLoader(data);
    const loadedCount = await super.loadFont(resolved, signal);

    this.checkAndMarkFont(data);

    return loadedCount;
  }

  override loadFontSync(font: FontLoaderSync): void {
    if (!this.isNewFont(font)) {
      return;
    }

    const resolved = resolveSyncFontLoader(font);
    super.loadFontSync(resolved);
    this.checkAndMarkFont(font);
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

  private isNewFont(font: FontLoader | FontLoaderSync) {
    const key = createFontKey(font);

    return isBuffer(key) ? !this.fontBuffersMark.has(key) : !this.fontsMark.has(key);
  }
}

const exportedModule = {
  ...nativeModule,
  Renderer,
  extractResourceUrls,
};

export default exportedModule;

function createFontKey(font: FontLoader | FontLoaderSync) {
  if ("key" in font && font.key) {
    return font.key;
  }

  if (isBuffer(font)) {
    return font;
  }

  return `${font.name ?? ""}-${font.style ?? ""}-${font.weight ?? ""}`;
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
