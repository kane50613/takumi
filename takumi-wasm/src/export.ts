import {
  Renderer as RendererInternal,
  type ByteBuf,
  type Font,
  type FontDetails,
  type ImageSource,
  type Node,
  type RegisteredFamily,
  type RenderAnimationOptions as RenderAnimationOptionsInternal,
  type RenderOptions as RenderOptionsInternal,
  type SvgRenderOptions as SvgRenderOptionsInternal,
} from "../pkg/takumi_wasm";

export * from "../pkg/takumi_wasm";
export { default } from "../pkg/takumi_wasm";

import { extractResourceUrls, pickFonts } from "@takumi-rs/helpers";

export { extractResourceUrls };

export type FontLoader =
  | Font
  | (Omit<FontDetails, "data"> & {
      data: () => Promise<FontDetails["data"]> | FontDetails["data"];
      /** Inclusive codepoint ranges this face covers; lets `render` skip it when unused. */
      ranges?: [number, number][];
    } & ({ key: string } | { name: string }));

type ImageLoaderData = ImageSource["data"];

export type ImageLoader = Omit<ImageSource, "data"> & {
  data: ImageLoaderData | (() => ImageLoaderData | Promise<ImageLoaderData>);
};

/**
 * Output format. Format-specific options live on the variant that supports them,
 * so `quality` cannot be paired with a lossless format. On wasm, WebP is always
 * lossless (lossy WebP is native-only).
 */
export type OutputFormatOptions =
  | { format?: "png" }
  | { format: "jpeg"; quality?: number }
  | { format: "webp" }
  | { format: "ico" }
  | { format: "raw" };

export type RenderOptions = Omit<RenderOptionsInternal, "images" | "format" | "quality"> &
  OutputFormatOptions & {
    fonts?: FontLoader[];
    images?: ImageLoader[];
    /** Register only the `fonts` subsets the content renders. @default true */
    subset?: boolean;
  };

/**
 * Animation output format. On wasm, WebP animation is always lossless (lossy
 * WebP is native-only).
 */
export type AnimationOutputFormatOptions =
  | { format?: "webp" }
  | { format: "apng" }
  | { format: "gif" };

export type RenderAnimationOptions = Omit<RenderAnimationOptionsInternal, "images" | "format"> &
  AnimationOutputFormatOptions & {
    fonts?: FontLoader[];
    images?: ImageLoader[];
    /** Register only the `fonts` subsets the content renders. @default true */
    subset?: boolean;
  };

export type SvgRenderOptions = Omit<SvgRenderOptionsInternal, "images"> & {
  fonts?: FontLoader[];
  images?: ImageLoader[];
  /** Register only the `fonts` subsets the content renders. @default true */
  subset?: boolean;
};

async function resolveImageLoaders(images: ImageLoader[]): Promise<ImageSource[]> {
  const bySrc = new Map<string, ImageLoader>();

  for (const image of images) {
    bySrc.set(image.src, image);
  }

  return Promise.all(
    [...bySrc.values()].map(async ({ src, data, cache }) => ({
      src,
      data: typeof data === "function" ? await data() : data,
      cache,
    })),
  );
}

export class Renderer {
  private fontsByKey = new Map<string, Promise<RegisteredFamily[]>>();
  private fontsByData = new WeakMap<ByteBuf, Promise<RegisteredFamily[]>>();
  private inner = new RendererInternal();

  private getFont(key: string | ByteBuf) {
    return typeof key === "string" ? this.fontsByKey.get(key) : this.fontsByData.get(key);
  }

  private setFont(key: string | ByteBuf, family: Promise<RegisteredFamily[]>) {
    if (typeof key === "string") {
      this.fontsByKey.set(key, family);
    } else {
      this.fontsByData.set(key, family);
    }
  }

  private deleteFont(key: string | ByteBuf) {
    if (typeof key === "string") {
      this.fontsByKey.delete(key);
    } else {
      this.fontsByData.delete(key);
    }
  }

  private async prepareFonts(fonts: FontLoader[] | undefined) {
    if (!fonts) {
      return;
    }

    const families = await Promise.all(fonts.map(this.registerFont.bind(this)));

    return [...new Set(families.flat().map((f) => f.name))];
  }

  async render(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, images, subset, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(pickFonts({ fonts, source: node, subset }));
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.render(node, {
      ...rest,
      images: resolvedImages,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });
  }

  async renderAsDataUrl(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, images, subset, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(pickFonts({ fonts, source: node, subset }));
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.renderAsDataUrl(node, {
      ...rest,
      images: resolvedImages,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });
  }

  async renderSvg(node: Node, options?: SvgRenderOptions) {
    const { fonts, fontFamilies, images, subset, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(pickFonts({ fonts, source: node, subset }));
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.renderSvg(node, {
      ...rest,
      images: resolvedImages,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });
  }

  async measure(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, images, subset, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(pickFonts({ fonts, source: node, subset }));
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.measure(node, {
      ...rest,
      images: resolvedImages,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });
  }

  async renderAnimation(options: RenderAnimationOptions) {
    const { fonts, fontFamilies, images, subset, ...rest } = options;
    const nodes = options.scenes.map((scene) => scene.node);
    const registeredFamilies = await this.prepareFonts(pickFonts({ fonts, source: nodes, subset }));
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.renderAnimation({
      ...rest,
      images: resolvedImages,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });
  }

  free() {
    this.inner.free();
  }

  async registerFont(font: FontLoader) {
    const key = createFontKey(font);

    const cached = this.getFont(key);
    if (cached) {
      return cached;
    }

    const extracted = extractFontBuffer(font);
    // Keep the descriptor's name/subsetOf/weight/style; only the data is resolved.
    const register = (data: ByteBuf) =>
      this.inner.registerFont(isBuffer(font) ? data : { ...font, data });

    if (isBuffer(extracted)) {
      const binded = register(extracted);

      this.setFont(key, Promise.resolve(binded));

      return binded;
    }

    const promise = extracted.then(register).catch((error) => {
      this.deleteFont(key);
      throw error;
    });

    this.setFont(key, promise);

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
