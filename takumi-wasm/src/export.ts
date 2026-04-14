import {
  Renderer as RendererInternal,
  type ByteBuf,
  type Font,
  type FontDetails,
  type ImageSource,
} from "../pkg/takumi_wasm";

export * from "../pkg/takumi_wasm";
export { default } from "../pkg/takumi_wasm";

export { extractResourceUrls } from "@takumi-rs/helpers";

export type ImageSourceLoader = Omit<ImageSource, "data"> & {
  data: ImageSource["data"] | (() => Promise<ImageSource["data"]> | ImageSource["data"]);
};

export type FontLoader =
  | Font
  | (Omit<FontDetails, "data"> & {
      key?: string;
      data: FontDetails["data"] | (() => Promise<FontDetails["data"]> | FontDetails["data"]);
    });

export type ImageSourceLoaderSync = Omit<ImageSource, "data"> & {
  data: ImageSource["data"] | (() => ImageSource["data"]);
};

export type FontLoaderSync =
  | Font
  | (Omit<FontDetails, "data"> & {
      key?: string;
      data: FontDetails["data"] | (() => FontDetails["data"]);
    });

export class Renderer extends RendererInternal {
  private fontsMark = new Set<string>();
  private fontBuffersMark = new WeakSet<ByteBuf>();
  private persistentImageSrcMark = new Set<string>();
  private pendingPersistentImages = new Map<string, Promise<void>>();

  override putPersistentImage(data: ImageSourceLoaderSync, signal?: AbortSignal): void;
  override putPersistentImage(data: ImageSourceLoader, signal?: AbortSignal): Promise<void>;
  override putPersistentImage(
    data: ImageSourceLoaderSync | ImageSourceLoader,
    signal?: AbortSignal,
  ): void | Promise<void> {
    if (signal?.aborted) {
      return;
    }

    if (!this.isNewPersistentImage(data.src)) {
      return this.pendingPersistentImages.get(data.src);
    }

    const resolved = resolveImageLoader(data);

    if (isPromise(resolved)) {
      const pending = resolved
        .then((value) => {
          if (signal?.aborted) {
            return;
          }

          super.putPersistentImage(value);
          this.persistentImageSrcMark.add(data.src);
        })
        .finally(() => {
          this.pendingPersistentImages.delete(data.src);
        });

      this.pendingPersistentImages.set(data.src, pending);

      return pending;
    }

    if (signal?.aborted) {
      return;
    }

    super.putPersistentImage(resolved);
    this.persistentImageSrcMark.add(data.src);
  }

  override clearImageStore(): void {
    super.clearImageStore();
    this.persistentImageSrcMark.clear();
    this.pendingPersistentImages.clear();
  }

  async loadFonts(fonts: FontLoader[], signal?: AbortSignal): Promise<number> {
    let loaded = 0;

    for (const font of fonts) {
      if (await this.loadFontInternal(font, signal)) {
        loaded += 1;
      }
    }

    return loaded;
  }

  override loadFont(data: FontLoaderSync, signal?: AbortSignal): void;
  override loadFont(data: FontLoader, signal?: AbortSignal): Promise<void>;
  override loadFont(data: FontLoaderSync | FontLoader, signal?: AbortSignal): void | Promise<void> {
    const loaded = this.loadFontInternal(data, signal);

    if (isPromise(loaded)) {
      return loaded.then(() => undefined);
    }
  }

  private loadFontInternal(
    font: FontLoaderSync | FontLoader,
    signal?: AbortSignal,
  ): boolean | Promise<boolean> {
    if (signal?.aborted) {
      return false;
    }

    const resolved = resolveFontLoader(font);

    if (isPromise(resolved)) {
      return resolved.then((value) => {
        if (signal?.aborted || !this.checkAndMarkFont(value)) {
          return false;
        }

        try {
          super.loadFont(value);
          return true;
        } catch (error) {
          this.unmarkFont(value);
          throw error;
        }
      });
    }

    if (!this.checkAndMarkFont(resolved)) {
      return false;
    }

    try {
      super.loadFont(resolved);
      return true;
    } catch (error) {
      this.unmarkFont(resolved);
      throw error;
    }
  }

  private checkAndMarkFont(font: FontLoaderSync | FontLoader): boolean {
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

  private unmarkFont(font: FontLoaderSync | FontLoader): void {
    const key = createFontKey(font);

    if (isBuffer(key)) {
      this.fontBuffersMark.delete(key);
      return;
    }

    this.fontsMark.delete(key);
  }

  private isNewPersistentImage(src: string): boolean {
    return !this.persistentImageSrcMark.has(src) && !this.pendingPersistentImages.has(src);
  }
}

function createFontKey(font: FontLoaderSync | FontLoader) {
  if ("key" in font && font.key) {
    return font.key;
  }

  if (isBuffer(font)) {
    return font;
  }

  return `${font.name ?? ""}-${font.style ?? ""}-${font.weight ?? ""}`;
}

function resolveFontLoader(font: FontLoaderSync | FontLoader): Font | Promise<Font> {
  if ("data" in font && typeof font.data === "function") {
    const resolved = font.data();

    if (isPromise(resolved)) {
      return resolved.then((data) => ({ ...font, data }));
    }

    return { ...font, data: resolved };
  }

  return font as Font;
}

function resolveImageLoader(
  source: ImageSourceLoaderSync | ImageSourceLoader,
): ImageSource | Promise<ImageSource> {
  if (typeof source.data === "function") {
    const resolved = source.data();

    if (isPromise(resolved)) {
      return resolved.then((data) => ({ ...source, data }));
    }

    return {
      ...source,
      data: resolved,
    };
  }

  return source as ImageSource;
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
