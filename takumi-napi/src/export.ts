import type {
  Node,
  RegisteredFamily,
  RenderAnimationOptions as RenderAnimationOptionsInternal,
  RenderOptions as RenderOptionsInternal,
  RendererOptions,
  SvgRenderOptions as SvgRenderOptionsInternal,
} from "../index";
export type * from "../index";
import { Renderer as RendererInternal } from "../index";

export { setGlyphCacheMaxBytes } from "../index";

import { FontRegistry, prepareRenderInput } from "@takumi-rs/helpers/renderer";
import { toNativeSource } from "./node-source";
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
  private inner: RendererInternal;
  private fonts: FontRegistry<RegisteredFamily>;

  constructor(options?: RendererOptions) {
    this.inner = new RendererInternal(options);
    this.fonts = new FontRegistry<RegisteredFamily>((font) => this.inner.registerFont(font));
  }

  async render(node: Node, options?: RenderOptions) {
    const { options: opts, signal } = await prepareRenderInput(this.fonts, options ?? {}, node);
    return this.inner.render(toNativeSource(node), opts, signal);
  }

  async renderSvg(node: Node, options?: SvgRenderOptions) {
    const { options: opts, signal } = await prepareRenderInput(this.fonts, options ?? {}, node);
    return this.inner.renderSvg(toNativeSource(node), opts, signal);
  }

  async measure(node: Node, options?: RenderOptions) {
    const { options: opts, signal } = await prepareRenderInput(this.fonts, options ?? {}, node);
    return this.inner.measure(toNativeSource(node), opts, signal);
  }

  async renderAnimation(options: RenderAnimationOptions) {
    const nodes = options.scenes.map((scene) => scene.node);
    const { options: opts, signal } = await prepareRenderInput(this.fonts, options, nodes);
    return this.inner.renderAnimation(opts, signal);
  }

  registerFont(font: FontLoader) {
    return this.fonts.register(font);
  }

  /**
   * Removes every face registered under `family`, each `unicodeRange` subset alike, so the
   * renderer lets go of it; returns whether it had any. A later render that lists the font
   * registers it again.
   */
  unregisterFont(family: string) {
    this.fonts.forget(family);

    return this.inner.unregisterFont(family);
  }

  /** Removes every registered font, leaving the bundled fallback font. */
  clearFonts() {
    this.fonts.forgetAll();
    this.inner.clearFonts();
  }
}
