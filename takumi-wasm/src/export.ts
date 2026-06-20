import {
  Renderer as RendererInternal,
  type AnimationFrameSource,
  type ByteBuf,
  type EncodeFramesOptions as EncodeFramesOptionsInternal,
  type Font,
  type FontDetails,
  type ImageSource,
  type Node,
  type RegisteredFamily,
  type RenderAnimationOptions as RenderAnimationOptionsInternal,
  type RenderOptions as RenderOptionsInternal,
} from "../pkg/takumi_wasm";

export * from "../pkg/takumi_wasm";
export { default } from "../pkg/takumi_wasm";

export { extractResourceUrls } from "@takumi-rs/helpers";

export type FontLoader =
  | Font
  | (Omit<FontDetails, "data"> & {
      data: () => Promise<FontDetails["data"]> | FontDetails["data"];
    } & ({ key: string } | { name: string }));

export type RenderOptions = RenderOptionsInternal & {
  fonts?: FontLoader[];
};

export type RenderAnimationOptions = RenderAnimationOptionsInternal & {
  fonts?: FontLoader[];
};

export type EncodeFramesOptions = EncodeFramesOptionsInternal & {
  fonts?: FontLoader[];
};

export class Renderer {
  private fontMapping = new Map<string | ByteBuf, Promise<RegisteredFamily[]>>();
  private sentImmutableSrcs = new Set<string>();
  private inner = new RendererInternal();

  private filterImages(images: ImageSource[] | undefined): {
    images: ImageSource[] | undefined;
    commit: () => void;
  } {
    if (!images) {
      return { images, commit: () => {} };
    }

    const filtered: ImageSource[] = [];
    const newlySent: string[] = [];

    for (const image of images) {
      if (image.cache === "immutable") {
        if (this.sentImmutableSrcs.has(image.src) || newlySent.includes(image.src)) {
          continue;
        }

        newlySent.push(image.src);
      }

      filtered.push(image);
    }

    return {
      images: filtered,
      commit: () => {
        for (const src of newlySent) {
          this.sentImmutableSrcs.add(src);
        }
      },
    };
  }

  private async prepareFonts(fonts: FontLoader[] | undefined) {
    if (!fonts) {
      return;
    }

    const families = await Promise.all(fonts.map(this.registerFont.bind(this)));

    return [...new Set(families.flat().map((f) => f.name))];
  }

  async render(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(fonts);
    const { images, commit } = this.filterImages(rest.images);

    const result = await this.inner.render(node, {
      ...rest,
      images,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });

    commit();

    return result;
  }

  async renderAsDataUrl(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(fonts);
    const { images, commit } = this.filterImages(rest.images);

    const result = await this.inner.renderAsDataUrl(node, {
      ...rest,
      images,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });

    commit();

    return result;
  }

  async measure(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(fonts);
    const { images, commit } = this.filterImages(rest.images);

    const result = await this.inner.measure(node, {
      ...rest,
      images,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });

    commit();

    return result;
  }

  async renderAnimation(options: RenderAnimationOptions) {
    const { fonts, fontFamilies, ...rest } = options;
    const registeredFamilies = await this.prepareFonts(fonts);
    const { images, commit } = this.filterImages(rest.images);

    const result = await this.inner.renderAnimation({
      ...rest,
      images,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });

    commit();

    return result;
  }

  async encodeFrames(frames: AnimationFrameSource[], options: EncodeFramesOptions) {
    const { fonts, fontFamilies, ...rest } = options;
    const registeredFamilies = await this.prepareFonts(fonts);
    const { images, commit } = this.filterImages(rest.images);

    const result = await this.inner.encodeFrames(frames, {
      ...rest,
      images,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });

    commit();

    return result;
  }

  free() {
    this.inner.free();
  }

  async registerFont(font: FontLoader) {
    const key = createFontKey(font);

    const cached = this.fontMapping.get(key);
    if (cached) {
      return cached;
    }

    const extracted = extractFontBuffer(font);

    if (isBuffer(extracted)) {
      const binded = this.inner.registerFont(extracted);

      this.fontMapping.set(key, Promise.resolve(binded));

      return binded;
    }

    const promise = extracted.then(this.inner.registerFont.bind(this.inner)).catch((error) => {
      this.fontMapping.delete(key);
      throw error;
    });

    this.fontMapping.set(key, promise);

    return promise;
  }
}

function extractFontBuffer(font: FontLoader) {
  if (isBuffer(font)) {
    return font;
  }

  if (typeof font.data !== "function") {
    return font.data;
  }

  return font.data();
}

function createFontKey(font: FontLoader) {
  if (isBuffer(font)) {
    return font;
  }

  if ("key" in font && font.key) {
    return font.key;
  }

  if (typeof font.data === "function") {
    return `${font.name ?? ""}:${font.weight ?? ""}:${font.style ?? ""}`;
  }

  return font.data;
}

function isBuffer(data: unknown): data is ByteBuf {
  return (
    data instanceof Uint8Array ||
    data instanceof ArrayBuffer ||
    (typeof Buffer !== "undefined" && Buffer.isBuffer(data))
  );
}
