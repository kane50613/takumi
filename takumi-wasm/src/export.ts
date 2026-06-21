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

type InnerOutputFormat = {
  format: NonNullable<RenderOptionsInternal["format"]>;
  quality?: number;
};

function toInnerOutputFormat(options: OutputFormatOptions): InnerOutputFormat {
  switch (options.format) {
    case "jpeg":
      return { format: "jpeg", quality: options.quality };
    case "webp":
      return { format: "webp" };
    case "ico":
      return { format: "ico" };
    case "raw":
      return { format: "raw" };
    default:
      return { format: "png" };
  }
}

export type RenderOptions = Omit<RenderOptionsInternal, "images" | "format" | "quality"> &
  OutputFormatOptions & {
    fonts?: FontLoader[];
    images?: ImageLoader[];
  };

/**
 * Animation output format. On wasm, WebP animation is always lossless (lossy
 * WebP is native-only).
 */
export type AnimationOutputFormatOptions =
  | { format?: "webp" }
  | { format: "apng" }
  | { format: "gif" };

type InnerAnimationFormat = {
  format: NonNullable<RenderAnimationOptionsInternal["format"]>;
};

function toInnerAnimationFormat(options: AnimationOutputFormatOptions): InnerAnimationFormat {
  switch (options.format) {
    case "apng":
      return { format: "apng" };
    case "gif":
      return { format: "gif" };
    default:
      return { format: "webp" };
  }
}

export type RenderAnimationOptions = Omit<RenderAnimationOptionsInternal, "images" | "format"> &
  AnimationOutputFormatOptions & {
    fonts?: FontLoader[];
    images?: ImageLoader[];
  };

export type EncodeFramesOptions = Omit<EncodeFramesOptionsInternal, "images" | "format"> &
  AnimationOutputFormatOptions & {
    fonts?: FontLoader[];
    images?: ImageLoader[];
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
  private fontMapping = new Map<string | ByteBuf, Promise<RegisteredFamily[]>>();
  private inner = new RendererInternal();

  private async prepareFonts(fonts: FontLoader[] | undefined) {
    if (!fonts) {
      return;
    }

    const families = await Promise.all(fonts.map(this.registerFont.bind(this)));

    return [...new Set(families.flat().map((f) => f.name))];
  }

  async render(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, images, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(fonts);
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.render(node, {
      ...rest,
      ...toInnerOutputFormat(options ?? {}),
      images: resolvedImages,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });
  }

  async renderAsDataUrl(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, images, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(fonts);
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.renderAsDataUrl(node, {
      ...rest,
      ...toInnerOutputFormat(options ?? {}),
      images: resolvedImages,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });
  }

  async measure(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, images, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(fonts);
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.measure(node, {
      ...rest,
      ...toInnerOutputFormat(options ?? {}),
      images: resolvedImages,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });
  }

  async renderAnimation(options: RenderAnimationOptions) {
    const { fonts, fontFamilies, images, ...rest } = options;
    const registeredFamilies = await this.prepareFonts(fonts);
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.renderAnimation({
      ...rest,
      ...toInnerAnimationFormat(options),
      images: resolvedImages,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });
  }

  async encodeFrames(frames: AnimationFrameSource[], options: EncodeFramesOptions) {
    const { fonts, fontFamilies, images, ...rest } = options;
    const registeredFamilies = await this.prepareFonts(fonts);
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.encodeFrames(frames, {
      ...rest,
      ...toInnerAnimationFormat(options),
      images: resolvedImages,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });
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
