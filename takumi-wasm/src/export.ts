import {
  Renderer as RendererInternal,
  type Node,
  type RegisteredFamily,
  type RendererOptions as RendererOptionsInternal,
  type RenderAnimationOptions as RenderAnimationOptionsInternal,
  type RenderOptions as RenderOptionsInternal,
  type SvgRenderOptions as SvgRenderOptionsInternal,
} from "../pkg/takumi_wasm";

export * from "../pkg/takumi_wasm";
export { default } from "../pkg/takumi_wasm";

import { FontRegistry, withRenderInput } from "@takumi-rs/helpers/renderer";
import type {
  BackendAnimationOptions,
  BackendRenderOptions,
  BackendSvgOptions,
  FontCacheRendererOptions,
  FontLoader,
  RegisteredFamilyLike,
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
export type RendererOptions = RendererOptionsInternal & FontCacheRendererOptions;

export class Renderer {
  private inner: RendererInternal;
  private fonts: FontRegistry<RegisteredFamily>;

  constructor(options?: RendererOptions) {
    const { fontCacheMaxBytes, ...native } = options ?? {};

    this.inner = new RendererInternal(native);
    this.fonts = new FontRegistry<RegisteredFamily>((font) => this.inner.registerFont(font), {
      maxBytes: fontCacheMaxBytes,
      unregister: (family) => this.inner.unregisterFont(family),
    });
  }

  render(node: Node, options?: RenderOptions) {
    return withRenderInput(this.fonts, options ?? {}, node, (opts) =>
      this.inner.render(node, opts),
    );
  }

  renderAsDataUrl(node: Node, options?: RenderOptions) {
    return withRenderInput(this.fonts, options ?? {}, node, (opts) =>
      this.inner.renderAsDataUrl(node, opts),
    );
  }

  renderSvg(node: Node, options?: SvgRenderOptions) {
    return withRenderInput(this.fonts, options ?? {}, node, (opts) =>
      this.inner.renderSvg(node, opts),
    );
  }

  measure(node: Node, options?: RenderOptions) {
    return withRenderInput(this.fonts, options ?? {}, node, (opts) =>
      this.inner.measure(node, opts),
    );
  }

  renderAnimation(options: RenderAnimationOptions) {
    const nodes = options.scenes.map((scene) => scene.node);

    return withRenderInput(this.fonts, options, nodes, (opts) => this.inner.renderAnimation(opts));
  }

  registerFont(font: FontLoader) {
    return this.fonts.register(font);
  }

  /**
   * Removes every face registered under the family `name`, from any `registerFont` call and each
   * `unicodeRange` subset alike; returns whether it had any. Takes a family `registerFont` returned
   * as well. A later render that lists one of its fonts registers it again.
   */
  unregisterFont(family: RegisteredFamilyLike) {
    this.fonts.forget(family.name);

    return this.inner.unregisterFont(family.name);
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
