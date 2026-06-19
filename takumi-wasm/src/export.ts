import {
  Renderer as RendererInternal,
  type ByteBuf,
  type Font,
  type FontDetails,
  type Node,
  type RegisteredFamily,
  type RenderOptions as RenderOptionsInternal,
} from "../pkg/takumi_wasm";

export * from "../pkg/takumi_wasm";
export { default } from "../pkg/takumi_wasm";

export { extractResourceUrls } from "@takumi-rs/helpers";

export type FontLoader =
  | Font
  | (Omit<FontDetails, "data"> & {
      key: string;
      data: () => Promise<FontDetails["data"]> | FontDetails["data"];
    });

export type RenderOptions = Omit<RenderOptionsInternal, "fonts"> & {
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
    const fonts = await this.prepareFonts(options?.fonts);

    return this.inner.render(node, {
      ...options,
      fonts,
    });
  }

  async renderAsDataUrl(node: Node, options?: RenderOptions) {
    const fonts = await this.prepareFonts(options?.fonts);

    return this.inner.renderAsDataUrl(node, {
      ...options,
      fonts,
    });
  }

  async measure(node: Node, options?: RenderOptions) {
    const fonts = await this.prepareFonts(options?.fonts);

    return this.inner.measure(node, {
      ...options,
      fonts,
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

    const promise = extracted.then(this.inner.registerFont.bind(this.inner));

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

  if ("key" in font) {
    return font.key;
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
