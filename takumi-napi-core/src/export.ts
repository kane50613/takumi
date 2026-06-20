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
} from "../index";
export type * from "../index";
import { Renderer as RendererInternal } from "../index";

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

export type RenderOptions = Omit<RenderOptionsInternal, "images"> & {
  fonts?: FontLoader[];
  signal?: AbortSignal;
  images?: ImageLoader[];
};

export type RenderAnimationOptions = Omit<RenderAnimationOptionsInternal, "images"> & {
  fonts?: FontLoader[];
  signal?: AbortSignal;
  images?: ImageLoader[];
};

export type EncodeFramesOptions = Omit<EncodeFramesOptionsInternal, "images"> & {
  fonts?: FontLoader[];
  signal?: AbortSignal;
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
    const { fonts, fontFamilies, signal, images, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(fonts);
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.render(
      node,
      {
        ...rest,
        images: resolvedImages,
        fontFamilies: fontFamilies ?? registeredFamilies,
      },
      signal,
    );
  }

  async measure(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(fonts);
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.measure(
      node,
      {
        ...rest,
        images: resolvedImages,
        fontFamilies: fontFamilies ?? registeredFamilies,
      },
      signal,
    );
  }

  async renderAnimation(options: RenderAnimationOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options;
    const registeredFamilies = await this.prepareFonts(fonts);
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.renderAnimation(
      {
        ...rest,
        images: resolvedImages,
        fontFamilies: fontFamilies ?? registeredFamilies,
      },
      signal,
    );
  }

  async encodeFrames(frames: AnimationFrameSource[], options: EncodeFramesOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options;
    const registeredFamilies = await this.prepareFonts(fonts);
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;

    return this.inner.encodeFrames(
      frames,
      {
        ...rest,
        images: resolvedImages,
        fontFamilies: fontFamilies ?? registeredFamilies,
      },
      signal,
    );
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
