import type {
  Node,
  RegisteredFamily,
  RenderAnimationOptions as RenderAnimationOptionsInternal,
  RenderOptions as RenderOptionsInternal,
  SvgRenderOptions as SvgRenderOptionsInternal,
} from "../index";
export type * from "../index";
import { Renderer as RendererInternal } from "../index";

import { FontRegistry, prepareRenderInput } from "@takumi-rs/helpers/renderer";
import type {
  BackendAnimationOptions,
  BackendRenderOptions,
  BackendSvgOptions,
  FontLoader,
} from "@takumi-rs/helpers/renderer";

export type {
  AnimationOutputFormatOptions,
  FontLoader,
  ImageLoader,
  ImagesInput,
  OutputFormatOptions,
} from "@takumi-rs/helpers/renderer";

export type RenderOptions = BackendRenderOptions<RenderOptionsInternal>;
export type RenderAnimationOptions = BackendAnimationOptions<RenderAnimationOptionsInternal>;
export type SvgRenderOptions = BackendSvgOptions<SvgRenderOptionsInternal>;

export class Renderer {
  private inner = new RendererInternal();
  private fonts = new FontRegistry<RegisteredFamily>((font) => this.inner.registerFont(font));

  async render(node: Node, options?: RenderOptions) {
    const { options: opts, signal } = await prepareRenderInput(this.fonts, options ?? {}, node);
    return this.inner.render(node, opts, signal);
  }

  async renderSvg(node: Node, options?: SvgRenderOptions) {
    const { options: opts, signal } = await prepareRenderInput(this.fonts, options ?? {}, node);
    return this.inner.renderSvg(node, opts, signal);
  }

  async measure(node: Node, options?: RenderOptions) {
    const { options: opts, signal } = await prepareRenderInput(this.fonts, options ?? {}, node);
    return this.inner.measure(node, opts, signal);
  }

  async renderAnimation(options: RenderAnimationOptions) {
    const nodes = options.scenes.map((scene) => scene.node);
    const { options: opts, signal } = await prepareRenderInput(this.fonts, options, nodes);
    return this.inner.renderAnimation(opts, signal);
  }

  registerFont(font: FontLoader) {
    return this.fonts.register(font);
  }
}
