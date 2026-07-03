import type {
  Node,
  RegisteredFamily,
  RenderAnimationOptions as RenderAnimationOptionsInternal,
  RenderOptions as RenderOptionsInternal,
  SvgRenderOptions as SvgRenderOptionsInternal,
} from "../index";
export type * from "../index";
import { Renderer as RendererInternal } from "../index";

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
    const resolved = await this.fonts.resolveResources(
      fonts && subsetFonts({ fonts, source: node }),
      images,
      fontFamilies,
    );

    return this.inner.render(node, { ...rest, ...resolved }, signal);
  }

  async renderSvg(node: Node, options?: SvgRenderOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options ?? {};
    const resolved = await this.fonts.resolveResources(
      fonts && subsetFonts({ fonts, source: node }),
      images,
      fontFamilies,
    );

    return this.inner.renderSvg(node, { ...rest, ...resolved }, signal);
  }

  async measure(node: Node, options?: RenderOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options ?? {};
    const resolved = await this.fonts.resolveResources(
      fonts && subsetFonts({ fonts, source: node }),
      images,
      fontFamilies,
    );

    return this.inner.measure(node, { ...rest, ...resolved }, signal);
  }

  async renderAnimation(options: RenderAnimationOptions) {
    const { fonts, fontFamilies, signal, images, ...rest } = options;
    const nodes = options.scenes.map((scene) => scene.node);
    const resolved = await this.fonts.resolveResources(
      fonts && subsetFonts({ fonts, source: nodes }),
      images,
      fontFamilies,
    );

    return this.inner.renderAnimation({ ...rest, ...resolved }, signal);
  }

  registerFont(font: FontLoader) {
    return this.fonts.register(font);
  }
}
