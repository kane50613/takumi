import type {
  AnimationFrameSource,
  ByteBuf,
  EncodeFramesOptions,
  Font,
  FontDetails,
  Node,
  RegisteredFamily,
  RenderAnimationOptions,
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

export type RenderOptions = RenderOptionsInternal & {
  fonts?: FontLoader[];
};

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
    const { fonts, fontFamilies, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(fonts);

    return this.inner.render(node, {
      ...rest,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });
  }

  async measure(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, ...rest } = options ?? {};
    const registeredFamilies = await this.prepareFonts(fonts);

    return this.inner.measure(node, {
      ...rest,
      fontFamilies: fontFamilies ?? registeredFamilies,
    });
  }

  renderAnimation(options: RenderAnimationOptions) {
    return this.inner.renderAnimation(options);
  }

  encodeFrames(frames: AnimationFrameSource[], options: EncodeFramesOptions) {
    return this.inner.encodeFrames(frames, options);
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
