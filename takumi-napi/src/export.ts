import type {
  Node,
  RegisteredFamily,
  RenderAnimationOptions as RenderAnimationOptionsInternal,
  RenderOptions as RenderOptionsInternal,
  RendererOptions as RendererOptionsInternal,
  SvgRenderOptions as SvgRenderOptionsInternal,
} from "../index";
export type * from "../index";
import { Renderer as RendererInternal } from "../index";

export { setGlyphCacheMaxBytes } from "../index";

import { FontRegistry, withRenderInput } from "@takumi-rs/helpers/renderer";
import { toNativeSource } from "./node-source";
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
    return withRenderInput(this.fonts, options ?? {}, node, (opts, signal) =>
      this.inner.render(toNativeSource(node), opts, signal),
    );
  }

  renderSvg(node: Node, options?: SvgRenderOptions) {
    return withRenderInput(this.fonts, options ?? {}, node, (opts, signal) =>
      this.inner.renderSvg(toNativeSource(node), opts, signal),
    );
  }

  measure(node: Node, options?: RenderOptions) {
    return withRenderInput(this.fonts, options ?? {}, node, (opts, signal) =>
      this.inner.measure(toNativeSource(node), opts, signal),
    );
  }

  renderAnimation(options: RenderAnimationOptions) {
    const nodes = options.scenes.map((scene) => scene.node);

    return withRenderInput(this.fonts, options, nodes, (opts, signal) =>
      this.inner.renderAnimation(opts, signal),
    );
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
}
