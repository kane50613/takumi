import {
  Renderer as RendererInternal,
  type ByteBuf,
  type Font,
  type FontDetails,
} from "../pkg/takumi_wasm";

export * from "../pkg/takumi_wasm";
export { default } from "../pkg/takumi_wasm";

export { extractResourceUrls } from "@takumi-rs/helpers";

export type FontLoader =
  | Font
  | (Omit<FontDetails, "data"> & {
      key?: string;
      data: FontDetails["data"] | (() => Promise<FontDetails["data"]> | FontDetails["data"]);
    });

export class Renderer extends RendererInternal {
  private fontsMark = new Set<string>();
  private fontBuffersMark = new WeakSet<ByteBuf>();

  override async loadFonts(fonts: FontLoader[], signal?: AbortSignal): Promise<number> {
    const batchFontsMark = new Set<string>();
    const batchFontBuffersMark = new WeakSet<ByteBuf>();
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

    if (signal?.aborted) {
      return 0;
    }

    super.loadFonts(resolvedFonts);
    targetFonts.forEach((font) => this.checkAndMarkFont(font));

    return resolvedFonts.length;
  }

  private checkAndMarkFont(font: FontLoader): void {
    const key = createFontKey(font);

    if (isBuffer(key)) {
      this.fontBuffersMark.add(key);
      return;
    }

    this.fontsMark.add(key);
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

function resolveFontLoader(font: FontLoader): Font | Promise<Font> {
  if ("data" in font && typeof font.data === "function") {
    const resolved = font.data();

    if (isPromise(resolved)) {
      return resolved.then((data) => ({ ...font, data }));
    }

    return { ...font, data: resolved };
  }

  return font as Font;
}

function isPromise<T>(value: T | Promise<T>): value is Promise<T> {
  return typeof value === "object" && value !== null && "then" in value;
}

function isBuffer(data: unknown): data is ByteBuf {
  return (
    data instanceof Uint8Array ||
    data instanceof ArrayBuffer ||
    (typeof Buffer !== "undefined" && Buffer.isBuffer(data))
  );
}
