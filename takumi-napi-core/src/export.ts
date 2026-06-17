import type { Font, FontDetails } from "../index";
export type * from "../index";
import { Renderer as NativeRenderer } from "../index";

export { extractResourceUrls } from "@takumi-rs/helpers";

export type FontLoader =
  | Font
  | (Omit<FontDetails, "data"> & {
      key?: string;
      data: FontDetails["data"] | (() => Promise<FontDetails["data"]> | FontDetails["data"]);
    });

export class Renderer extends NativeRenderer {
  private fontsMark = new Set<string>();
  private fontBuffersMark = new WeakSet<FontDetails["data"]>();

  override async loadFonts(fonts: FontLoader[], signal?: AbortSignal) {
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

  private checkAndMarkFont(font: FontLoader) {
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

function createFontKey(font: FontLoader) {
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

function isBuffer(data: unknown): data is Uint8Array | ArrayBuffer {
  return data instanceof Uint8Array || data instanceof ArrayBuffer;
}
