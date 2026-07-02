import type {
  ByteBuf,
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

import { fontFromUrl, subsetFonts } from "@takumi-rs/helpers";

/**
 * A font to register. Either a URL string (fetched on demand, with name/weight/style read from the
 * file), raw bytes, or a descriptor with a lazy `data()` loader.
 */
export type FontLoader =
  | string
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

/** Images for a render: pre-fetched entries, or a group with a decode-cache default. */
export type ImagesInput =
  | ImageLoader[]
  | {
      /** Pre-fetched entries, same as the array form. */
      sources?: ImageLoader[];
      /** Decode-cache default for every image this render; a source's own `cache` wins. */
      cache?: NonNullable<ImageLoader["cache"]>;
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
    images?: ImagesInput;
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
    images?: ImagesInput;
  };

export type SvgRenderOptions = Omit<SvgRenderOptionsInternal, "images"> & {
  fonts?: FontLoader[];
  signal?: AbortSignal;
  images?: ImagesInput;
};

async function resolveImageLoaders(images: ImagesInput): Promise<ImageSource[]> {
  const { sources = [], cache } = Array.isArray(images) ? { sources: images } : images;
  const bySrc = new Map<string, ImageLoader>();

  for (const image of sources) {
    bySrc.set(image.src, image);
  }

  return Promise.all(
    [...bySrc.values()].map(async ({ src, data, cache: own }) => ({
      src,
      data: typeof data === "function" ? await data() : data,
      cache: own ?? cache,
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
    images: ImagesInput | undefined,
    fontFamilies: string[] | undefined,
  ) {
    const registeredFamilies = await this.prepareFonts(fonts);

    return {
      images: images ? await resolveImageLoaders(images) : undefined,
      fontFamilies: fontFamilies ?? registeredFamilies,
    };
  }

  async render(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options ?? {};
    const resolved = await this.resolveResources(
      fonts && subsetFonts({ fonts, source: node }),
      images,
      fontFamilies,
    );

    return this.inner.render(node, { ...rest, ...resolved }, signal);
  }

  async renderSvg(node: Node, options?: SvgRenderOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options ?? {};
    const resolved = await this.resolveResources(
      fonts && subsetFonts({ fonts, source: node }),
      images,
      fontFamilies,
    );

    return this.inner.renderSvg(node, { ...rest, ...resolved }, signal);
  }

  async measure(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options ?? {};
    const resolved = await this.resolveResources(
      fonts && subsetFonts({ fonts, source: node }),
      images,
      fontFamilies,
    );

    return this.inner.measure(node, { ...rest, ...resolved }, signal);
  }

  async renderAnimation(options: RenderAnimationOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options;
    const nodes = options.scenes.map((scene) => scene.node);
    const resolved = await this.resolveResources(
      fonts && subsetFonts({ fonts, source: nodes }),
      images,
      fontFamilies,
    );

    return this.inner.renderAnimation({ ...rest, ...resolved }, signal);
  }

  async registerFont(font: FontLoader) {
    const loader = typeof font === "string" ? fontFromUrl(font) : font;
    const key = createFontKey(loader);

    const cached = this.getFont(key);
    if (cached) {
      return cached;
    }

    const extracted = extractFontBuffer(loader);
    // Keep the descriptor's name/subsetOf/weight/style; only the data is resolved.
    const register = (data: ByteBuf) =>
      this.inner.registerFont(isBuffer(loader) ? data : { ...loader, data });

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

function extractFontBuffer(font: Exclude<FontLoader, string>) {
  if (isBuffer(font)) {
    return font;
  }

  if (typeof font.data !== "function") {
    return font.data;
  }

  return font.data();
}

function createFontKey(font: Exclude<FontLoader, string>) {
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
