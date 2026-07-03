import {
  Renderer as RendererInternal,
  type Node,
  type RegisteredFamily,
  type RenderAnimationOptions as RenderAnimationOptionsInternal,
  type RenderOptions as RenderOptionsInternal,
  type SvgRenderOptions as SvgRenderOptionsInternal,
} from "../pkg/takumi_wasm";

export * from "../pkg/takumi_wasm";
export { default } from "../pkg/takumi_wasm";

import { subsetFonts } from "@takumi-rs/helpers";
import { FontRegistry } from "@takumi-rs/helpers/renderer";
import type {
  AnimationOutputFormatOptions,
  FontLoader,
  ImagesInput,
  OutputFormatOptions,
} from "@takumi-rs/helpers/renderer";

export type {
  AnimationOutputFormatOptions,
  FontLoader,
  ImageLoader,
  ImagesInput,
  OutputFormatOptions,
} from "@takumi-rs/helpers/renderer";

export type RenderOptions = Omit<
  RenderOptionsInternal,
  "images" | "format" | "quality" | "lossless"
> &
  OutputFormatOptions & {
    fonts?: FontLoader[];
    signal?: AbortSignal;
    images?: ImagesInput;
  };

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

export class Renderer {
  private inner = new RendererInternal();
  private fonts = new FontRegistry<RegisteredFamily>((font) => this.inner.registerFont(font));

  async render(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options ?? {};
    signal?.throwIfAborted();
    const resolved = await this.fonts.resolveResources(
      fonts && subsetFonts({ fonts, source: node }),
      images,
      fontFamilies,
    );
    signal?.throwIfAborted();

    return this.inner.render(node, { ...rest, ...resolved });
  }

  async renderAsDataUrl(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options ?? {};
    signal?.throwIfAborted();
    const resolved = await this.fonts.resolveResources(
      fonts && subsetFonts({ fonts, source: node }),
      images,
      fontFamilies,
    );
    signal?.throwIfAborted();

    return this.inner.renderAsDataUrl(node, { ...rest, ...resolved });
  }

  async renderSvg(node: Node, options?: SvgRenderOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options ?? {};
    signal?.throwIfAborted();
    const resolved = await this.fonts.resolveResources(
      fonts && subsetFonts({ fonts, source: node }),
      images,
      fontFamilies,
    );
    signal?.throwIfAborted();

    return this.inner.renderSvg(node, { ...rest, ...resolved });
  }

  async measure(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options ?? {};
    signal?.throwIfAborted();
    const resolved = await this.fonts.resolveResources(
      fonts && subsetFonts({ fonts, source: node }),
      images,
      fontFamilies,
    );
    signal?.throwIfAborted();

    return this.inner.measure(node, { ...rest, ...resolved });
  }

  async renderAnimation(options: RenderAnimationOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options;
    signal?.throwIfAborted();
    const nodes = options.scenes.map((scene) => scene.node);
    const resolved = await this.fonts.resolveResources(
      fonts && subsetFonts({ fonts, source: nodes }),
      images,
      fontFamilies,
    );
    signal?.throwIfAborted();

    return this.inner.renderAnimation({ ...rest, ...resolved });
  }

  registerFont(font: FontLoader) {
    return this.fonts.register(font);
  }

  /** Releases the underlying wasm renderer's memory. */
  free() {
    this.inner.free();
  }
}
