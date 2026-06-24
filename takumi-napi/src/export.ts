import type {
  AnimationFrameSource,
  ByteBuf,
  EncodeFramesOptions as EncodeFramesOptionsInternal,
  Font,
  FontDetails,
  ImageSource,
  Node,
  RegisteredFamily,
  RenderAnimationOptions as RenderAnimationOptionsInternal,
  RenderOptions as RenderOptionsInternal,
  SvgRenderOptions as SvgRenderOptionsInternal,
} from "../index";
export type * from "../index";
import { Renderer as RendererInternal } from "../index";

import { pickFonts } from "@takumi-rs/helpers";

export { extractResourceUrls } from "@takumi-rs/helpers";

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
 * so `quality` cannot be paired with PNG/ICO/raw, and `lossless` is WebP-only.
 * For WebP, `lossless` takes precedence over `quality`; omitting both encodes
 * losslessly.
 */
export type OutputFormatOptions =
  | { format?: "png" }
  | { format: "jpeg"; quality?: number }
  | { format: "webp"; quality?: number; lossless?: boolean }
  | { format: "ico" }
  | { format: "raw" };

export type RenderOptions = Omit<
  RenderOptionsInternal,
  "images" | "format" | "quality" | "lossless"
> &
  OutputFormatOptions & {
    fonts?: FontLoader[];
    signal?: AbortSignal;
    images?: ImageLoader[];
    /** Register only the `fonts` subsets the content renders. @default true */
    subset?: boolean;
  };

/**
 * Animation output format. `quality` and `lossless` are WebP-only; for WebP,
 * `lossless` takes precedence over `quality`, and omitting both encodes
 * losslessly.
 */
export type AnimationOutputFormatOptions =
  | { format?: "webp"; quality?: number; lossless?: boolean }
  | { format: "apng" }
  | { format: "gif" };

export type RenderAnimationOptions = Omit<
  RenderAnimationOptionsInternal,
  "images" | "format" | "quality" | "lossless"
> &
  AnimationOutputFormatOptions & {
    fonts?: FontLoader[];
    signal?: AbortSignal;
    images?: ImageLoader[];
    /** Register only the `fonts` subsets the content renders. @default true */
    subset?: boolean;
  };

export type EncodeFramesOptions = Omit<
  EncodeFramesOptionsInternal,
  "images" | "format" | "quality" | "lossless"
> &
  AnimationOutputFormatOptions & {
    fonts?: FontLoader[];
    signal?: AbortSignal;
    images?: ImageLoader[];
    /** Register only the `fonts` subsets the content renders. @default true */
    subset?: boolean;
  };

export type SvgRenderOptions = Omit<SvgRenderOptionsInternal, "images"> & {
  fonts?: FontLoader[];
  signal?: AbortSignal;
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

  /** Registers `fonts` and resolves lazy `images`, yielding the `images`/`fontFamilies`
   * the napi binding expects. Explicit `fontFamilies` wins over the registered set. */
  private async resolveResources(
    fonts: FontLoader[] | undefined,
    images: ImageLoader[] | undefined,
    fontFamilies: string[] | undefined,
  ) {
    const registeredFamilies = await this.prepareFonts(fonts);

    return {
      images: images ? await resolveImageLoaders(images) : undefined,
      fontFamilies: fontFamilies ?? registeredFamilies,
    };
  }

  async render(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, signal, images, subset, ...rest } = options ?? {};
    const resolved = await this.resolveResources(
      pickFonts(fonts, node, subset),
      images,
      fontFamilies,
    );

    return this.inner.render(node, { ...rest, ...resolved }, signal);
  }

  async renderSvg(node: Node, options?: SvgRenderOptions) {
    const { fonts, fontFamilies, signal, images, subset, ...rest } = options ?? {};
    const resolved = await this.resolveResources(
      pickFonts(fonts, node, subset),
      images,
      fontFamilies,
    );

    return this.inner.renderSvg(node, { ...rest, ...resolved }, signal);
  }

  async measure(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, signal, images, subset, ...rest } = options ?? {};
    const resolved = await this.resolveResources(
      pickFonts(fonts, node, subset),
      images,
      fontFamilies,
    );

    return this.inner.measure(node, { ...rest, ...resolved }, signal);
  }

  async renderAnimation(options: RenderAnimationOptions) {
    const { fonts, fontFamilies, signal, images, subset, ...rest } = options;
    const nodes = options.scenes.map((scene) => scene.node);
    const resolved = await this.resolveResources(
      pickFonts(fonts, nodes, subset),
      images,
      fontFamilies,
    );

    return this.inner.renderAnimation({ ...rest, ...resolved }, signal);
  }

  async encodeFrames(frames: AnimationFrameSource[], options: EncodeFramesOptions) {
    const { fonts, fontFamilies, signal, images, subset, ...rest } = options;
    const nodes = frames.map((frame) => frame.node);
    const resolved = await this.resolveResources(
      pickFonts(fonts, nodes, subset),
      images,
      fontFamilies,
    );

    return this.inner.encodeFrames(frames, { ...rest, ...resolved }, signal);
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
