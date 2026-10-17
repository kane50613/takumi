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
    const { options: opts } = await prepareRenderInput(this.fonts, options ?? {}, node);
    return this.inner.render(node, opts);
  }

  async renderAsDataUrl(node: Node, options?: RenderOptions) {
    const { options: opts } = await prepareRenderInput(this.fonts, options ?? {}, node);
    return this.inner.renderAsDataUrl(node, opts);
  }

  async renderSvg(node: Node, options?: SvgRenderOptions) {
    const { options: opts } = await prepareRenderInput(this.fonts, options ?? {}, node);
    return this.inner.renderSvg(node, opts);
  }

  async measure(node: Node, options?: RenderOptions) {
    const { options: opts } = await prepareRenderInput(this.fonts, options ?? {}, node);
    return this.inner.measure(node, opts);
  }

  async renderAnimation(options: RenderAnimationOptions) {
    const nodes = options.scenes.map((scene) => scene.node);
    const { options: opts } = await prepareRenderInput(this.fonts, options, nodes);
    return this.inner.renderAnimation(opts);
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

  /** Releases the underlying wasm renderer's memory. */
  free() {
    this.inner.free();
  }
}
